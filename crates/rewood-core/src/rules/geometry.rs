use super::{mm, Rule, RuleInput};
use crate::diagnostics::{Diagnostic, Severity};

/// FAB-101: two parts share volume. Touching is fine; overlapping is not,
/// unless the parts declared each other exempt (a back inside its groove).
pub struct PartCollision;

impl Rule for PartCollision {
    fn id(&self) -> &'static str {
        "FAB-101"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let mut out = Vec::new();
        let parts = input.parts;
        for i in 0..parts.len() {
            for j in (i + 1)..parts.len() {
                let (a, b) = (&parts[i], &parts[j]);
                if a.overlap_exempt.contains(&b.id) || b.overlap_exempt.contains(&a.id) {
                    continue;
                }
                if let Some(x) = a.aabb.intersection(&b.aabb) {
                    let s = x.size();
                    out.push(
                        Diagnostic::new(
                            self.id(),
                            Severity::Error,
                            format!(
                                "{} ({}) y {} ({}) se superponen {}×{}×{} mm",
                                a.id,
                                a.name,
                                b.id,
                                b.name,
                                mm(s.0),
                                mm(s.1),
                                mm(s.2)
                            ),
                        )
                        .entity(a.id.clone())
                        .location(b.id.clone())
                        .suggestion("Revisá las dimensiones o la posición de los componentes que las generan."),
                    );
                }
            }
        }
        out
    }
}

/// FAB-102: a leg's body shares volume with a part or with another leg.
/// A leg is the one piece of hardware that stands in open space, so the
/// part check alone does not see it: a plinth set in front of the legs
/// can still end up running through them. The leg is taken as its base
/// plate's square, as tall as the leg, hanging from its mounting face.
pub struct LegCollision;

impl Rule for LegCollision {
    fn id(&self) -> &'static str {
        "FAB-102"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let mut legs: Vec<(&str, crate::geometry::Aabb)> = Vec::new();
        for joint in input.joints {
            if joint.kind != "fixture" {
                continue;
            }
            let Some(leg) = joint
                .hardware
                .first()
                .and_then(|h| input.libs.hardware.get(h))
                .and_then(|h| h.leg.as_ref())
            else {
                continue;
            };
            let Some(part) = input.parts.iter().find(|p| p.id == joint.face_part) else {
                continue;
            };
            legs.push((
                joint.id.as_str(),
                leg_envelope(joint.contact.center(), &part.aabb, leg),
            ));
        }
        let mut out = Vec::new();
        for (i, (joint, leg)) in legs.iter().enumerate() {
            for part in input.parts {
                if let Some(x) = leg.intersection(&part.aabb) {
                    let s = x.size();
                    out.push(
                        Diagnostic::new(
                            self.id(),
                            Severity::Error,
                            format!(
                                "la pata {joint} atraviesa {} ({}): {}×{}×{} mm",
                                part.id,
                                part.name,
                                mm(s.0),
                                mm(s.1),
                                mm(s.2)
                            ),
                        )
                        .entity(part.id.clone())
                        .location(joint.to_string())
                        .suggestion(
                            "Alejá las patas del borde (inset) o adelantá el zócalo (setback).",
                        ),
                    );
                }
            }
            for (other, leg_b) in &legs[i + 1..] {
                if leg.intersection(leg_b).is_some() {
                    out.push(
                        Diagnostic::new(
                            self.id(),
                            Severity::Error,
                            format!("las patas {joint} y {other} se pisan"),
                        )
                        .location(joint.to_string())
                        .suggestion("Separá los módulos o alejá las patas del costado."),
                    );
                }
            }
        }
        out
    }
}

/// The box a leg fills: its base plate's square around the mounting point,
/// reaching `height` out of the face it is screwed to.
fn leg_envelope(
    centre: crate::geometry::Vec3,
    face: &crate::geometry::Aabb,
    leg: &crate::library::hardware::LegSpec,
) -> crate::geometry::Aabb {
    use crate::geometry::{Aabb, Vec3};
    let r = leg.base_diameter / 2.0;
    let mut min = [centre.0 - r, centre.1 - r, centre.2 - r];
    let mut max = [centre.0 + r, centre.1 + r, centre.2 + r];
    let c = [centre.0, centre.1, centre.2];
    let lo = [face.min.0, face.min.1, face.min.2];
    let hi = [face.max.0, face.max.1, face.max.2];
    // The axis the point sits flat on is the face normal; the leg grows
    // away from the part.
    let eps = 1e-6;
    if let Some(i) = (0..3).find(|i| (c[*i] - lo[*i]).abs() < eps) {
        min[i] = c[i] - leg.height;
        max[i] = c[i];
    } else if let Some(i) = (0..3).find(|i| (c[*i] - hi[*i]).abs() < eps) {
        min[i] = c[i];
        max[i] = c[i] + leg.height;
    }
    Aabb {
        min: Vec3(min[0], min[1], min[2]),
        max: Vec3(max[0], max[1], max[2]),
    }
}
