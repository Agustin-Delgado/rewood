use super::{mm, Rule, RuleInput};
use crate::diagnostics::{Diagnostic, Severity};
use crate::geometry::Vec3;
use crate::model::{OpGeometry, Operation, Part};
use crate::units::EPS;

/// A hole as a cylinder in part space: axis segment plus radius.
pub(crate) struct Cylinder {
    pub start: Vec3,
    pub end: Vec3,
    pub radius: f64,
    pub group: Option<(String, String, usize)>,
    pub op_id: String,
}

pub(crate) fn cylinders(part: &Part, material_thickness: f64) -> Vec<Cylinder> {
    part.operations
        .iter()
        .filter_map(|op| match &op.geometry {
            OpGeometry::Drill {
                u,
                v,
                diameter,
                depth,
                ..
            } => {
                let start = part.dims.uv_to_local(op.face, *u, *v);
                let normal = op.face.normal_local();
                let available = if op.face.is_large() {
                    material_thickness
                } else {
                    part.dims.along(normal)
                };
                let len = depth.unwrap_or(available);
                let end = start + normal.vec() * (-len);
                Some(Cylinder {
                    start,
                    end,
                    radius: diameter / 2.0,
                    group: op
                        .source
                        .as_ref()
                        .map(|s| (s.joint.clone(), s.hardware.clone(), s.fastener)),
                    op_id: op.id.clone(),
                })
            }
            _ => None,
        })
        .collect()
}

/// Closest distance between two segments (Ericson, Real-Time Collision
/// Detection, 5.1.9).
pub(crate) fn segment_distance(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> f64 {
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);
    let (s, t);
    if a <= EPS && e <= EPS {
        return r.length();
    }
    if a <= EPS {
        s = 0.0;
        t = (f / e).clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        if e <= EPS {
            t = 0.0;
            s = (-c / a).clamp(0.0, 1.0);
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut s_ = if denom != 0.0 {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t_ = (b * s_ + f) / e;
            if t_ < 0.0 {
                t_ = 0.0;
                s_ = (-c / a).clamp(0.0, 1.0);
            } else if t_ > 1.0 {
                t_ = 1.0;
                s_ = ((b - c) / a).clamp(0.0, 1.0);
            }
            s = s_;
            t = t_;
        }
    }
    let c1 = p1 + d1 * s;
    let c2 = p2 + d2 * t;
    (c1 - c2).length()
}

fn drill_ops(part: &Part) -> impl Iterator<Item = (&Operation, f64, f64, f64, Option<f64>)> {
    part.operations.iter().filter_map(|op| match &op.geometry {
        OpGeometry::Drill {
            u,
            v,
            diameter,
            depth,
            ..
        } => Some((op, *u, *v, *diameter, *depth)),
        _ => None,
    })
}

/// FAB-201: the hole centre lies outside its face.
pub struct HoleInsideFace;

impl Rule for HoleInsideFace {
    fn id(&self) -> &'static str {
        "FAB-201"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for part in input.parts {
            for (op, u, v, _, _) in drill_ops(part) {
                let (lu, lv) = part.dims.face_extent(op.face);
                if u < -EPS || v < -EPS || u > lu + EPS || v > lv + EPS {
                    out.push(
                        Diagnostic::new(
                            self.id(),
                            Severity::Error,
                            format!("{} / {}: la perforación en ({}, {}) cae fuera de la cara {:?} de {}×{} mm", part.id, op.id, mm(u), mm(v), op.face, mm(lu), mm(lv)),
                        )
                        .entity(part.id.clone())
                        .location(op.id.clone()),
                    );
                }
            }
        }
        out
    }
}

/// FAB-203: hole wall too close to the edge of its face.
pub struct MinEdgeDistance;

impl Rule for MinEdgeDistance {
    fn id(&self) -> &'static str {
        "FAB-203"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let min = input.libs.profile.min_edge_distance;
        let mut out = Vec::new();
        for part in input.parts {
            for (op, u, v, diameter, _) in drill_ops(part) {
                let (lu, lv) = part.dims.face_extent(op.face);
                let wall = u.min(lu - u).min(v).min(lv - v) - diameter / 2.0;
                if wall < min - EPS {
                    out.push(
                        Diagnostic::new(
                            self.id(),
                            Severity::Error,
                            format!(
                                "{} / {}: la perforación Ø{} queda a {} mm del borde; el mínimo es {} mm",
                                part.id,
                                op.id,
                                mm(diameter),
                                mm(wall.max(0.0)),
                                mm(min)
                            ),
                        )
                        .entity(part.id.clone())
                        .location(op.id.clone())
                        .suggestion(format!("Alejá la perforación a {} mm o más del borde.", mm(min))),
                    );
                }
            }
        }
        out
    }
}

/// FAB-204: a blind hole leaves less material than the profile allows, or
/// a through hole is asked on an edge.
pub struct HoleDepth;

impl Rule for HoleDepth {
    fn id(&self) -> &'static str {
        "FAB-204"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let keep = input.libs.profile.min_remaining_thickness;
        let mut out = Vec::new();
        for part in input.parts {
            let actual = input
                .libs
                .materials
                .material(&part.material)
                .map(|m| m.actual_thickness)
                .unwrap_or(part.dims.thickness);
            for (op, _, _, _, depth) in drill_ops(part) {
                let available = if op.face.is_large() {
                    actual
                } else {
                    part.dims.along(op.face.normal_local())
                };
                match depth {
                    None if !op.face.is_large() => out.push(
                        Diagnostic::new(
                            self.id(),
                            Severity::Error,
                            format!("{} / {}: perforación pasante sobre un canto", part.id, op.id),
                        )
                        .entity(part.id.clone())
                        .location(op.id.clone()),
                    ),
                    Some(d) if d > available - keep + EPS => out.push(
                        Diagnostic::new(
                            self.id(),
                            Severity::Error,
                            format!(
                                "{} / {}: perforación ciega de {} mm en {} mm de material deja {} mm; el mínimo es {} mm",
                                part.id,
                                op.id,
                                mm(d),
                                mm(available),
                                mm((available - d).max(0.0)),
                                mm(keep)
                            ),
                        )
                        .entity(part.id.clone())
                        .location(op.id.clone())
                        .suggestion("Reducí la profundidad o usá un material más grueso."),
                    ),
                    _ => {}
                }
            }
        }
        out
    }
}

/// FAB-205: two holes of different fasteners run into each other.
pub struct HoleCollision;

impl Rule for HoleCollision {
    fn id(&self) -> &'static str {
        "FAB-205"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let spacing = input.libs.profile.min_hole_spacing;
        let mut out = Vec::new();
        for part in input.parts {
            let actual = input
                .libs
                .materials
                .material(&part.material)
                .map(|m| m.actual_thickness)
                .unwrap_or(part.dims.thickness);
            let cyls = cylinders(part, actual);
            for i in 0..cyls.len() {
                for j in (i + 1)..cyls.len() {
                    let (a, b) = (&cyls[i], &cyls[j]);
                    if a.group.is_some() && a.group == b.group {
                        continue;
                    }
                    let d = segment_distance(a.start, a.end, b.start, b.end);
                    if d < a.radius + b.radius + spacing - EPS {
                        out.push(
                            Diagnostic::new(
                                self.id(),
                                Severity::Error,
                                format!(
                                    "{}: las perforaciones {} y {} se cruzan (distancia entre ejes {} mm)",
                                    part.id,
                                    a.op_id,
                                    b.op_id,
                                    mm(d)
                                ),
                            )
                            .entity(part.id.clone())
                            .location(format!("{},{}", a.op_id, b.op_id))
                            .suggestion("Cambiá el reparto de herrajes en la unión o el tipo de herraje."),
                        );
                    }
                }
            }
        }
        out
    }
}

/// FAB-208: groove leaves too little material.
pub struct GrooveDepth;

impl Rule for GrooveDepth {
    fn id(&self) -> &'static str {
        "FAB-208"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let keep = input.libs.profile.min_remaining_thickness;
        let mut out = Vec::new();
        for part in input.parts {
            let actual = input
                .libs
                .materials
                .material(&part.material)
                .map(|m| m.actual_thickness)
                .unwrap_or(part.dims.thickness);
            for op in &part.operations {
                if let OpGeometry::Groove { depth, .. } = op.geometry {
                    if depth > actual - keep + EPS {
                        out.push(
                            Diagnostic::new(
                                self.id(),
                                Severity::Error,
                                format!(
                                    "{} / {}: ranura de {} mm en {} mm de material deja {} mm; el mínimo es {} mm",
                                    part.id,
                                    op.id,
                                    mm(depth),
                                    mm(actual),
                                    mm((actual - depth).max(0.0)),
                                    mm(keep)
                                ),
                            )
                            .entity(part.id.clone())
                            .location(op.id.clone()),
                        );
                    }
                }
            }
        }
        out
    }
}

/// FAB-206: no drill of that diameter in the profile.
pub struct ToolAvailable;

impl Rule for ToolAvailable {
    fn id(&self) -> &'static str {
        "FAB-206"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let profile = &input.libs.profile;
        let mut out = Vec::new();
        for part in input.parts {
            for (op, _, _, diameter, _) in drill_ops(part) {
                if diameter < profile.min_hole_diameter - EPS {
                    out.push(
                        Diagnostic::new(
                            self.id(),
                            Severity::Error,
                            format!(
                                "{} / {}: Ø{} está por debajo del mínimo del perfil (Ø{})",
                                part.id,
                                op.id,
                                mm(diameter),
                                mm(profile.min_hole_diameter)
                            ),
                        )
                        .entity(part.id.clone())
                        .location(op.id.clone()),
                    );
                } else if !profile.has_drill(diameter) {
                    out.push(
                        Diagnostic::new(
                            self.id(),
                            Severity::Error,
                            format!(
                                "{} / {}: el perfil '{}' no tiene mecha Ø{}",
                                part.id,
                                op.id,
                                profile.id,
                                mm(diameter)
                            ),
                        )
                        .entity(part.id.clone())
                        .location(op.id.clone())
                        .suggestion("Agregá la mecha a 'tools' del perfil o cambiá el herraje."),
                    );
                }
            }
        }
        out
    }
}

/// FAB-207: operation kind the profile does not allow.
pub struct OperationAllowed;

impl Rule for OperationAllowed {
    fn id(&self) -> &'static str {
        "FAB-207"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let allowed = &input.libs.profile.allowed_operations;
        let mut out = Vec::new();
        for part in input.parts {
            for op in &part.operations {
                if !allowed.contains(&op.kind()) {
                    out.push(
                        Diagnostic::new(
                            self.id(),
                            Severity::Error,
                            format!(
                                "{} / {}: el perfil '{}' no admite operaciones {:?}",
                                part.id,
                                op.id,
                                input.libs.profile.id,
                                op.kind()
                            ),
                        )
                        .entity(part.id.clone())
                        .location(op.id.clone()),
                    );
                }
            }
        }
        out
    }
}

/// A groove as an axis-aligned box in part space.
struct Slot {
    min: Vec3,
    max: Vec3,
    pub op_id: String,
}

fn slots(part: &Part) -> Vec<Slot> {
    part.operations
        .iter()
        .filter_map(|op| match &op.geometry {
            OpGeometry::Groove {
                from,
                to,
                width,
                depth,
            } => {
                let a = part.dims.uv_to_local(op.face, from[0], from[1]);
                let b = part.dims.uv_to_local(op.face, to[0], to[1]);
                let n = op.face.normal_local();
                let (fu, fv) = op.face.uv_axes();
                // The groove runs along one in-plane axis; it is `width`
                // wide across the other and `depth` deep into the part.
                let along = if (b - a).dot(fu.vec()).abs() >= (b - a).dot(fv.vec()).abs() {
                    fu
                } else {
                    fv
                };
                let across = if along == fu { fv } else { fu };
                let half = across.vec() * (width / 2.0);
                let inward = n.vec() * (-depth);
                let corners = [
                    a,
                    b,
                    a + half,
                    b + half,
                    a - half,
                    b - half,
                    a + inward,
                    b + inward,
                ];
                let mut min = a;
                let mut max = a;
                for c in corners {
                    min = Vec3(min.0.min(c.0), min.1.min(c.1), min.2.min(c.2));
                    max = Vec3(max.0.max(c.0), max.1.max(c.1), max.2.max(c.2));
                }
                Some(Slot {
                    min,
                    max,
                    op_id: op.id.clone(),
                })
            }
            _ => None,
        })
        .collect()
}

/// Does an axis-aligned cylinder (its axis along one local axis) cut into
/// an axis-aligned box? Interval overlap along the axis, circle against
/// rectangle across it.
fn cylinder_hits_box(c: &Cylinder, s: &Slot) -> bool {
    let d = c.end - c.start;
    let axis = [d.0.abs(), d.1.abs(), d.2.abs()]
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap();
    let (lo, hi) = (
        c.start.component(axis).min(c.end.component(axis)),
        c.start.component(axis).max(c.end.component(axis)),
    );
    if hi <= s.min.component(axis) + EPS || lo >= s.max.component(axis) - EPS {
        return false;
    }
    let mut dist2 = 0.0;
    for i in (0..3).filter(|i| *i != axis) {
        let p = c.start.component(i);
        let nearest = p.max(s.min.component(i)).min(s.max.component(i));
        dist2 += (p - nearest) * (p - nearest);
    }
    dist2 < (c.radius - EPS) * (c.radius - EPS)
}

/// FAB-209: a hole cuts into a groove of the same part — the fastener
/// would sit in the slot the back panel (or drawer bottom) runs in, and
/// the panel would not go in.
pub struct HoleThroughGroove;

impl Rule for HoleThroughGroove {
    fn id(&self) -> &'static str {
        "FAB-209"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        for part in input.parts {
            let slots = slots(part);
            if slots.is_empty() {
                continue;
            }
            let t = input
                .libs
                .materials
                .material(&part.material)
                .map(|m| m.actual_thickness)
                .unwrap_or(part.dims.thickness);
            for c in cylinders(part, t) {
                for s in &slots {
                    if cylinder_hits_box(&c, s) {
                        out.push(
                            Diagnostic::new(
                                self.id(),
                                Severity::Error,
                                format!(
                                    "{}: la perforación {} cae dentro de la ranura {}",
                                    part.id, c.op_id, s.op_id
                                ),
                            )
                            .entity(part.id.clone())
                            .location(c.op_id.clone())
                            .suggestion(
                                "Corré el herraje con 'placement.endOffset' o alejá la ranura del borde ('groove.inset').",
                            ),
                        );
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_distance_cases() {
        // Parallel, offset by 10.
        let d = segment_distance(
            Vec3(0.0, 0.0, 0.0),
            Vec3(100.0, 0.0, 0.0),
            Vec3(0.0, 10.0, 0.0),
            Vec3(100.0, 10.0, 0.0),
        );
        assert!((d - 10.0).abs() < 1e-9);
        // Perpendicular, crossing.
        let d = segment_distance(
            Vec3(0.0, 0.0, 0.0),
            Vec3(100.0, 0.0, 0.0),
            Vec3(50.0, -10.0, 0.0),
            Vec3(50.0, 10.0, 0.0),
        );
        assert!(d.abs() < 1e-9);
        // Perpendicular, one ends 5 short of the other.
        let d = segment_distance(
            Vec3(0.0, 0.0, 0.0),
            Vec3(100.0, 0.0, 0.0),
            Vec3(50.0, 5.0, 0.0),
            Vec3(50.0, 20.0, 0.0),
        );
        assert!((d - 5.0).abs() < 1e-9);
        // Degenerate (a point) vs segment.
        let d = segment_distance(
            Vec3(50.0, 3.0, 0.0),
            Vec3(50.0, 3.0, 0.0),
            Vec3(0.0, 0.0, 0.0),
            Vec3(100.0, 0.0, 0.0),
        );
        assert!((d - 3.0).abs() < 1e-9);
    }
}

/// FAB-210: a cutout too close to the edge of its panel, or cutting away a
/// hole or a groove of the same part (a hinge plate, a joint, the back's
/// groove would be left hanging in the opening).
pub struct CutoutPlacement;

impl Rule for CutoutPlacement {
    fn id(&self) -> &'static str {
        "FAB-210"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let min_edge = input.libs.profile.min_edge_distance;
        let mut out = Vec::new();
        for part in input.parts {
            let t = input
                .libs
                .materials
                .material(&part.material)
                .map(|m| m.actual_thickness)
                .unwrap_or(part.dims.thickness);
            for op in &part.operations {
                let OpGeometry::Cutout {
                    u,
                    v,
                    width,
                    height,
                    ..
                } = op.geometry
                else {
                    continue;
                };
                let (lu, lv) = part.dims.face_extent(op.face);
                let margin = [
                    u - width / 2.0,
                    lu - (u + width / 2.0),
                    v - height / 2.0,
                    lv - (v + height / 2.0),
                ]
                .into_iter()
                .fold(f64::INFINITY, f64::min);
                if margin < min_edge - EPS {
                    out.push(
                        Diagnostic::new(
                            self.id(),
                            Severity::Error,
                            format!(
                                "{} / {}: el recorte queda a {} mm del borde; el mínimo es {} mm",
                                part.id,
                                op.id,
                                crate::rules::mm(margin.max(0.0)),
                                crate::rules::mm(min_edge)
                            ),
                        )
                        .entity(part.id.clone())
                        .location(op.id.clone())
                        .suggestion("Un recorte más chico o una pieza más grande."),
                    );
                }
                // The opening as a box through the part.
                let a = part
                    .dims
                    .uv_to_local(op.face, u - width / 2.0, v - height / 2.0);
                let b = part
                    .dims
                    .uv_to_local(op.face, u + width / 2.0, v + height / 2.0);
                let inward = op.face.normal_local().vec() * (-t);
                let mut min = a;
                let mut max = a;
                for c in [a, b, a + inward, b + inward] {
                    min = Vec3(min.0.min(c.0), min.1.min(c.1), min.2.min(c.2));
                    max = Vec3(max.0.max(c.0), max.1.max(c.1), max.2.max(c.2));
                }
                let opening = Slot {
                    min,
                    max,
                    op_id: op.id.clone(),
                };
                let mut lost: Vec<String> = cylinders(part, t)
                    .iter()
                    .filter(|c| cylinder_hits_box(c, &opening))
                    .map(|c| c.op_id.clone())
                    .collect();
                for g in slots(part) {
                    let overlaps = (0..3).all(|i| {
                        g.min.component(i) < opening.max.component(i) - EPS
                            && g.max.component(i) > opening.min.component(i) + EPS
                    });
                    if overlaps {
                        lost.push(g.op_id.clone());
                    }
                }
                if !lost.is_empty() {
                    out.push(
                        Diagnostic::new(
                            self.id(),
                            Severity::Error,
                            format!(
                                "{} / {}: el recorte se lleva {}",
                                part.id,
                                op.id,
                                lost.join(", ")
                            ),
                        )
                        .entity(part.id.clone())
                        .location(op.id.clone())
                        .suggestion("Corré el recorte o el herraje que cae adentro."),
                    );
                }
            }
        }
        out
    }
}
