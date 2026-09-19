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

fn weight_kg(part: &crate::model::Part, libs: &crate::library::Libraries) -> f64 {
    let density = libs
        .materials
        .material(&part.material)
        .map(|m| m.density)
        .unwrap_or(0.0);
    part.dims.volume() / 1e9 * density
}

/// Contents a drawer is expected to carry on top of its own weight, kg.
/// A kitchen drawer of pots is more; the message says what was assumed.
const DRAWER_CONTENTS_KG: f64 = 10.0;

/// DESIGN-111: a door heavier than its hinges carry, or a drawer whose
/// box plus a nominal load exceeds the slides' rating. Uses `maxLoadKg`
/// from the hardware library; hardware without it is not judged.
pub struct HardwareLoad;

impl Rule for HardwareLoad {
    fn id(&self) -> &'static str {
        "DESIGN-111"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        let part = |id: &str| input.parts.iter().find(|p| p.id == id);
        for joint in input.joints {
            match joint.kind.as_str() {
                "hinge" => {
                    let Some(door) = part(&joint.edge_part) else {
                        continue;
                    };
                    let Some(hw) = joint
                        .hardware
                        .iter()
                        .find_map(|h| input.libs.hardware.get(h))
                    else {
                        continue;
                    };
                    let Some(max) = hw.max_load_kg else {
                        continue;
                    };
                    let n = joint.fasteners.len().max(1);
                    let w = weight_kg(door, input.libs);
                    let each = w / n as f64;
                    if each > max + 1e-9 {
                        out.push(
                            Diagnostic::new(
                                self.id(),
                                Severity::Warning,
                                format!(
                                    "{} ({}): {} kg colgados de {} bisagras '{}' de {} kg cada una",
                                    door.id,
                                    door.name,
                                    kg(w),
                                    n,
                                    hw.name,
                                    kg(max)
                                ),
                            )
                            .entity(door.component.clone())
                            .location(door.id.clone())
                            .suggestion(
                                "Más bisagras ('hinge.placement'), una puerta más angosta o una placa más liviana.",
                            ),
                        );
                    }
                }
                "slide" => {
                    // One joint per side; judge the pair once, from the left.
                    let Some(side) = part(&joint.edge_part) else {
                        continue;
                    };
                    if !side.role.ends_with("_side_left") {
                        continue;
                    }
                    let Some(hw) = joint
                        .hardware
                        .iter()
                        .find_map(|h| input.libs.hardware.get(h))
                    else {
                        continue;
                    };
                    let Some(max) = hw.max_load_kg else {
                        continue;
                    };
                    let prefix = side.role.trim_end_matches("_side_left");
                    let own: f64 = input
                        .parts
                        .iter()
                        .filter(|p| p.component == side.component && p.role.starts_with(prefix))
                        .map(|p| weight_kg(p, input.libs))
                        .sum();
                    let total = own + DRAWER_CONTENTS_KG;
                    if total > max + 1e-9 {
                        out.push(
                            Diagnostic::new(
                                self.id(),
                                Severity::Warning,
                                format!(
                                    "cajón {} de '{}': la caja pesa {} kg y con {} kg de contenido supera los {} kg de '{}'",
                                    prefix.trim_start_matches("drawer_"),
                                    side.component,
                                    kg(own),
                                    kg(DRAWER_CONTENTS_KG),
                                    kg(max),
                                    hw.name
                                ),
                            )
                            .entity(side.component.clone())
                            .location(side.id.clone())
                            .suggestion("Una corredera de más carga, o una caja más chica o más liviana."),
                        );
                    }
                }
                _ => {}
            }
        }
        out
    }
}

fn kg(v: f64) -> String {
    mm((v * 10.0).round() / 10.0)
}
