//! Template options: the choices a spec offers a person (how many drawers,
//! which side the pedestal goes), each bound to a literal parameter. The
//! engine resolves their bounds, checks the current values against them
//! and hands them to the UI as data; the UI draws controls and writes the
//! parameter back. Nothing in the build reads an option: the parameters
//! already carry the values.

use serde::{Deserialize, Serialize};

use crate::diagnostics::{Diagnostic, Severity};
use crate::expr::{Expr, Scope, Value};
use crate::params::ParamInput;
use crate::spec::{ChoiceSpec, FurnitureSpec, OptionSpec};
use crate::units::EPS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionKind {
    /// A number between `min` and `max`.
    Number,
    /// A boolean parameter.
    Toggle,
    /// One of `choices`.
    Choice,
}

/// An option as the UI draws it: bounds evaluated, value current.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanOption {
    pub param: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
    pub kind: OptionKind,
    pub value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<ChoiceSpec>,
    /// `false` when the option's `when` does not hold: shown dimmed, its
    /// value kept and not checked.
    pub active: bool,
}

/// Resolve every option of the spec against the evaluated parameters
/// (and the values the components published). An option that cannot be
/// resolved is left out with a finding; one whose value is out of bounds
/// stays in, with a finding and a one-click fix.
pub fn resolve(spec: &FurnitureSpec, scope: &dyn Scope) -> (Vec<PlanOption>, Vec<Diagnostic>) {
    let mut out = Vec::new();
    let mut diags = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for o in &spec.options {
        match resolve_one(o, spec, scope) {
            Ok(r) => {
                if !seen.insert(o.param.clone()) {
                    diags.push(bad(o, "aparece dos veces entre las opciones".into()));
                    continue;
                }
                if let Some(d) = out_of_bounds(&r) {
                    diags.push(d);
                }
                out.push(r);
            }
            Err(d) => diags.push(d),
        }
    }
    (out, diags)
}

fn bad(o: &OptionSpec, why: String) -> Diagnostic {
    Diagnostic::new(
        "SPEC-501",
        Severity::Error,
        format!("la opción '{}' ({}) {why}", o.label, o.param),
    )
    .entity(o.param.clone())
}

fn resolve_one(
    o: &OptionSpec,
    spec: &FurnitureSpec,
    scope: &dyn Scope,
) -> Result<PlanOption, Diagnostic> {
    let literal = match spec.parameters.get(&o.param) {
        None => return Err(bad(o, "no nombra un parámetro del mueble".into())),
        Some(ParamInput::Expr(src)) => {
            return Err(Diagnostic::new(
                "SPEC-502",
                Severity::Error,
                format!(
                    "la opción '{}' pisaría la fórmula de '{}' ({src}): una opción va sobre un valor, no sobre una expresión",
                    o.label, o.param
                ),
            )
            .entity(o.param.clone()))
        }
        Some(v) => v,
    };
    let value = match literal {
        ParamInput::Number(n) => Value::Number(*n),
        ParamInput::Bool(b) => Value::Bool(*b),
        ParamInput::Expr(_) => unreachable!(),
    };
    let number = |field: &str, v: &Option<ParamInput>| -> Result<Option<f64>, Diagnostic> {
        match v {
            None => Ok(None),
            Some(ParamInput::Number(n)) => Ok(Some(*n)),
            Some(ParamInput::Bool(_)) => Err(bad(o, format!("tiene '{field}' booleano"))),
            Some(ParamInput::Expr(src)) => Expr::parse(src)
                .and_then(|e| e.eval(scope))
                .and_then(|v| v.as_number())
                .map(Some)
                .map_err(|e| bad(o, format!("no pudo evaluar '{field}': {e}"))),
        }
    };
    let min = number("min", &o.min)?;
    let max = number("max", &o.max)?;
    let active = match &o.when {
        None => true,
        Some(ParamInput::Bool(b)) => *b,
        Some(ParamInput::Number(_)) => return Err(bad(o, "tiene un 'when' numérico".into())),
        Some(ParamInput::Expr(src)) => match Expr::parse(src).and_then(|e| e.eval(scope)) {
            Ok(Value::Bool(b)) => b,
            Ok(Value::Number(_)) => {
                return Err(bad(
                    o,
                    format!("tiene un 'when' que no es una condición: {src}"),
                ))
            }
            Err(e) => return Err(bad(o, format!("no pudo evaluar 'when': {e}"))),
        },
    };
    let kind = match (value, o.choices.is_empty()) {
        (Value::Bool(_), _) => OptionKind::Toggle,
        (Value::Number(_), false) => OptionKind::Choice,
        (Value::Number(_), true) => OptionKind::Number,
    };
    Ok(PlanOption {
        param: o.param.clone(),
        label: o.label.clone(),
        group: o.group.clone(),
        help: o.help.clone(),
        kind,
        value,
        min,
        max,
        step: o.step,
        unit: o.unit.clone(),
        choices: o.choices.clone(),
        active,
    })
}

/// SPEC-503: an active option whose value the template does not allow.
fn out_of_bounds(r: &PlanOption) -> Option<Diagnostic> {
    let Value::Number(v) = r.value else {
        return None;
    };
    if !r.active {
        return None;
    }
    let (why, fixed) = match r.kind {
        OptionKind::Choice => {
            if r.choices.iter().any(|c| (c.value - v).abs() <= EPS) {
                return None;
            }
            let names: Vec<&str> = r.choices.iter().map(|c| c.label.as_str()).collect();
            (
                format!("vale {v} y se puede elegir {}", names.join(", ")),
                r.choices.first().map(|c| c.value)?,
            )
        }
        _ => {
            let lo = r.min.unwrap_or(f64::NEG_INFINITY);
            let hi = r.max.unwrap_or(f64::INFINITY);
            if lo > hi + EPS {
                // Bounds from expressions can cross: nothing is allowed.
                return Some(
                    Diagnostic::new(
                        "SPEC-503",
                        Severity::Error,
                        format!(
                            "'{}' ({}) no tiene valor posible: va de {lo} a {hi}",
                            r.label, r.param
                        ),
                    )
                    .entity(r.param.clone()),
                );
            }
            if v >= lo - EPS && v <= hi + EPS {
                return None;
            }
            let range = match (r.min, r.max) {
                (Some(a), Some(b)) => format!("entre {a} y {b}"),
                (Some(a), None) => format!("desde {a}"),
                (None, Some(b)) => format!("hasta {b}"),
                (None, None) => unreachable!(),
            };
            (format!("vale {v} y va {range}"), v.clamp(lo, hi))
        }
    };
    Some(
        Diagnostic::new(
            "SPEC-503",
            Severity::Error,
            format!("'{}' ({}) {why}", r.label, r.param),
        )
        .entity(r.param.clone())
        .fix(
            format!("Dejarlo en {fixed}"),
            "",
            format!("parameters.{}", r.param),
            serde_json::json!(fixed),
        ),
    )
}
