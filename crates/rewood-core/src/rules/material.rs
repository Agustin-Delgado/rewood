use super::{mm, Rule, RuleInput};
use crate::diagnostics::{Diagnostic, Severity};
use crate::library::material::GrainKind;
use crate::model::Grain;
use crate::units::EPS;

fn fits(a: f64, b: f64, la: f64, lb: f64) -> bool {
    a <= la + EPS && b <= lb + EPS
}

/// FAB-301: the raw panel does not come out of one sheet. With directional
/// grain the part's grain must follow the sheet's length, so only one
/// orientation counts; otherwise either does.
pub struct PartFitsSheet;

impl Rule for PartFitsSheet {
    fn id(&self) -> &'static str {
        "FAB-301"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for part in input.parts {
            let Some(m) = input
                .libs
                .materials
                .stock(&part.material, part.decor.as_deref())
            else {
                continue;
            };
            let (l, w) = (part.cut.length, part.cut.width);
            // The same room the nesting has: the sheet less its margin all
            // round. A part that only fits the bare sheet is left out of
            // every layout, and the BOM would buy one sheet short.
            let margin = input.libs.profile.nesting.margin;
            let (sl, sw) = (m.sheet_length - 2.0 * margin, m.sheet_width - 2.0 * margin);
            let ok = match (m.grain, part.grain) {
                (GrainKind::Directional, Grain::Length) => fits(l, w, sl, sw),
                (GrainKind::Directional, Grain::Width) => fits(w, l, sl, sw),
                _ => fits(l, w, sl, sw) || fits(w, l, sl, sw),
            };
            if !ok {
                out.push(
                    Diagnostic::new(
                        self.id(),
                        Severity::Error,
                        format!(
                            "{} ({}): {}×{} mm no sale de una placa de {}×{} mm de {} (útil {}×{} con {} de margen)",
                            part.id,
                            part.name,
                            mm(l),
                            mm(w),
                            mm(m.sheet_length),
                            mm(m.sheet_width),
                            m.name,
                            mm(sl),
                            mm(sw),
                            mm(margin)
                        ),
                    )
                    .entity(part.id.clone())
                    .suggestion("Dividí el componente o cambiá la orientación de la veta."),
                );
            }
        }
        out
    }
}

/// FAB-302 / FAB-303: part larger than the machine's work area, or too
/// small to be held.
pub struct PartFitsMachine;

impl Rule for PartFitsMachine {
    fn id(&self) -> &'static str {
        "FAB-302"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let p = &input.libs.profile;
        let mut out = Vec::new();
        for part in input.parts.iter().filter(|p| !p.outsourced) {
            let (l, w) = (part.cut.length, part.cut.width);
            let [ml, mw] = p.max_part_size;
            if !(fits(l, w, ml, mw) || fits(w, l, ml, mw)) {
                out.push(
                    Diagnostic::new(
                        self.id(),
                        Severity::Error,
                        format!(
                            "{} ({}): {}×{} mm excede el área de trabajo de '{}' ({}×{} mm)",
                            part.id,
                            part.name,
                            mm(l),
                            mm(w),
                            p.id,
                            mm(ml),
                            mm(mw)
                        ),
                    )
                    .entity(part.id.clone()),
                );
            }
            let [nl, nw] = p.min_part_size;
            if l < nl - EPS || w < nw - EPS {
                out.push(
                    Diagnostic::new(
                        "FAB-303",
                        Severity::Warning,
                        format!("{} ({}): {}×{} mm es más chica que el mínimo sujetable de '{}' ({}×{} mm)", part.id, part.name, mm(l), mm(w), p.id, mm(nl), mm(nw)),
                    )
                    .entity(part.id.clone()),
                );
            }
        }
        out
    }
}

/// FAB-304: hardware used on a panel thickness it is not made for.
pub struct HardwareThickness;

impl Rule for HardwareThickness {
    fn id(&self) -> &'static str {
        "FAB-304"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for joint in input.joints {
            for hw_id in &joint.hardware {
                let Some(hw) = input.libs.hardware.get(hw_id) else {
                    continue;
                };
                let [lo, hi] = hw.compatible_thickness;
                for part_id in [&joint.edge_part, &joint.face_part] {
                    let Some(part) = input.parts.iter().find(|p| &p.id == part_id) else {
                        continue;
                    };
                    let t = part.dims.thickness;
                    if t < lo - EPS || t > hi + EPS {
                        out.push(
                            Diagnostic::new(
                                self.id(),
                                Severity::Error,
                                format!(
                                    "{}: '{}' está pensado para {}–{} mm y {} ({}) tiene {} mm",
                                    joint.id,
                                    hw.name,
                                    mm(lo),
                                    mm(hi),
                                    part.id,
                                    part.name,
                                    mm(t)
                                ),
                            )
                            .entity(joint.id.clone())
                            .location(part.id.clone())
                            .suggestion("Elegí un herraje compatible con ese espesor."),
                        );
                    }
                }
            }
        }
        out
    }
}
