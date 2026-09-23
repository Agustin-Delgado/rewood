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
                    // `drawer_1_` and not `drawer_1`, which is also drawer 10.
                    let own_prefix = format!("{prefix}_");
                    let own: f64 = input
                        .parts
                        .iter()
                        .filter(|p| {
                            p.component == side.component && p.role.starts_with(&own_prefix)
                        })
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
                                    drawer_label(prefix),
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

/// `bay2_drawer_3` → "3 (bahía 2)", `drawer_3` → "3".
fn drawer_label(prefix: &str) -> String {
    let (bay, drawer) = match prefix.split_once("_drawer_") {
        Some((bay, n)) => (Some(bay.trim_start_matches("bay")), n),
        None => (None, prefix.trim_start_matches("drawer_")),
    };
    match bay {
        Some(b) => format!("{drawer} (bahía {b})"),
        None => drawer.to_string(),
    }
}

fn kg(v: f64) -> String {
    mm((v * 10.0).round() / 10.0)
}

/// DESIGN-118: a door, swung open, runs into a drawer pulled out or into
/// another open door. The door is taken at 95° on its hinge line, the
/// drawer out by its slide's length; a warning, with the other hinge side
/// as the fix when the door hangs alone in its bay.
pub struct FrontsCollideOpen;

/// Beyond square, how far a door is taken open.
const OPEN_ANGLE_PAST_SQUARE: f64 = 5.0;

struct Opened<'a> {
    part: &'a crate::model::Part,
    swept: crate::geometry::Aabb,
    /// For a door: the side it hangs on (true = right) and whether it is
    /// alone in its bay (then the other side is a one-click fix).
    door: Option<(bool, bool)>,
}

/// Where a door hung on `panel` stands open at 95°: a slab its thickness
/// wide on the hinge side, leaning out past square, as deep as the door is
/// wide in front of it. And whether it hangs on its right (the carcass's
/// +X before any turn). Works on any carcass turn: the door's thin
/// horizontal axis is its facing, outwards away from the panel.
fn door_swept(
    door: &crate::model::Part,
    panel: &crate::model::Part,
) -> (bool, crate::geometry::Aabb) {
    use crate::geometry::{Aabb, Vec3};
    let a = door.aabb;
    let (dc, pc) = (a.center(), panel.aabb.center());
    let size = a.size();
    // Facing axis (thickness) and width axis, both horizontal.
    let (ti, wi) = if size.0 < size.1 { (0, 1) } else { (1, 0) };
    let (t, w) = (size.component(ti), size.component(wi));
    let out = if dc.component(ti) >= pc.component(ti) {
        1.0
    } else {
        -1.0
    };
    let hinge_hi = pc.component(wi) > dc.component(wi);
    let lean = w * OPEN_ANGLE_PAST_SQUARE.to_radians().sin();
    let (w0, w1) = if hinge_hi {
        (a.max.component(wi) - t, a.max.component(wi) + lean)
    } else {
        (a.min.component(wi) - lean, a.min.component(wi) + t)
    };
    let face = if out > 0.0 {
        a.max.component(ti)
    } else {
        a.min.component(ti)
    };
    let (t0, t1) = if out > 0.0 {
        (face, face + w)
    } else {
        (face - w, face)
    };
    let mut min = [0.0; 3];
    let mut max = [0.0; 3];
    (min[ti], max[ti], min[wi], max[wi]) = (t0, t1, w0, w1);
    (min[2], max[2]) = (a.min.2, a.max.2);
    // "Right" is the carcass's +X: the facing turned a quarter clockwise.
    let facing = if ti == 1 {
        Vec3(0.0, out, 0.0)
    } else {
        Vec3(out, 0.0, 0.0)
    };
    let right = Vec3(facing.1, -facing.0, 0.0);
    let hinge = (pc - dc).dot(right) > 0.0;
    (
        hinge,
        Aabb {
            min: Vec3(min[0], min[1], min[2]),
            max: Vec3(max[0], max[1], max[2]),
        },
    )
}

/// A drawer front pulled out by `travel`: along its facing, away from the
/// box behind it.
fn drawer_swept(
    front: &crate::model::Part,
    side: &crate::model::Part,
    travel: f64,
) -> crate::geometry::Aabb {
    use crate::geometry::Vec3;
    let a = front.aabb;
    let size = a.size();
    let ti = if size.0 < size.1 { 0 } else { 1 };
    let out = a.center().component(ti) >= side.aabb.center().component(ti);
    let mut min = [a.min.0, a.min.1, a.min.2];
    let mut max = [a.max.0, a.max.1, a.max.2];
    if out {
        min[ti] = max[ti];
        max[ti] += travel;
    } else {
        max[ti] = min[ti];
        min[ti] -= travel;
    }
    crate::geometry::Aabb {
        min: Vec3(min[0], min[1], min[2]),
        max: Vec3(max[0], max[1], max[2]),
    }
}

/// Room left between an open door and the inner drawer front beside it.
const SPACER_CLEARANCE: f64 = 3.0;

/// Inner drawers behind a door, on a slide screwed to the panel the door
/// hangs on, whose fronts come out where the door stands open: how far
/// each stack has to move off that panel (`components::spacer_key`).
pub(crate) fn spacer_needs(
    parts: &[crate::model::Part],
    joints: &[crate::model::Joint],
) -> std::collections::BTreeMap<String, f64> {
    let part = |id: &str| parts.iter().find(|p| p.id == id);
    let mut out = std::collections::BTreeMap::new();
    for h in joints
        .iter()
        .filter(|j| j.kind == "hinge" && j.axis.index() == 2)
    {
        let (Some(door), Some(panel)) = (part(&h.edge_part), part(&h.face_part)) else {
            continue;
        };
        let (right, swept) = door_swept(door, panel);
        let d = door.aabb;
        for s in joints
            .iter()
            .filter(|j| j.kind == "slide" && j.face_part == panel.id)
        {
            let Some(side) = part(&s.edge_part) else {
                continue;
            };
            let suffix = if right { "_side_right" } else { "_side_left" };
            let Some(prefix) = side.role.strip_suffix(suffix) else {
                continue;
            };
            let front_role = format!("{prefix}_front");
            let Some(front) = parts
                .iter()
                .find(|p| p.component == side.component && p.role == front_role)
            else {
                continue;
            };
            let f = front.aabb;
            // Behind the door, at its height, inside its width.
            let behind = f.max.1 <= d.min.1 + 1.0;
            let level = f.min.2 < d.max.2 - 1.0 && f.max.2 > d.min.2 + 1.0;
            let within = f.min.0 < d.max.0 - 1.0 && f.max.0 > d.min.0 + 1.0;
            if !(behind && level && within) {
                continue;
            }
            let need = if right {
                f.max.0 - (swept.min.0 - SPACER_CLEARANCE)
            } else {
                swept.max.0 + SPACER_CLEARANCE - f.min.0
            };
            if need > 0.0 {
                let key = crate::components::spacer_key(&side.component, panel);
                let e = out.entry(key).or_insert(0.0_f64);
                *e = e.max(need);
            }
        }
    }
    out
}

impl Rule for FrontsCollideOpen {
    fn id(&self) -> &'static str {
        "DESIGN-118"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let part = |id: &str| input.parts.iter().find(|p| p.id == id);
        let mut opened: Vec<Opened> = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for j in input.joints {
            match j.kind.as_str() {
                // Doors on a vertical hinge line.
                "hinge" if j.axis.index() == 2 => {
                    let (Some(door), Some(panel)) = (part(&j.edge_part), part(&j.face_part)) else {
                        continue;
                    };
                    if !seen.insert(door.id.clone()) {
                        continue;
                    }
                    let a = door.aabb;
                    let (right, swept) = door_swept(door, panel);
                    // Alone in its bay: no other door of the component
                    // shares its height and meets it side by side.
                    let wi = if a.size().0 < a.size().1 { 1 } else { 0 };
                    let alone = !input.parts.iter().any(|q| {
                        q.id != door.id
                            && q.component == door.component
                            && q.role.contains("door")
                            && (q.aabb.min.2 - a.min.2).abs() < 1.0
                            && ((q.aabb.min.component(wi) - a.max.component(wi)).abs() < 10.0
                                || (a.min.component(wi) - q.aabb.max.component(wi)).abs() < 10.0)
                    });
                    opened.push(Opened {
                        part: door,
                        swept,
                        door: Some((right, alone)),
                    });
                }
                // A drawer: its front, out by the slide's length.
                "slide" => {
                    let Some(side) = part(&j.edge_part) else {
                        continue;
                    };
                    let Some(prefix) = side.role.strip_suffix("_side_left") else {
                        continue;
                    };
                    let front_role = format!("{prefix}_front");
                    let Some(front) = input
                        .parts
                        .iter()
                        .find(|p| p.component == side.component && p.role == front_role)
                    else {
                        continue;
                    };
                    let travel = j
                        .hardware
                        .iter()
                        .find_map(|h| input.libs.hardware.get(h)?.slide.as_ref())
                        .map_or(0.0, |s| s.length);
                    opened.push(Opened {
                        part: front,
                        swept: drawer_swept(front, side, travel),
                        door: None,
                    });
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for (i, a) in opened.iter().enumerate() {
            let Some((right, alone)) = a.door else {
                continue;
            };
            // One finding per door, naming everything it runs into: other
            // fronts opened, and fronts that stay where they are (a fixed
            // front, or a closed front of a run at right angles).
            let mut hits: Vec<(&crate::model::Part, bool)> = opened
                .iter()
                .enumerate()
                .filter(|(k, b)| *k != i && a.swept.intersection(&b.swept).is_some())
                .map(|(_, b)| (b.part, true))
                .collect();
            for q in input.parts {
                let front = q.role.contains("door")
                    || q.role.contains("fixed_front")
                    || (q.role.ends_with("_front") && !q.role.ends_with("box_front"));
                if front
                    && q.id != a.part.id
                    && !hits.iter().any(|(h, _)| h.id == q.id)
                    && a.swept.intersection(&q.aabb).is_some()
                {
                    hits.push((q, false));
                }
            }
            if hits.is_empty() {
                continue;
            }
            let names: Vec<String> = hits
                .iter()
                .map(|(b, open)| {
                    format!(
                        "{} ({}){}",
                        b.id,
                        b.name,
                        if *open { " abierto" } else { "" }
                    )
                })
                .collect();
            let mut d = Diagnostic::new(
                self.id(),
                Severity::Warning,
                format!(
                    "al abrirse, {} ({}), con la bisagra a la {}, choca con {}",
                    a.part.id,
                    a.part.name,
                    if right { "derecha" } else { "izquierda" },
                    names.join(", "),
                ),
            )
            .entity(a.part.id.clone())
            .location(hits[0].0.id.clone())
            .suggestion("Colgá la puerta del otro lado (hingeSide) o separá los frentes.");
            if alone {
                let other = if right { "left" } else { "right" };
                d = d.fix(
                    format!(
                        "Bisagra a la {}",
                        if right { "izquierda" } else { "derecha" }
                    ),
                    a.part.component.clone(),
                    "hingeSide",
                    serde_json::json!(other),
                );
            }
            out.push(d);
        }
        out
    }
}

/// DESIGN-119: a door or drawer front of a cabinet that stands straight on
/// the floor, with less than `FLOOR_CLEARANCE` under it. The floor is
/// never flat: the front scrapes it (or the rug) when it opens, and a
/// drawer front that low has nothing to grip under it either. Legs with a
/// plinth are the fix. A wall-hung cabinet has no floor under it and is
/// left alone.
pub struct FrontOnFloor;

/// Least gap between an opening front and the floor.
const FLOOR_CLEARANCE: f64 = 10.0;

fn opens(role: &str) -> bool {
    !role.contains("fixed_front")
        && (role.contains("door")
            || role.contains("flap")
            || role.contains("mirror")
            || (role.ends_with("_front") && !role.ends_with("box_front")))
}

impl Rule for FrontOnFloor {
    fn id(&self) -> &'static str {
        "DESIGN-119"
    }

    fn check(&self, input: &RuleInput<'_>) -> Vec<Diagnostic> {
        let hardware =
            |j: &crate::model::Joint| j.hardware.first().and_then(|h| input.libs.hardware.get(h));
        if input
            .joints
            .iter()
            .any(|j| hardware(j).is_some_and(|h| h.kind == "hanger"))
        {
            return Vec::new();
        }
        // The floor: the lowest a part or a leg reaches.
        let mut floor = input
            .parts
            .iter()
            .map(|p| p.aabb.min.2)
            .fold(f64::INFINITY, f64::min);
        for j in input.joints.iter().filter(|j| j.kind == "fixture") {
            if let Some(leg) = hardware(j).and_then(|h| h.leg.as_ref()) {
                floor = floor.min(j.contact.center().2 - leg.height);
            }
        }
        // component → (lowest gap, part ids)
        let mut hits: BTreeMap<String, (f64, Vec<String>)> = BTreeMap::new();
        for part in input.parts.iter().filter(|p| opens(&p.role)) {
            let gap = part.aabb.min.2 - floor;
            if gap < FLOOR_CLEARANCE - EPS {
                let e = hits
                    .entry(part.component.clone())
                    .or_insert((f64::INFINITY, Vec::new()));
                e.0 = e.0.min(gap);
                e.1.push(part.id.clone());
            }
        }
        hits.into_iter()
            .map(|(component, (gap, ids))| {
                Diagnostic::new(
                    self.id(),
                    Severity::Warning,
                    format!(
                        "'{component}': {} a {} mm del piso ({}): roza al abrir, el piso nunca es plano; hacen falta {} mm",
                        if ids.len() == 1 { "el frente queda" } else { "los frentes quedan" },
                        mm(gap),
                        ids.join(", "),
                        mm(FLOOR_CLEARANCE)
                    ),
                )
                .entity(component)
                .suggestion("Poné patas con zócalo en la carcasa (legs con plinth), o colgala.")
            })
            .collect()
    }
}
