//! Holes drilled into both faces of a divider at the same spot run into
//! each other: a shelf on each side at the same height, two drawer stacks
//! sharing a divider, a slide's screw facing a hinge plate. Before the
//! rules look, one of the two fasteners moves along its joint line until
//! it clears everything, as a joiner would:
//!
//! - a dowel, a cam bolt or a screw holding a front to its box by 16 mm
//!   steps;
//! - a slide's screw pair (cabinet and drawer member, facing each
//!   other) to the next holes of the rails, 32 mm on;
//! - a hinge, cup and plate together, one System 32 pitch.
//!
//! Shelf-pin rows do not move. Hole ids stay; only positions change, and
//! the rules still see whatever could not be cleared.

use std::collections::BTreeSet;

use crate::geometry::Vec3;
use crate::library::Libraries;
use crate::model::{Joint, OpGeometry, Part};
use crate::rules::machining::{cylinders, segment_distance, Cylinder};
use crate::units::EPS;

/// Offsets tried, in order: the nearest free spot wins (the end
/// fasteners sit at the minimum distance from the ends already, and the
/// margin check forbids going out).
const STEPS: [f64; 10] = [
    16.0, -16.0, 32.0, -32.0, 48.0, -48.0, 64.0, -64.0, 96.0, -96.0,
];
/// On the System 32 grid: slide rail holes, hinge positions.
const PITCH_STEPS: [f64; 4] = [32.0, -32.0, 64.0, -64.0];
/// Enough for any real layout; a bound so a pathological one cannot spin.
const MAX_MOVES: usize = 500;

/// A fastener: joint id, hardware, index within that hardware.
type Group = (String, String, usize);
/// Operation index and its (u, v) before a tentative move.
type Saved = Vec<(usize, f64, f64)>;

pub fn stagger(parts: &mut [Part], joints: &mut [Joint], libs: &Libraries) {
    let mut stuck: BTreeSet<Group> = BTreeSet::new();
    for _ in 0..MAX_MOVES {
        let Some(group) = next_movable(parts, joints, libs, &stuck) else {
            return;
        };
        if !try_move(parts, joints, libs, &group) {
            stuck.insert(group);
        }
    }
}

fn thickness(part: &Part, libs: &Libraries) -> f64 {
    libs.materials
        .material(&part.material)
        .map(|m| m.actual_thickness)
        .unwrap_or(part.dims.thickness)
}

fn clash(a: &Cylinder, b: &Cylinder, spacing: f64) -> bool {
    segment_distance(a.start, a.end, b.start, b.end) < a.radius + b.radius + spacing - EPS
}

/// How a fastener of this joint may move, by preference: dowels first,
/// then a slide screw, a hinge last. `None` = it stays.
fn freedom(joints: &[Joint], id: &str) -> Option<(u8, &'static [f64])> {
    match joints.iter().find(|j| j.id == id)?.kind.as_str() {
        "butt" | "face_to_face" => Some((0, &STEPS)),
        "slide" => Some((1, &PITCH_STEPS)),
        "hinge" => Some((2, &PITCH_STEPS)),
        _ => None,
    }
}

/// The first collision (parts in order, holes in order) with a fastener
/// that may move: of the two, the freer one, a dowel before a cam or a
/// screw, and of equals the one from the later joint.
fn next_movable(
    parts: &[Part],
    joints: &[Joint],
    libs: &Libraries,
    stuck: &BTreeSet<Group>,
) -> Option<Group> {
    let spacing = libs.profile.min_hole_spacing;
    for part in parts {
        let cyls = cylinders(part, thickness(part, libs));
        for i in 0..cyls.len() {
            for j in (i + 1)..cyls.len() {
                let (a, b) = (&cyls[i], &cyls[j]);
                let (Some(ga), Some(gb)) = (&a.group, &b.group) else {
                    continue;
                };
                // Two fasteners of one hardware in one joint are its own
                // layout; anything else meeting on a panel (two joints on
                // the faces of a divider, or two library rules landing a
                // dowel on a cam) is the engine's to sort out.
                if (ga.0 == gb.0 && ga.1 == gb.1) || !clash(a, b, spacing) {
                    continue;
                }
                let not_dowel =
                    |g: &Group| libs.hardware.get(&g.1).is_none_or(|h| h.kind != "dowel");
                let mut candidates: Vec<(u8, bool, &Group)> = [ga, gb]
                    .into_iter()
                    .filter(|g| !stuck.contains(*g))
                    .filter_map(|g| freedom(joints, &g.0).map(|(rank, _)| (rank, not_dowel(g), g)))
                    .collect();
                candidates
                    .sort_by(|x, y| x.0.cmp(&y.0).then(x.1.cmp(&y.1)).then(y.2 .0.cmp(&x.2 .0)));
                if let Some((_, _, g)) = candidates.first() {
                    return Some((*g).clone());
                }
            }
        }
    }
    None
}

/// Move one fastener to the first offset that keeps it inside its joint
/// and clear of every other hole in the parts it drills. Both parts'
/// holes move together.
fn try_move(parts: &mut [Part], joints: &mut [Joint], libs: &Libraries, group: &Group) -> bool {
    let Some(ji) = joints.iter().position(|j| j.id == group.0) else {
        return false;
    };
    let Some((_, steps)) = freedom(joints, &group.0) else {
        return false;
    };
    let joint = &joints[ji];
    let Some(fi) = joint
        .fasteners
        .iter()
        .position(|f| f.hardware == group.1 && f.index == group.2)
    else {
        return false;
    };
    let axis = joint.axis.vec();
    let k = joint.axis.index();
    let along = |p: Vec3| [p.0, p.1, p.2][k];
    let (lo, hi) = (
        along(joint.contact.min).min(along(joint.contact.max)),
        along(joint.contact.min).max(along(joint.contact.max)),
    );
    // Never closer to an end than the joint's own closest fastener.
    let margin = joint
        .fasteners
        .iter()
        .map(|f| (along(f.position) - lo).min(hi - along(f.position)))
        .fold(f64::INFINITY, f64::min)
        .max(0.0);
    let here = along(joint.fasteners[fi].position);
    let touched: Vec<usize> = parts
        .iter()
        .enumerate()
        .filter(|(_, p)| p.id == joint.edge_part || p.id == joint.face_part)
        .map(|(i, _)| i)
        .collect();
    let spacing = libs.profile.min_hole_spacing;
    let mine = |c: &Cylinder| c.group.as_ref() == Some(group);

    for &step in steps {
        let target = here + step * joint.axis.sign();
        if target < lo + margin - EPS || target > hi - margin + EPS {
            continue;
        }
        let d = axis * step;
        let saved: Vec<(usize, Saved)> = touched
            .iter()
            .map(|&pi| (pi, shift(&mut parts[pi], group, d)))
            .collect();
        let clear = touched.iter().all(|&pi| {
            let part = &parts[pi];
            let cyls = cylinders(part, thickness(part, libs));
            cyls.iter().filter(|c| mine(c)).all(|m| {
                cyls.iter()
                    .filter(|c| c.group != m.group)
                    .all(|o| !clash(m, o, spacing))
            }) && inside(part, group)
        });
        if clear {
            let f = &mut joints[ji].fasteners[fi];
            f.position = f.position + d;
            return true;
        }
        for (pi, ops) in saved {
            for (oi, u, v) in ops {
                if let OpGeometry::Drill { u: ou, v: ov, .. } =
                    &mut parts[pi].operations[oi].geometry
                {
                    *ou = u;
                    *ov = v;
                }
            }
        }
    }
    false
}

/// Move a fastener's holes in one part by a world vector; returns the
/// previous (u, v) of each moved operation.
fn shift(part: &mut Part, group: &Group, d: Vec3) -> Saved {
    let o = part.placement.origin;
    let mut saved = Vec::new();
    for oi in 0..part.operations.len() {
        let op = &part.operations[oi];
        let ours = op
            .source
            .as_ref()
            .is_some_and(|s| s.joint == group.0 && s.hardware == group.1 && s.fastener == group.2);
        if !ours {
            continue;
        }
        let face = op.face;
        let (u0, v0) = part.world_point_to_face_uv(face, o);
        let (u1, v1) = part.world_point_to_face_uv(face, o + d);
        if let OpGeometry::Drill { u, v, .. } = &mut part.operations[oi].geometry {
            saved.push((oi, *u, *v));
            *u += u1 - u0;
            *v += v1 - v0;
        }
    }
    saved
}

/// The fastener's hole centres are still on their faces.
fn inside(part: &Part, group: &Group) -> bool {
    part.operations.iter().all(|op| {
        let ours = op
            .source
            .as_ref()
            .is_some_and(|s| s.joint == group.0 && s.hardware == group.1 && s.fastener == group.2);
        match (&op.geometry, ours) {
            (OpGeometry::Drill { u, v, .. }, true) => {
                let (lu, lv) = part.dims.face_extent(op.face);
                *u >= -EPS && *v >= -EPS && *u <= lu + EPS && *v <= lv + EPS
            }
            _ => true,
        }
    })
}
