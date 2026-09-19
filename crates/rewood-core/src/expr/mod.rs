//! Expression language for parameters and constraints.
//!
//! `internal_width = width - 2 * thickness`, `door.width <= carcass.width`,
//! `max(2, ceil(width / 600))`. Numbers are millimetres (or dimensionless);
//! booleans come from comparisons and `and`/`or`/`not`.

mod lexer;
mod parser;

use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ExprError {
    #[error("error léxico: {0}")]
    Lex(String),
    #[error("error de sintaxis: {0}")]
    Parse(String),
    #[error("referencia desconocida '{0}'")]
    UnknownRef(String),
    #[error("función desconocida '{0}'")]
    UnknownFunction(String),
    #[error("la función '{0}' esperaba {1} argumentos")]
    Arity(String, usize),
    #[error("tipo inválido: {0}")]
    Type(String),
    #[error("división por cero")]
    DivByZero,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Bool(bool),
    Ref(String),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

/// Runtime value of an expression. Strings and enums from the spec are
/// resolved before evaluation; the evaluator only sees numbers and booleans.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    Number(f64),
    Bool(bool),
}

impl Value {
    pub fn as_number(self) -> Result<f64, ExprError> {
        match self {
            Value::Number(v) => Ok(v),
            Value::Bool(_) => Err(ExprError::Type(
                "se esperaba un número, llegó un booleano".into(),
            )),
        }
    }

    pub fn as_bool(self) -> Result<bool, ExprError> {
        match self {
            Value::Bool(b) => Ok(b),
            Value::Number(_) => Err(ExprError::Type(
                "se esperaba un booleano, llegó un número".into(),
            )),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(v) => write!(f, "{v}"),
            Value::Bool(b) => write!(f, "{b}"),
        }
    }
}

/// Anything that can resolve a reference name to a value.
pub trait Scope {
    fn lookup(&self, name: &str) -> Option<Value>;
}

impl Scope for std::collections::BTreeMap<String, Value> {
    fn lookup(&self, name: &str) -> Option<Value> {
        self.get(name).copied()
    }
}

impl Scope for std::collections::BTreeMap<String, f64> {
    fn lookup(&self, name: &str) -> Option<Value> {
        self.get(name).map(|v| Value::Number(*v))
    }
}

/// Two scopes chained: the first one wins.
pub struct Chain<'a>(pub &'a dyn Scope, pub &'a dyn Scope);

impl Scope for Chain<'_> {
    fn lookup(&self, name: &str) -> Option<Value> {
        self.0.lookup(name).or_else(|| self.1.lookup(name))
    }
}

impl Expr {
    pub fn parse(src: &str) -> Result<Expr, ExprError> {
        let tokens = lexer::tokenize(src)?;
        parser::Parser::new(tokens).parse()
    }

    /// Names this expression reads, sorted and deduplicated.
    pub fn references(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        self.collect_refs(&mut out);
        out
    }

    fn collect_refs(&self, out: &mut BTreeSet<String>) {
        match self {
            Expr::Number(_) | Expr::Bool(_) => {}
            Expr::Ref(n) => {
                out.insert(n.clone());
            }
            Expr::Unary(_, e) => e.collect_refs(out),
            Expr::Binary(_, a, b) => {
                a.collect_refs(out);
                b.collect_refs(out);
            }
            Expr::Call(_, args) => args.iter().for_each(|a| a.collect_refs(out)),
        }
    }

    pub fn eval(&self, scope: &dyn Scope) -> Result<Value, ExprError> {
        match self {
            Expr::Number(v) => Ok(Value::Number(*v)),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Ref(name) => scope
                .lookup(name)
                .ok_or_else(|| ExprError::UnknownRef(name.clone())),
            Expr::Unary(UnOp::Neg, e) => Ok(Value::Number(-e.eval(scope)?.as_number()?)),
            Expr::Unary(UnOp::Not, e) => Ok(Value::Bool(!e.eval(scope)?.as_bool()?)),
            Expr::Binary(op, a, b) => {
                let a = a.eval(scope)?;
                let b = b.eval(scope)?;
                Ok(match op {
                    BinOp::Add => Value::Number(a.as_number()? + b.as_number()?),
                    BinOp::Sub => Value::Number(a.as_number()? - b.as_number()?),
                    BinOp::Mul => Value::Number(a.as_number()? * b.as_number()?),
                    BinOp::Div => {
                        let d = b.as_number()?;
                        if d == 0.0 {
                            return Err(ExprError::DivByZero);
                        }
                        Value::Number(a.as_number()? / d)
                    }
                    BinOp::Mod => {
                        let d = b.as_number()?;
                        if d == 0.0 {
                            return Err(ExprError::DivByZero);
                        }
                        Value::Number(a.as_number()? % d)
                    }
                    BinOp::Lt => Value::Bool(a.as_number()? < b.as_number()?),
                    BinOp::Le => Value::Bool(a.as_number()? <= b.as_number()?),
                    BinOp::Gt => Value::Bool(a.as_number()? > b.as_number()?),
                    BinOp::Ge => Value::Bool(a.as_number()? >= b.as_number()?),
                    BinOp::Eq => Value::Bool(match (a, b) {
                        (Value::Number(x), Value::Number(y)) => (x - y).abs() <= crate::units::EPS,
                        (Value::Bool(x), Value::Bool(y)) => x == y,
                        _ => false,
                    }),
                    BinOp::Ne => Value::Bool(match (a, b) {
                        (Value::Number(x), Value::Number(y)) => (x - y).abs() > crate::units::EPS,
                        (Value::Bool(x), Value::Bool(y)) => x != y,
                        _ => true,
                    }),
                    BinOp::And => Value::Bool(a.as_bool()? && b.as_bool()?),
                    BinOp::Or => Value::Bool(a.as_bool()? || b.as_bool()?),
                })
            }
            Expr::Call(name, args) => call(name, args, scope),
        }
    }
}

fn call(name: &str, args: &[Expr], scope: &dyn Scope) -> Result<Value, ExprError> {
    let nums = |n: usize| -> Result<Vec<f64>, ExprError> {
        if args.len() != n {
            return Err(ExprError::Arity(name.to_string(), n));
        }
        args.iter().map(|a| a.eval(scope)?.as_number()).collect()
    };
    Ok(match name {
        "abs" => Value::Number(nums(1)?[0].abs()),
        "floor" => Value::Number(nums(1)?[0].floor()),
        "ceil" => Value::Number(nums(1)?[0].ceil()),
        "round" => Value::Number(nums(1)?[0].round()),
        "sqrt" => Value::Number(nums(1)?[0].sqrt()),
        "min" | "max" => {
            if args.is_empty() {
                return Err(ExprError::Arity(name.to_string(), 1));
            }
            let vals: Vec<f64> = args
                .iter()
                .map(|a| a.eval(scope)?.as_number())
                .collect::<Result<_, _>>()?;
            let v = if name == "min" {
                vals.iter().cloned().fold(f64::INFINITY, f64::min)
            } else {
                vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            };
            Value::Number(v)
        }
        "clamp" => {
            let v = nums(3)?;
            Value::Number(v[0].max(v[1]).min(v[2]))
        }
        "if" => {
            if args.len() != 3 {
                return Err(ExprError::Arity(name.to_string(), 3));
            }
            let cond = args[0].eval(scope)?.as_bool()?;
            if cond {
                args[1].eval(scope)?
            } else {
                args[2].eval(scope)?
            }
        }
        _ => return Err(ExprError::UnknownFunction(name.to_string())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn scope() -> BTreeMap<String, f64> {
        let mut s = BTreeMap::new();
        s.insert("width".into(), 1800.0);
        s.insert("thickness".into(), 18.0);
        s.insert("carcass.width".into(), 1000.0);
        s
    }

    #[test]
    fn arithmetic_and_precedence() {
        let e = Expr::parse("width - 2 * thickness").unwrap();
        assert_eq!(e.eval(&scope()).unwrap(), Value::Number(1764.0));
        let e = Expr::parse("(width - 2 * thickness) / 2").unwrap();
        assert_eq!(e.eval(&scope()).unwrap(), Value::Number(882.0));
        let e = Expr::parse("-thickness + 20").unwrap();
        assert_eq!(e.eval(&scope()).unwrap(), Value::Number(2.0));
    }

    #[test]
    fn comparisons_and_logic() {
        let e = Expr::parse("width <= 2800 and thickness >= 16").unwrap();
        assert_eq!(e.eval(&scope()).unwrap(), Value::Bool(true));
        let e = Expr::parse("not (width > 1000) or carcass.width == 1000").unwrap();
        assert_eq!(e.eval(&scope()).unwrap(), Value::Bool(true));
    }

    #[test]
    fn functions() {
        let e = Expr::parse("max(2, ceil((width - 100) / 600) + 1)").unwrap();
        assert_eq!(e.eval(&scope()).unwrap(), Value::Number(4.0));
        let e = Expr::parse("if(width > 1500, 3, 2)").unwrap();
        assert_eq!(e.eval(&scope()).unwrap(), Value::Number(3.0));
        let e = Expr::parse("clamp(width, 0, 1000)").unwrap();
        assert_eq!(e.eval(&scope()).unwrap(), Value::Number(1000.0));
    }

    #[test]
    fn references_are_collected() {
        let e = Expr::parse("width - left.thickness - right.thickness").unwrap();
        let refs: Vec<_> = e.references().into_iter().collect();
        assert_eq!(refs, vec!["left.thickness", "right.thickness", "width"]);
    }

    #[test]
    fn errors() {
        assert!(matches!(
            Expr::parse("width +").unwrap_err(),
            ExprError::Parse(_)
        ));
        assert!(matches!(
            Expr::parse("width = 3").unwrap_err(),
            ExprError::Lex(_)
        ));
        let e = Expr::parse("depth * 2").unwrap();
        assert_eq!(
            e.eval(&scope()).unwrap_err(),
            ExprError::UnknownRef("depth".into())
        );
        let e = Expr::parse("width / 0").unwrap();
        assert_eq!(e.eval(&scope()).unwrap_err(), ExprError::DivByZero);
    }
}
