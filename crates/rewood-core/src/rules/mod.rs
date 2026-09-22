//! Manufacturing rules: a layer independent of the geometry that looks at
//! the resolved model and says what a workshop would reject. Every rule is
//! a pure function; rules run in a fixed order and the result is sorted, so
//! the findings for a given model never change between runs.

mod design;
mod geometry;
pub(crate) mod machining;
mod material;

use crate::diagnostics::{Diagnostic, Diagnostics};
use crate::library::Libraries;
use crate::model::{Joint, Part};

pub struct RuleInput<'a> {
    pub parts: &'a [Part],
    pub joints: &'a [Joint],
    pub libs: &'a Libraries,
}

pub trait Rule {
    fn id(&self) -> &'static str;
    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic>;
}

pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(geometry::PartCollision),
        Box::new(machining::HoleInsideFace),
        Box::new(machining::MinEdgeDistance),
        Box::new(machining::HoleDepth),
        Box::new(machining::HoleCollision),
        Box::new(machining::GrooveDepth),
        Box::new(machining::HoleThroughGroove),
        Box::new(machining::ToolAvailable),
        Box::new(machining::OperationAllowed),
        Box::new(material::PartFitsSheet),
        Box::new(material::PartFitsMachine),
        Box::new(material::HardwareThickness),
        Box::new(design::PanelSpan),
        Box::new(design::HardwareLoad),
    ]
}

pub fn run_all(input: &RuleInput<'_>) -> Diagnostics {
    let mut out = Diagnostics::default();
    for rule in all_rules() {
        for d in rule.check(input) {
            out.push(d);
        }
    }
    out
}

/// Format a millimetre value for a message: up to three decimals, no
/// trailing zeros.
pub fn mm(v: f64) -> String {
    let r = crate::units::round3(v);
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}
