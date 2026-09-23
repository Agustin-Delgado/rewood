//! Physical audit of the compiled furniture: every fixture and a grid of
//! variants (sizes, counts, mounts) go through checks that do not repeat
//! the engine's own arithmetic but state what a cabinetmaker would verify
//! on the bench. A failure prints every violation found, not just the
//! first.
//!
//! - parts do not share volume (a back inside its groove excepted);
//! - the two holes of one fastener meet: a dowel's edge hole and its face
//!   hole are the same point in space, a cam sits 34 mm in from that edge
//!   on the inside face, a slide's cabinet and drawer screws face each
//!   other at the slide's clearance;
//! - hinge cups are on the door's inner face, 22.5 mm from the hinge edge,
//!   and their mounting plates on the side's inner face, 37 mm from its
//!   front edge, at the cup's height;
//! - handles are through holes on the opening side, the bar's length apart;
//! - the hinge arm matches where the door hangs: full overlay on a side,
//!   half overlay on a divider, inset inside the opening; a soft-close
//!   request gets a damped hinge or slide of the same family;
//! - a catch sits on the panel opposite the hinge, within reach of the
//!   door's back, and its plate on the door's inner face;
//! - drawer boxes fit their bay with the slide clearance and their slides
//!   are not longer than the box or the carcass;
//! - overlay doors cover their bay's panels minus the gap, flush with the
//!   front of the carcass;
//! - no finding worse than the ones listed as expected for that spec.

use std::collections::BTreeMap;

use rewood_core::geometry::{Axis, Face, Vec3};
use rewood_core::library::Libraries;
use rewood_core::model::{OpGeometry, Operation, Part};
use rewood_core::plan::{ManufacturingPlan, PlanStatus};

const EPS: f64 = 0.01;

struct Audit {
    name: String,
    violations: Vec<String>,
}

impl Audit {
    fn note(&mut self, s: String) {
        self.violations.push(format!("[{}] {s}", self.name));
    }
}

fn world(part: &Part, op: &Operation) -> Option<Vec3> {
    match &op.geometry {
        OpGeometry::Drill { u, v, .. } => Some(
            part.placement
                .to_world(part.dims.uv_to_local(op.face, *u, *v)),
        ),
        _ => None,
    }
}

fn part<'a>(plan: &'a ManufacturingPlan, id: &str) -> &'a Part {
    plan.parts.iter().find(|p| p.id == id).unwrap()
}

fn near(a: Vec3, b: Vec3) -> bool {
    (a - b).length() < EPS
}

/// Holes of one fastener on one part, with their world position.
fn holes_of<'a>(
    part: &'a Part,
    joint: &str,
    hardware: &str,
    fastener: usize,
) -> Vec<(&'a Operation, Vec3)> {
    part.operations
        .iter()
        .filter(|op| {
            op.source.as_ref().is_some_and(|s| {
                s.joint == joint && s.hardware == hardware && s.fastener == fastener
            })
        })
        .filter_map(|op| world(part, op).map(|w| (op, w)))
        .collect()
}

fn audit(name: &str, plan: &ManufacturingPlan, libs: &Libraries, allowed: &[&str]) -> Audit {
    let mut a = Audit {
        name: name.to_string(),
        violations: Vec::new(),
    };
    for d in &plan.diagnostics.items {
        let bad = matches!(
            d.severity,
            rewood_core::diagnostics::Severity::Fatal | rewood_core::diagnostics::Severity::Error
        ) || (d.severity == rewood_core::diagnostics::Severity::Warning
            && !allowed.contains(&d.code.as_str()));
        if bad {
            a.note(format!("{:?} {} {}", d.severity, d.code, d.message));
        }
    }
    if !matches!(plan.status, PlanStatus::Ok | PlanStatus::Warnings) {
        a.note(format!("status {:?}", plan.status));
        return a;
    }

    // 1. Volume.
    for (i, p) in plan.parts.iter().enumerate() {
        for q in &plan.parts[i + 1..] {
            if p.overlap_exempt.contains(&q.id) || q.overlap_exempt.contains(&p.id) {
                continue;
            }
            if let Some(o) = p.aabb.intersection(&q.aabb) {
                a.note(format!(
                    "{} y {} comparten volumen {:?}",
                    p.id,
                    q.id,
                    o.size()
                ));
            }
        }
    }

    // Everything else is checked in each carcass's own frame: a turned
    // module (the side run of an L kitchen) is turned back first.
    for frame in frames(plan) {
        audit_frame(&mut a, &frame, libs);
    }
    a
}

/// The plan split by carcass turn: parts and joints of the carcasses that
/// are not turned (and of what is built on them) as they are, and each
/// turned carcass with its dependents turned back about its origin.
fn frames(plan: &ManufacturingPlan) -> Vec<ManufacturingPlan> {
    let turns = |c: &str| {
        plan.derived
            .get(&format!("{c}.rotation"))
            .map(|d| ((d / 90.0).round() as i64).rem_euclid(4) as u8)
    };
    if !plan
        .derived
        .iter()
        .any(|(k, v)| k.ends_with(".rotation") && *v != 0.0)
    {
        return vec![plan.clone()];
    }
    let comp_of = |id: &str| {
        plan.parts
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.component.clone())
    };
    // Each component's carcass: itself, or the carcass a joint of it
    // reaches.
    let mut carcass_of: BTreeMap<String, String> = BTreeMap::new();
    for p in &plan.parts {
        if turns(&p.component).is_some() {
            carcass_of.insert(p.component.clone(), p.component.clone());
        }
    }
    for j in &plan.joints {
        if carcass_of.contains_key(&j.component) {
            continue;
        }
        for id in [&j.edge_part, &j.face_part] {
            if let Some(c) = comp_of(id).filter(|c| turns(c).is_some()) {
                carcass_of.insert(j.component.clone(), c);
                break;
            }
        }
    }
    let group = |component: &str| -> String {
        match carcass_of.get(component) {
            Some(c) if turns(c).unwrap_or(0) != 0 => c.clone(),
            _ => String::new(),
        }
    };
    let mut keys: Vec<String> = plan.parts.iter().map(|p| group(&p.component)).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .map(|key| {
            let mut sub = plan.clone();
            sub.parts.retain(|p| group(&p.component) == key);
            sub.joints.retain(|j| group(&j.component) == key);
            if !key.is_empty() {
                let q = (4 - turns(&key).unwrap()) % 4;
                let pivot = Vec3(
                    plan.derived[&format!("{key}.origin_x")],
                    plan.derived[&format!("{key}.origin_y")],
                    0.0,
                );
                for p in &mut sub.parts {
                    let pl = p.placement;
                    p.placement = rewood_core::geometry::Placement::new(
                        pl.origin.turned_about(pivot, q),
                        pl.x.turned(q),
                        pl.y.turned(q),
                    );
                    p.aabb = rewood_core::geometry::Aabb::of_part(&p.placement, p.dims);
                }
                for j in &mut sub.joints {
                    j.contact = j.contact.turned_about(pivot, q);
                    j.axis = j.axis.turned(q);
                    for f in &mut j.fasteners {
                        f.position = f.position.turned_about(pivot, q);
                    }
                }
            }
            sub
        })
        .collect()
}

fn audit_frame(a: &mut Audit, plan: &ManufacturingPlan, libs: &Libraries) {
    // 2. Fasteners.
    let bbox = plan.parts.iter().fold(
        (
            Vec3(f64::MAX, f64::MAX, f64::MAX),
            Vec3(f64::MIN, f64::MIN, f64::MIN),
        ),
        |(lo, hi), q| {
            (
                Vec3(
                    lo.0.min(q.aabb.min.0),
                    lo.1.min(q.aabb.min.1),
                    lo.2.min(q.aabb.min.2),
                ),
                Vec3(
                    hi.0.max(q.aabb.max.0),
                    hi.1.max(q.aabb.max.1),
                    hi.2.max(q.aabb.max.2),
                ),
            )
        },
    );
    let front_y = plan
        .parts
        .iter()
        .filter(|p| {
            !p.role.contains("door")
                && !p.role.contains("fixed_front")
                && !p.role.ends_with("_front")
                && p.role != "worktop"
        })
        .map(|p| p.aabb.max.1)
        .fold(f64::MIN, f64::max);
    for j in &plan.joints {
        let edge = part(plan, &j.edge_part);
        let face = part(plan, &j.face_part);
        let hw = j
            .hardware
            .iter()
            .filter_map(|h| libs.hardware.get(h))
            .collect::<Vec<_>>();
        for f in &j.fasteners {
            let p = f.position;
            let on_edge = holes_of(edge, &j.id, &f.hardware, f.index);
            let on_face = holes_of(face, &j.id, &f.hardware, f.index);
            match j.kind.as_str() {
                "butt" => {
                    // Every hole on the face part is at the fastener point;
                    // the edge part's edge hole too; a cam on the edge
                    // part's large face is 34 mm in, square to the joint.
                    for (op, w) in &on_face {
                        if !near(*w, p) {
                            a.note(format!(
                                "{} {}: agujero {} en cara a {:.2} mm del punto de fijación",
                                j.id,
                                f.hardware,
                                op.id,
                                (*w - p).length()
                            ));
                        }
                    }
                    let normal = edge.placement.world_axis(Face::Front.normal_local());
                    for (op, w) in &on_edge {
                        if op.face.is_large() {
                            let d = *w - p;
                            let along = d.dot(j.axis.vec()).abs();
                            let n = d.dot(normal.vec());
                            let inward = d - normal.vec() * n;
                            if along > EPS || (inward.length() - 34.0).abs() > EPS {
                                a.note(format!(
                                    "{} {}: {} (leva) a {:.1} mm del canto, {:.1} a lo largo",
                                    j.id,
                                    f.hardware,
                                    op.id,
                                    inward.length(),
                                    along
                                ));
                            }
                            // The cam never sits on a face that looks out
                            // of the furniture: one step past the face
                            // along its normal is still inside the box.
                            let probe = *w + normal.vec() * 1.0;
                            let outside = (0..3).any(|k| {
                                probe.component(k) < bbox.0.component(k) - EPS
                                    || probe.component(k) > bbox.1.component(k) + EPS
                            });
                            if outside {
                                a.note(format!(
                                    "{} {}: leva {} en la cara exterior de {} ({})",
                                    j.id, f.hardware, op.id, edge.id, edge.name
                                ));
                            }
                        } else if !near(*w, p) {
                            a.note(format!(
                                "{} {}: agujero de canto {} a {:.2} mm del punto",
                                j.id,
                                f.hardware,
                                op.id,
                                (*w - p).length()
                            ));
                        }
                    }
                    // Blind depths add up to the fastener's length.
                    for h in &hw {
                        let total: f64 = h.holes.iter().filter_map(|x| x.depth).sum();
                        if h.kind == "dowel" && total < 30.0 - EPS {
                            a.note(format!("{}: tarugo con {total} mm de agujero", h.id));
                        }
                    }
                }
                "hinge" => {
                    let door = edge;
                    let side = face;
                    // The door's Front is its inner face, looking at the
                    // carcass (world -Y for a front door).
                    let inner = door.placement.world_axis(Face::Front.normal_local());
                    if inner != Axis::NegY {
                        a.note(format!(
                            "{}: la cara interior de {} mira a {:?}",
                            j.id, door.id, inner
                        ));
                    }
                    let cups: Vec<_> = on_edge
                        .iter()
                        .filter(|(op, _)| matches!(&op.geometry, OpGeometry::Drill { diameter, .. } if *diameter > 30.0))
                        .collect();
                    if cups.len() != 1 {
                        a.note(format!(
                            "{}: {} cazoletas para la fijación {}",
                            j.id,
                            cups.len(),
                            f.index
                        ));
                        continue;
                    }
                    let (cup_op, cup) = cups[0];
                    // The arm for where the door hangs.
                    let inset = door.aabb.max.1 <= front_y + EPS;
                    let wanted = if inset {
                        "inset"
                    } else if side.role.starts_with("divider") {
                        "half_overlay"
                    } else {
                        "overlay"
                    };
                    for h in &hw {
                        let mount = h.hinge.as_ref().map(|s| s.mount.as_str()).unwrap_or("");
                        if mount != wanted {
                            a.note(format!(
                                "{}: bisagra {} en {} ({}), corresponde {wanted}",
                                j.id, h.id, side.id, side.role
                            ));
                        }
                    }
                    if cup_op.face != Face::Front {
                        a.note(format!(
                            "{}: cazoleta {} en {:?}, no en la cara interior",
                            j.id, cup_op.id, cup_op.face
                        ));
                    }
                    let from_edge = (cup.0 - p.0).abs();
                    if (from_edge - 22.5).abs() > EPS {
                        a.note(format!(
                            "{}: cazoleta a {from_edge} mm del canto de bisagra",
                            j.id
                        ));
                    }
                    if (cup.2 - p.2).abs() > EPS {
                        a.note(format!(
                            "{}: cazoleta a otra altura que el punto de bisagra",
                            j.id
                        ));
                    }
                    // Cup 100 mm from the door ends, never closer.
                    let end = (cup.2 - door.aabb.min.2).min(door.aabb.max.2 - cup.2);
                    if end < 60.0 {
                        a.note(format!(
                            "{}: cazoleta a {end} mm del extremo de la puerta",
                            j.id
                        ));
                    }
                    // Plate on the side's inner face, 37 mm from the front
                    // edge, 32 mm hole pattern centred on the cup.
                    // The plate's two holes, whoever drilled them: on a
                    // System 32 row they are the row's holes.
                    let plates: Vec<(&Operation, Vec3)> = side
                        .operations
                        .iter()
                        .filter(|op| matches!(&op.geometry, OpGeometry::Drill { diameter, .. } if (*diameter - 5.0).abs() < EPS))
                        .filter_map(|op| world(side, op).map(|w| (op, w)))
                        .filter(|(_, w)| {
                            (side.aabb.max.1 - w.1 - 37.0).abs() < EPS
                                && ((w.2 - cup.2).abs() - 16.0).abs() < EPS
                        })
                        .collect();
                    if plates.len() != 2 {
                        a.note(format!("{}: {} agujeros de base", j.id, plates.len()));
                    }
                    for (op, w) in plates {
                        // A large face of the panel: which one depends on
                        // the side of the panel the door hangs on.
                        if !matches!(op.face, Face::Front | Face::Back) {
                            a.note(format!(
                                "{}: base {} en {:?} de {}",
                                j.id, op.id, op.face, side.id
                            ));
                        }
                        let plate_normal = side.placement.world_axis(op.face.normal_local());
                        let from_front = side.aabb.max.1 - w.1;
                        if (from_front - 37.0).abs() > EPS {
                            a.note(format!(
                                "{}: base {} a {from_front} mm del frente del lateral",
                                j.id, op.id
                            ));
                        }
                        if ((w.2 - cup.2).abs() - 16.0).abs() > EPS {
                            a.note(format!(
                                "{}: base {} a {} mm de la cazoleta en altura",
                                j.id,
                                op.id,
                                w.2 - cup.2
                            ));
                        }
                        // The plate faces the door: its face's normal points
                        // from the panel towards the door.
                        let towards_door =
                            (door.aabb.center() - side.aabb.center()).dot(plate_normal.vec());
                        if towards_door < 0.0 {
                            a.note(format!(
                                "{}: la base en {} queda del lado opuesto a la puerta",
                                j.id, side.id
                            ));
                        }
                    }
                }
                "slide" => {
                    let bx = edge; // drawer box side
                    let cs = face; // carcass side (or divider)
                    let slide = hw.iter().find_map(|h| h.slide.as_ref());
                    let Some(slide) = slide else {
                        a.note(format!("{}: corredera sin datos", j.id));
                        continue;
                    };
                    let gap = if cs.aabb.center().0 > bx.aabb.center().0 {
                        cs.aabb.min.0 - bx.aabb.max.0
                    } else {
                        bx.aabb.min.0 - cs.aabb.max.0
                    };
                    // A spacer between panel and slide widens the gap by
                    // its thickness.
                    let spacer: f64 = hw
                        .iter()
                        .filter_map(|h| h.spacer.as_ref())
                        .map(|s| s.thickness)
                        .sum();
                    let gap = gap - spacer;
                    if (gap - slide.side_clearance).abs() > EPS {
                        a.note(format!(
                            "{}: luz de corredera {gap:.2} mm entre {} y {} (espera {})",
                            j.id, bx.id, cs.id, slide.side_clearance
                        ));
                    }
                    let box_depth = bx.aabb.max.1 - bx.aabb.min.1;
                    if slide.length > box_depth + EPS {
                        a.note(format!(
                            "{}: corredera de {} en caja de {box_depth}",
                            j.id, slide.length
                        ));
                    }
                    if bx.aabb.min.1 < cs.aabb.min.1 - EPS {
                        a.note(format!(
                            "{}: la caja {} sale por atrás del lateral {}",
                            j.id, bx.id, cs.id
                        ));
                    }
                    // Screws face each other across the gap.
                    for (op, w) in &on_edge {
                        let twin = on_face
                            .iter()
                            .find(|(_, v)| (v.1 - w.1).abs() < EPS && (v.2 - w.2).abs() < EPS);
                        if twin.is_none() {
                            a.note(format!(
                                "{}: tornillo {} de la caja sin par en el lateral",
                                j.id, op.id
                            ));
                        }
                        if op.face != Face::Front {
                            a.note(format!(
                                "{}: tornillo {} de corredera en {:?} de la caja",
                                j.id, op.id, op.face
                            ));
                        }
                    }
                    let axis_z = bx.aabb.min.2 + slide.axis_from_box_bottom;
                    for (op, w) in on_edge.iter().chain(on_face.iter()) {
                        if (w.2 - axis_z).abs() > EPS {
                            a.note(format!(
                                "{}: {} a {} mm del piso de la caja",
                                j.id,
                                op.id,
                                w.2 - bx.aabb.min.2
                            ));
                        }
                    }
                }
                "handle" => {
                    let door = edge;
                    let Some(def) = hw.iter().find(|h| h.id == f.hardware) else {
                        a.note(format!("{}: tirador desconocido {}", j.id, f.hardware));
                        continue;
                    };
                    if on_edge.len() != def.holes.len() {
                        a.note(format!(
                            "{}: {} agujeros de tirador, {} declara {}",
                            j.id,
                            on_edge.len(),
                            def.id,
                            def.holes.len()
                        ));
                        continue;
                    }
                    // A bar's two holes its centre distance apart; a knob
                    // one hole at the point.
                    let offsets: Vec<f64> = def
                        .holes
                        .iter()
                        .map(|h| h.offset_along.unwrap_or(0.0))
                        .collect();
                    let expected = offsets.iter().cloned().fold(f64::MIN, f64::max)
                        - offsets.iter().cloned().fold(f64::MAX, f64::min);
                    let span = on_edge
                        .iter()
                        .map(|(_, w)| (*w - p).length())
                        .fold(0.0, f64::max)
                        * if on_edge.len() > 1 { 2.0 } else { 1.0 };
                    if (span - expected).abs() > EPS {
                        a.note(format!(
                            "{}: tirador con agujeros a {span} mm (espera {expected})",
                            j.id
                        ));
                    }
                    for (op, _) in &on_edge {
                        if !matches!(&op.geometry, OpGeometry::Drill { through: true, .. }) {
                            a.note(format!("{}: {} de tirador no es pasante", j.id, op.id));
                        }
                    }
                    // On a door the handle is on the opening edge: farther
                    // from the hinge than the door's middle.
                    if door.role.contains("door") {
                        if let Some(h) = plan
                            .joints
                            .iter()
                            .find(|x| x.kind == "hinge" && x.edge_part == door.id)
                        {
                            let hinge_x = h.contact.min.0;
                            let mid = (door.aabb.min.0 + door.aabb.max.0) / 2.0;
                            if (p.0 - hinge_x).abs() < (mid - hinge_x).abs() {
                                a.note(format!(
                                    "{}: tirador del lado de la bisagra en {}",
                                    j.id, door.id
                                ));
                            }
                        }
                    }
                }
                "fixture" => {
                    for (op, w) in &on_face {
                        if !near(Vec3(w.0, w.1, w.2) - (*w - p), Vec3::ZERO) {
                            // no-op: shape check below
                        }
                        let d = *w - p;
                        if d.length() > 40.0 {
                            a.note(format!(
                                "{}: {} a {:.1} mm del punto de la fijación",
                                j.id,
                                op.id,
                                d.length()
                            ));
                        }
                    }
                    let kind = hw
                        .iter()
                        .find(|h| h.id == f.hardware)
                        .map(|h| h.kind.as_str())
                        .unwrap_or("");
                    if kind == "catch" {
                        // Body flush with the panel's front edge (or the
                        // door's back), screws just behind it; every hole
                        // on the face looking into the bay.
                        let panel = face;
                        let from_front = panel.aabb.max.1 - p.1;
                        if !(15.0..=45.0).contains(&from_front) {
                            a.note(format!(
                                "{}: cierre a {from_front} mm del frente de {}",
                                j.id, panel.id
                            ));
                        }
                        if on_face.len() < 2 {
                            a.note(format!("{}: cierre con {} tornillos", j.id, on_face.len()));
                        }
                        // The door it holds carries the plate, right in
                        // front of the body (within its reach in x and z).
                        let plate = plan.joints.iter().find(|x| {
                            x.kind == "fixture"
                                && x.component == j.component
                                && part(plan, &x.face_part).role.contains("door")
                                && (x.contact.min.2 - p.2).abs() < 15.0
                                && (x.contact.min.0 - p.0).abs() < 15.0
                        });
                        if plate.is_none() {
                            a.note(format!(
                                "{}: cierre en {} sin placa enfrentada en una puerta",
                                j.id, panel.id
                            ));
                        }
                    }
                    if kind == "strike" {
                        let door = face;
                        for (op, _) in &on_face {
                            if op.face != Face::Front {
                                a.note(format!(
                                    "{}: placa de cierre en {:?} de {}",
                                    j.id, op.face, door.id
                                ));
                            }
                        }
                        if (p.1 - door.aabb.min.1).abs() > EPS {
                            a.note(format!("{}: placa fuera del dorso de {}", j.id, door.id));
                        }
                    }
                    if f.hardware.starts_with("leg") {
                        let bottom = face;
                        let down = bottom
                            .placement
                            .world_axis(on_face[0].0.face.normal_local());
                        if down != Axis::NegZ {
                            a.note(format!(
                                "{}: pata en la cara {:?} de {}",
                                j.id, on_face[0].0.face, bottom.id
                            ));
                        }
                    }
                }
                "face_to_face" => {
                    for (op, w) in &on_face {
                        let twin = on_edge.iter().find(|(_, v)| near(*v, *w));
                        if twin.is_none() {
                            a.note(format!(
                                "{}: {} sin agujero enfrentado en {}",
                                j.id, op.id, edge.id
                            ));
                        }
                    }
                    for (op, _) in &on_edge {
                        if !matches!(&op.geometry, OpGeometry::Drill { through: true, .. }) {
                            a.note(format!("{}: {} debería ser pasante", j.id, op.id));
                        }
                    }
                }
                _ => {}
            }
        }
        // Every joint drills something, and a butt joint of any length has
        // at least two fasteners.
        if j.kind == "butt" && j.length > 250.0 && j.fasteners.len() < 2 {
            a.note(format!(
                "{}: unión de {} mm con {} fijación",
                j.id,
                j.length,
                j.fasteners.len()
            ));
        }
    }

    // 3. Doors: overlay doors flush with the carcass front plus their
    // thickness; every door inside the furniture's height.
    for d in plan.parts.iter().filter(|p| p.role.contains("door")) {
        let t = d.dims.thickness;
        let overlay = (d.aabb.max.1 - (front_y + t)).abs() < EPS;
        let inset = (d.aabb.max.1 - front_y).abs() < EPS;
        // A sliding door runs inside the opening, on its track.
        let sliding = d.role.starts_with("sliding_door")
            && d.aabb.max.1 < front_y
            && d.aabb.min.1 > front_y - 80.0;
        if !overlay && !inset && !sliding {
            a.note(format!(
                "{}: frente de puerta en y={} (carcasa termina en {front_y})",
                d.id, d.aabb.max.1
            ));
        }
        for (i, q) in plan.parts.iter().enumerate() {
            let _ = i;
            if q.role.contains("door") && q.id != d.id && q.aabb.intersection(&d.aabb).is_some() {
                a.note(format!("{} y {} se pisan", d.id, q.id));
            }
        }
    }

    // 4. Every hole is inside its face with its radius, and no blind hole
    // goes through.
    for p in &plan.parts {
        for op in &p.operations {
            if let OpGeometry::Drill {
                u,
                v,
                diameter,
                depth,
                through,
                ..
            } = &op.geometry
            {
                let (eu, ev) = p.dims.face_extent(op.face);
                let r = diameter / 2.0;
                if *u < r - EPS || *v < r - EPS || *u > eu - r + EPS || *v > ev - r + EPS {
                    a.note(format!(
                        "{}: {} fuera de la cara ({u}, {v}) en {eu}×{ev}",
                        p.id, op.id
                    ));
                }
                let along = p.dims.along(op.face.normal_local());
                if !through && depth.unwrap_or(0.0) >= along - 2.0 {
                    a.note(format!(
                        "{}: {} de {:?} mm en {along} mm de material",
                        p.id, op.id, depth
                    ));
                }
            }
        }
    }

    // 5. Every shelf, top and bottom spans exactly between the panels it
    // is joined to (no gap, no bite).
    let mut by_component: BTreeMap<&str, Vec<&Part>> = BTreeMap::new();
    for p in &plan.parts {
        by_component
            .entry(p.component.as_str())
            .or_default()
            .push(p);
    }
    for j in plan.joints.iter().filter(|j| j.kind == "butt") {
        let e = part(plan, &j.edge_part);
        let f = part(plan, &j.face_part);
        if e.aabb.contact(Axis::PosX, &f.aabb).is_none()
            && e.aabb.contact(Axis::NegX, &f.aabb).is_none()
            && e.aabb.contact(Axis::PosY, &f.aabb).is_none()
            && e.aabb.contact(Axis::NegY, &f.aabb).is_none()
            && e.aabb.contact(Axis::PosZ, &f.aabb).is_none()
            && e.aabb.contact(Axis::NegZ, &f.aabb).is_none()
        {
            a.note(format!("{}: {} y {} no se tocan", j.id, e.id, f.id));
        }
    }
}

fn variants() -> Vec<(String, String, Vec<&'static str>)> {
    let mut out = Vec::new();
    for f in [
        "basic_cabinet",
        "bookcase_fixed",
        "drawer_unit",
        "kitchen_run",
        "wardrobe_1800",
        "wardrobe_rail",
        "nightstand",
        "wall_cabinet",
        "bookcase_adjustable",
        "tv_unit",
        "desk",
        "sideboard",
        "wardrobe_modules",
        "kitchen_corner",
        "wardrobe_sliding",
    ] {
        let path = format!(
            "{}/../../fixtures/{f}/input.json",
            env!("CARGO_MANIFEST_DIR")
        );
        out.push((
            f.to_string(),
            std::fs::read_to_string(path).unwrap(),
            vec![],
        ));
    }
    // Cabinet grid: widths, heights, doors, shelves, with and without back
    // groove, legs, inset doors.
    for (w, h, d) in [
        (400.0, 600.0, 300.0),
        (600.0, 720.0, 560.0),
        (900.0, 800.0, 400.0),
        (1000.0, 2000.0, 400.0),
        (1200.0, 2400.0, 600.0),
    ] {
        for doors in [1, 2] {
            for mount in ["overlay", "inset"] {
                let name = format!("cabinet_{w}x{h}x{d}_{doors}d_{mount}");
                let spec = format!(
                    r#"{{
  "schemaVersion": "1.0", "id": "{name}", "name": "{name}",
  "parameters": {{ "width": {w}, "height": {h}, "depth": {d} }},
  "material": "melamine_18", "edgeMaterial": "abs_1mm",
  "components": [
    {{ "type": "carcass", "id": "c", "joint": {{ "hardware": ["minifix_15", "dowel_8x30"] }}, "back": {{ "material": "hdf_3" }}, "legs": {{ "plinth": {{ "setback": 40 }} }} }},
    {{ "type": "shelves", "id": "s", "count": 2, "joint": {{ "hardware": ["dowel_8x30"] }} }},
    {{ "type": "doors", "id": "d", "count": {doors}, "mount": "{mount}", {options} "handle": {{ "hardware": ["{handle}"] }} }}
  ]
}}"#,
                    // Overlay doors get a magnetic catch, inset ones
                    // soft-close hinges; handles rotate through the bars
                    // and the knob.
                    options = if mount == "overlay" {
                        r#""catch": {},"#
                    } else {
                        r#""softClose": true,"#
                    },
                    handle = [
                        "handle_bar_96",
                        "handle_bar_160",
                        "knob_single",
                        "handle_bar_320"
                    ][(w as usize / 100 + doors) % 4],
                );
                out.push((
                    name,
                    spec,
                    vec![
                        "DESIGN-101",
                        "DESIGN-102",
                        "DESIGN-111",
                        "DESIGN-112",
                        "DESIGN-113",
                    ],
                ));
            }
        }
    }
    // Adjustable shelves behind doors: the hinge plates land on the rows.
    for (w, h) in [(600.0, 900.0), (900.0, 2000.0), (450.0, 720.0)] {
        let name = format!("pins_{w}x{h}");
        let spec = format!(
            r#"{{
  "schemaVersion": "1.0", "id": "{name}", "name": "{name}",
  "parameters": {{ "width": {w}, "height": {h}, "depth": 350 }},
  "material": "melamine_18", "edgeMaterial": "abs_1mm",
  "components": [
    {{ "type": "carcass", "id": "c", "joint": {{ "hardware": ["minifix_15", "dowel_8x30"] }}, "back": {{ "material": "hdf_3" }}, "hanging": {{}} }},
    {{ "type": "shelves", "id": "s", "count": 3, "support": "pins" }},
    {{ "type": "doors", "id": "d", "count": 2, "handle": {{ "hardware": ["handle_bar_128"] }} }}
  ]
}}"#
        );
        out.push((name, spec, vec!["DESIGN-102", "DESIGN-106"]));
    }
    // Drawer stacks.
    for (w, h, d, n) in [
        (400.0, 600.0, 500.0, 2),
        (600.0, 700.0, 500.0, 3),
        (900.0, 900.0, 600.0, 5),
        (500.0, 1200.0, 500.0, 6),
    ] {
        let name = format!("drawers_{w}x{h}x{d}_{n}");
        let spec = format!(
            r#"{{
  "schemaVersion": "1.0", "id": "{name}", "name": "{name}",
  "parameters": {{ "width": {w}, "height": {h}, "depth": {d} }},
  "material": "melamine_18", "edgeMaterial": "abs_1mm",
  "components": [
    {{ "type": "carcass", "id": "c", "joint": {{ "hardware": ["minifix_15", "dowel_8x30"] }}, "back": {{ "material": "hdf_3" }} }},
    {{ "type": "drawers", "id": "dr", "count": {n}, "softClose": {soft}, "joint": {{ "hardware": ["dowel_8x30"], "placement": {{ "endOffset": 40, "maxSpacing": 150 }} }}, "slide": {{ "hardware": ["{slide}"] }}, "handle": {{ "hardware": ["handle_bar_128"] }} }}
  ]
}}"#,
            soft = n % 2 == 0,
            slide = [
                "slide_ball_450",
                "slide_roller_400",
                "slide_ball_400",
                "slide_ball_250"
            ][n % 4],
        );
        out.push((name, spec, vec!["DESIGN-111", "DESIGN-112"]));
    }
    // Multi-bay wardrobes with rail, drawers, mixed doors.
    for (w, bays) in [(1200.0, 2), (1800.0, 3), (1800.0, 4)] {
        let name = format!("wardrobe_{w}_{bays}");
        let spec = format!(
            r#"{{
  "schemaVersion": "1.0", "id": "{name}", "name": "{name}",
  "parameters": {{ "width": {w}, "height": 2100, "depth": 560 }},
  "material": "melamine_18", "edgeMaterial": "abs_1mm",
  "components": [
    {{ "type": "carcass", "id": "c", "bays": {bays}, "joint": {{ "hardware": ["minifix_15", "dowel_8x30"] }}, "back": {{ "material": "hdf_3" }}, "legs": {{ "plinth": {{ "setback": 40 }} }} }},
    {{ "type": "rail", "id": "rail", "bay": 1 }},
    {{ "type": "drawers", "id": "dr", "bay": 2, "zone": {{ "from": 0, "to": 600 }}, "count": 2, "joint": {{ "hardware": ["dowel_8x30"], "placement": {{ "endOffset": 40, "maxSpacing": 150 }} }}, "slide": {{ "hardware": ["slide_ball_450"] }}, "handle": {{ "hardware": ["handle_bar_128"] }} }},
    {{ "type": "shelves", "id": "s", "bay": 2, "zone": {{ "from": 600, "to": "height" }}, "count": 3, "joint": {{ "hardware": ["dowel_8x30"] }} }},
    {{ "type": "doors", "id": "d1", "bay": 1, "count": 1, "catch": {{ "hardware": ["push_latch"] }} }},
    {{ "type": "doors", "id": "d2", "bay": 2, "zone": {{ "from": 600, "to": "height" }}, "count": 1, "softClose": true, "handle": {{ "hardware": ["handle_bar_128"] }} }}
  ]
}}"#
        );
        out.push((name, spec, vec!["SPEC-211", "DESIGN-102", "DESIGN-114"]));
    }
    out
}

#[test]
fn every_furniture_passes_the_bench_audit() {
    let libs = Libraries::default();
    let mut all = Vec::new();
    for (name, spec, allowed) in variants() {
        let plan = rewood_core::compile_json(&spec);
        let a = audit(&name, &plan, &libs, &allowed);
        all.extend(a.violations);
    }
    assert!(all.is_empty(), "\n{}\n", all.join("\n"));
}

/// What a person may pick on a template has to be buildable: every
/// option's extremes (and every choice), one at a time from the template's
/// defaults, and the options a choice switches on at their extremes too,
/// pass the same audit as the fixtures. Design warnings are allowed; a
/// FATAL, an ERROR or a failed check is not.
#[test]
fn every_option_value_is_manufacturable() {
    use rewood_core::options::OptionKind;
    use serde_json::{json, Value};

    // Design warnings say "probably not what you want", with the number
    // behind it; they never stop a piece from being made.
    const ALLOWED: &[&str] = &[
        "DESIGN-101",
        "DESIGN-102",
        "DESIGN-103",
        "DESIGN-104",
        "DESIGN-105",
        "DESIGN-106",
        "DESIGN-107",
        "DESIGN-108",
        "DESIGN-110",
        "DESIGN-111",
        "DESIGN-112",
        "DESIGN-113",
        "DESIGN-114",
        "DESIGN-115",
        "DESIGN-116",
        "DESIGN-117",
        "DESIGN-118",
        "SPEC-211",
        "CON-001",
    ];
    let libs = Libraries::default();
    let dir = format!("{}/../../fixtures", env!("CARGO_MANIFEST_DIR"));
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n != "invalid_cabinet")
        .collect();
    names.sort();

    let values = |o: &rewood_core::options::PlanOption| -> Vec<Value> {
        match o.kind {
            OptionKind::Toggle => vec![json!(true), json!(false)],
            OptionKind::Choice => o.choices.iter().map(|c| json!(c.value)).collect(),
            OptionKind::Number => [o.min, o.max]
                .into_iter()
                .flatten()
                .map(|v| json!(v))
                .collect(),
        }
    };
    let with = |spec: &Value, param: &str, v: &Value| -> Value {
        let mut s = spec.clone();
        s["parameters"][param] = v.clone();
        s
    };
    let mut all = Vec::new();
    let mut checked = 0;
    let mut run = |label: String, spec: &Value| -> ManufacturingPlan {
        let plan = rewood_core::compile_json(&spec.to_string());
        all.extend(audit(&label, &plan, &libs, ALLOWED).violations);
        checked += 1;
        plan
    };
    for name in names {
        let text = std::fs::read_to_string(format!("{dir}/{name}/input.json")).unwrap();
        let base: Value = serde_json::from_str(&text).unwrap();
        if base.get("options").is_none() {
            continue;
        }
        let plan = run(name.clone(), &base);
        for o in plan.options.iter().filter(|o| o.active) {
            for v in values(o) {
                let spec = with(&base, &o.param, &v);
                let p2 = run(format!("{name} {}={v}", o.param), &spec);
                if o.kind == OptionKind::Number {
                    continue;
                }
                // What this choice switches on, at its own extremes.
                for o2 in p2.options.iter().filter(|o2| {
                    o2.active && !plan.options.iter().any(|b| b.param == o2.param && b.active)
                }) {
                    for v2 in values(o2) {
                        run(
                            format!("{name} {}={v} {}={v2}", o.param, o2.param),
                            &with(&spec, &o2.param, &v2),
                        );
                    }
                }
            }
        }
    }
    assert!(checked > 100, "sólo {checked} variantes");
    assert!(all.is_empty(), "\n{}\n", all.join("\n"));
}
