//! Parameter graph: named values, some literal, some expressions over other
//! parameters. Evaluation follows a topological order of the dependency graph,
//! so a change to `width` re-evaluates `inner_width`, then `door_width`, then
//! whatever depends on that — and a cycle is an error, never a hang.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::expr::{Expr, ExprError, Scope, Value};

/// A parameter as written in the spec: a literal or an expression string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamInput {
    Number(f64),
    Bool(bool),
    Expr(String),
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ParamError {
    #[error("parámetro '{name}': {source}")]
    Expr { name: String, source: ExprError },
    #[error("ciclo de dependencias entre parámetros: {}", .0.join(" -> "))]
    Cycle(Vec<String>),
    #[error("parámetro desconocido '{0}'")]
    Unknown(String),
}

#[derive(Debug, Clone)]
struct Node {
    expr: Option<Expr>,
    value: Value,
}

#[derive(Debug, Clone)]
pub struct ParamGraph {
    nodes: BTreeMap<String, Node>,
    /// Evaluation order: every name appears after everything it references.
    order: Vec<String>,
}

impl ParamGraph {
    pub fn build(inputs: &BTreeMap<String, ParamInput>) -> Result<Self, ParamError> {
        let mut nodes = BTreeMap::new();
        for (name, input) in inputs {
            let node = match input {
                ParamInput::Number(v) => Node {
                    expr: None,
                    value: Value::Number(*v),
                },
                ParamInput::Bool(b) => Node {
                    expr: None,
                    value: Value::Bool(*b),
                },
                ParamInput::Expr(src) => {
                    let expr = Expr::parse(src).map_err(|source| ParamError::Expr {
                        name: name.clone(),
                        source,
                    })?;
                    Node {
                        expr: Some(expr),
                        value: Value::Number(f64::NAN),
                    }
                }
            };
            nodes.insert(name.clone(), node);
        }
        let order = topo_order(&nodes)?;
        let mut graph = ParamGraph { nodes, order };
        graph.eval_all()?;
        Ok(graph)
    }

    fn eval_all(&mut self) -> Result<(), ParamError> {
        let order = self.order.clone();
        for name in order {
            self.eval_one(&name)?;
        }
        Ok(())
    }

    fn eval_one(&mut self, name: &str) -> Result<(), ParamError> {
        let Some(expr) = self.nodes[name].expr.clone() else {
            return Ok(());
        };
        let value = expr.eval(self).map_err(|source| ParamError::Expr {
            name: name.to_string(),
            source,
        })?;
        self.nodes.get_mut(name).unwrap().value = value;
        Ok(())
    }

    /// Override a literal parameter and recompute everything downstream of it.
    pub fn set(&mut self, name: &str, value: Value) -> Result<(), ParamError> {
        let node = self
            .nodes
            .get_mut(name)
            .ok_or_else(|| ParamError::Unknown(name.to_string()))?;
        node.expr = None;
        node.value = value;
        let dependents = self.dependents_of(name);
        let order = self.order.clone();
        for n in order.iter().filter(|n| dependents.contains(*n)) {
            self.eval_one(n)?;
        }
        Ok(())
    }

    /// Transitive closure of the names that read `name`, directly or not.
    pub fn dependents_of(&self, name: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let mut frontier = vec![name.to_string()];
        while let Some(current) = frontier.pop() {
            for (other, node) in &self.nodes {
                if let Some(expr) = &node.expr {
                    if expr.references().contains(&current) && out.insert(other.clone()) {
                        frontier.push(other.clone());
                    }
                }
            }
        }
        out
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        self.nodes.get(name).map(|n| n.value)
    }

    pub fn number(&self, name: &str) -> Result<f64, ParamError> {
        self.get(name)
            .ok_or_else(|| ParamError::Unknown(name.to_string()))?
            .as_number()
            .map_err(|source| ParamError::Expr {
                name: name.to_string(),
                source,
            })
    }

    pub fn values(&self) -> BTreeMap<String, Value> {
        self.nodes
            .iter()
            .map(|(k, n)| (k.clone(), n.value))
            .collect()
    }

    pub fn order(&self) -> &[String] {
        &self.order
    }
}

impl Scope for ParamGraph {
    fn lookup(&self, name: &str) -> Option<Value> {
        self.get(name)
    }
}

/// Kahn's algorithm over the reference graph. Names are processed in sorted
/// order at every step, so the result is deterministic regardless of input
/// order. References to names outside the graph are left for evaluation to
/// report (they may be resolved by an outer scope in the future).
fn topo_order(nodes: &BTreeMap<String, Node>) -> Result<Vec<String>, ParamError> {
    let deps: BTreeMap<&str, BTreeSet<String>> = nodes
        .iter()
        .map(|(name, node)| {
            let refs = node
                .expr
                .as_ref()
                .map(|e| {
                    e.references()
                        .into_iter()
                        .filter(|r| nodes.contains_key(r))
                        .collect()
                })
                .unwrap_or_default();
            (name.as_str(), refs)
        })
        .collect();

    let mut remaining: BTreeSet<&str> = nodes.keys().map(String::as_str).collect();
    let mut done: BTreeSet<&str> = BTreeSet::new();
    let mut order = Vec::new();
    while !remaining.is_empty() {
        let ready: Vec<&str> = remaining
            .iter()
            .copied()
            .filter(|n| deps[n].iter().all(|d| done.contains(d.as_str())))
            .collect();
        if ready.is_empty() {
            return Err(ParamError::Cycle(find_cycle(&deps, &remaining)));
        }
        for n in ready {
            remaining.remove(n);
            done.insert(n);
            order.push(n.to_string());
        }
    }
    Ok(order)
}

fn find_cycle(deps: &BTreeMap<&str, BTreeSet<String>>, remaining: &BTreeSet<&str>) -> Vec<String> {
    // Walk from the smallest remaining name following unresolved deps until a
    // name repeats; that suffix is a cycle.
    let start = *remaining.iter().next().unwrap();
    let mut path = vec![start.to_string()];
    let mut current = start;
    loop {
        let next = deps[current]
            .iter()
            .find(|d| remaining.contains(d.as_str()))
            .map(String::as_str)
            .unwrap_or(current);
        if let Some(pos) = path.iter().position(|p| p == next) {
            let mut cycle = path[pos..].to_vec();
            cycle.push(next.to_string());
            return cycle;
        }
        path.push(next.to_string());
        current = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(pairs: &[(&str, ParamInput)]) -> BTreeMap<String, ParamInput> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn evaluates_in_dependency_order() {
        let g = ParamGraph::build(&inputs(&[
            (
                "hinge_count",
                ParamInput::Expr("max(2, ceil(door_height / 500))".into()),
            ),
            ("door_height", ParamInput::Expr("height - 4".into())),
            (
                "inner_width",
                ParamInput::Expr("width - 2 * thickness".into()),
            ),
            ("thickness", ParamInput::Number(18.0)),
            ("width", ParamInput::Number(1800.0)),
            ("height", ParamInput::Number(2100.0)),
        ]))
        .unwrap();
        assert_eq!(g.number("inner_width").unwrap(), 1764.0);
        assert_eq!(g.number("door_height").unwrap(), 2096.0);
        assert_eq!(g.number("hinge_count").unwrap(), 5.0);
        let order = g.order();
        let idx = |n: &str| order.iter().position(|o| o == n).unwrap();
        assert!(idx("height") < idx("door_height"));
        assert!(idx("door_height") < idx("hinge_count"));
    }

    #[test]
    fn set_propagates_to_dependents() {
        let mut g = ParamGraph::build(&inputs(&[
            ("width", ParamInput::Number(1200.0)),
            ("thickness", ParamInput::Number(18.0)),
            (
                "inner_width",
                ParamInput::Expr("width - 2 * thickness".into()),
            ),
            ("door_width", ParamInput::Expr("inner_width / 2".into())),
        ]))
        .unwrap();
        assert_eq!(g.number("door_width").unwrap(), 582.0);
        g.set("width", Value::Number(1800.0)).unwrap();
        assert_eq!(g.number("inner_width").unwrap(), 1764.0);
        assert_eq!(g.number("door_width").unwrap(), 882.0);
        let deps: Vec<_> = g.dependents_of("width").into_iter().collect();
        assert_eq!(deps, vec!["door_width", "inner_width"]);
    }

    #[test]
    fn detects_cycles() {
        let err = ParamGraph::build(&inputs(&[
            ("a", ParamInput::Expr("b + 1".into())),
            ("b", ParamInput::Expr("c + 1".into())),
            ("c", ParamInput::Expr("a + 1".into())),
        ]))
        .unwrap_err();
        match err {
            ParamError::Cycle(path) => {
                assert_eq!(path.first(), path.last());
                assert!(path.len() >= 3);
            }
            other => panic!("expected cycle, got {other:?}"),
        }
    }

    #[test]
    fn reports_unknown_reference() {
        let err =
            ParamGraph::build(&inputs(&[("a", ParamInput::Expr("depth * 2".into()))])).unwrap_err();
        assert!(matches!(
            err,
            ParamError::Expr {
                source: ExprError::UnknownRef(_),
                ..
            }
        ));
    }
}
