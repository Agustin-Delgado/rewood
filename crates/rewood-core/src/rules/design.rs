//! Design heuristics on the resolved parts: what a cabinetmaker would
//! point at before anything is cut. None of these block manufacturing;
//! they say "this is probably not what you want" with the number that
//! says why.

use std::collections::BTreeMap;

use super::{mm, Rule, RuleInput};
use crate::diagnostics::{Diagnostic, Severity};
use crate::geometry::Axis;
use crate::units::EPS;

/// DESIGN-101: a shelf bridges more than the sheet's unsupported span, so
/// it will sag under load. One finding per component, listing the
/// panels, so a bookcase with six shelves reads as one problem and not
/// six. (The carcass top and bottom get the same check from their
/// generator, which knows where the dividers are.)
pub struct PanelSpan;

impl Rule for PanelSpan {
    fn id(&self) -> &'static str {
        "DESIGN-101"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        // component → (material, span, max, part ids)
        let mut hits: BTreeMap<String, (String, f64, f64, Vec<String>)> = BTreeMap::new();
        for part in input.parts {
            let horizontal = part.face_facing(Axis::PosZ).is_large();
            if !horizontal || !is_bridging_role(&part.role) {
                continue;
            }
            let Some(m) = input.libs.materials.material(&part.material) else {
                continue;
            };
            let span = part.aabb.size().0;
            let max = m.max_span.unwrap_or(50.0 * m.nominal_thickness);
            if span > max + EPS {
                let e = hits
                    .entry(part.component.clone())
                    .or_insert_with(|| (m.name.clone(), span, max, Vec::new()));
                e.1 = e.1.max(span);
                e.3.push(part.id.clone());
            }
        }
        hits.into_iter()
            .map(|(component, (material, span, max, ids))| {
                let what = if ids.len() == 1 {
                    format!("el panel {} de '{component}'", ids[0])
                } else {
                    format!("{} paneles de '{component}' ({})", ids.len(), ids.join(", "))
                };
                Diagnostic::new(
                    self.id(),
                    Severity::Warning,
                    format!(
                        "{what}: {} mm de luz en {material}, que aguanta {} sin pandear",
                        mm(span),
                        mm(max)
                    ),
                )
                .entity(component)
                .location(ids[0].clone())
                .suggestion("Partí el ancho con un divisor (bays), agregá un estante fijo o usá una placa más gruesa.")
            })
            .collect()
    }
}

/// Shelves, spread or fixed: a panel resting on its two ends. Drawer
/// bottoms sit in a groove on four sides and backs are vertical.
fn is_bridging_role(role: &str) -> bool {
    role.contains("shelf")
}
