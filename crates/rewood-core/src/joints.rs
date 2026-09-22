//! Joint resolution: from "join these two parts with this hardware" to the
//! actual contact geometry, the fastener points along it, and the holes each
//! fastener needs on each part.
//!
//! Two kinds exist. A *butt* joint is found from the geometry: one part's
//! edge meets the other's large face. A *hinge* is declared by the doors
//! generator: the door is the edge part (its hinge edge is the joint line)
//! and the carcass side is the face part, even though they never touch.

use crate::components::{JointKind, JointRequest};
use crate::diagnostics::{Diagnostic, Diagnostics, Severity};
use crate::geometry::{Aabb, Axis, Face, Vec3};
use crate::library::{HoleLocation, HoleSpec, JointSide, Libraries};
use crate::model::{CountersinkGeom, Fastener, Joint, OpGeometry, OpSource, Operation, Part};

struct Contact {
    /// Joint rectangle in furniture space, flat along the contact normal.
    rect: Aabb,
    /// The part whose large face receives the other's edge (or, for a
    /// hinge, the carcass side that takes the mounting plate).
    face_part: usize,
    face_part_face: Face,
    /// The part whose edge meets the face part (or the door).
    edge_part: usize,
    edge_part_face: Face,
    /// World direction from the face part into the edge part; for a hinge,
    /// from the hinge edge into the door.
    into_edge_part: Axis,
    /// Direction the joint line runs along, from its start. Positive for
    /// butt joints and hinges; a slide runs from the front backwards.
    axis: Axis,
    /// For a hinge: the direction from the joint line towards the back of
    /// the carcass, along which the mounting plate sits inset.
    inset_dir: Axis,
    /// Face-to-face only: the in-plane axis across the joint, along which
    /// fasteners are laid out in rows.
    across: Option<usize>,
}

fn find_butt_contact(parts: &[Part], ia: usize, ib: usize) -> Result<Contact, &'static str> {
    let a = &parts[ia];
    let b = &parts[ib];
    for fa in Face::ALL {
        let na = a.placement.world_axis(fa.normal_local());
        let Some(rect) = a.aabb.contact(na, &b.aabb) else {
            continue;
        };
        let fb = b.face_facing(na.negate());
        let (face_part, face_part_face, edge_part, edge_part_face, into_edge_part) =
            match (fa.is_large(), fb.is_large()) {
                (true, false) => (ia, fa, ib, fb, na),
                (false, true) => (ib, fb, ia, fa, na.negate()),
                (true, true) => return Err("face_to_face"),
                (false, false) => return Err("edge_to_edge"),
            };
        // The joint line runs along the long side of the edge part's edge
        // face; the other in-plane extent is that part's thickness.
        let (u_axis, _) = edge_part_face.uv_axes();
        let axis = positive(parts[edge_part].placement.world_axis(u_axis));
        return Ok(Contact {
            rect,
            face_part,
            face_part_face,
            edge_part,
            edge_part_face,
            into_edge_part,
            axis,
            // Unused for butt joints; pointing away from the contact plane
            // keeps `face_inset` holes inside the face part if ever used.
            inset_dir: into_edge_part.negate(),
            across: None,
        });
    }
    Err("no_contact")
}

/// Two large faces pressed together. `a` is drilled through, `b` takes
/// the screws.
fn face_to_face_contact(parts: &[Part], ia: usize, ib: usize) -> Result<Contact, &'static str> {
    let a = &parts[ia];
    let b = &parts[ib];
    for fa in [Face::Front, Face::Back] {
        let na = a.placement.world_axis(fa.normal_local());
        let Some(rect) = a.aabb.contact(na, &b.aabb) else {
            continue;
        };
        let fb = b.face_facing(na.negate());
        if !fb.is_large() {
            return Err("not_face_to_face");
        }
        let size = rect.size();
        let mut in_plane: Vec<usize> = (0..3).filter(|i| *i != na.index()).collect();
        in_plane.sort_by(|x, y| size.component(*y).partial_cmp(&size.component(*x)).unwrap());
        let axis = [Axis::PosX, Axis::PosY, Axis::PosZ][in_plane[0]];
        return Ok(Contact {
            rect,
            face_part: ib,
            face_part_face: fb,
            edge_part: ia,
            edge_part_face: fa,
            into_edge_part: na.negate(),
            axis,
            inset_dir: na,
            across: Some(in_plane[1]),
        });
    }
    Err("no_contact")
}

/// A handle: a single point on the panel's `Front`, the handle running
/// along `along`. Holes hang off it with `offset_along`.
fn handle_contact(parts: &[Part], ip: usize, centre: Vec3, along: Axis) -> Contact {
    let p = &parts[ip];
    let front = p.placement.world_axis(Face::Front.normal_local());
    Contact {
        rect: Aabb {
            min: centre,
            max: centre,
        },
        face_part: ip,
        face_part_face: Face::Front,
        edge_part: ip,
        edge_part_face: Face::Front,
        into_edge_part: front.negate(),
        axis: along,
        inset_dir: front.negate(),
        across: None,
    }
}

/// A fixture on one face of one part: leg, plinth clip.
fn fixture_contact(parts: &[Part], ip: usize, centre: Vec3, face: Face, along: Axis) -> Contact {
    let p = &parts[ip];
    let normal = p.placement.world_axis(face.normal_local());
    // The other in-plane axis, for `offsetAcross`.
    let across = (0..3)
        .find(|i| *i != normal.index() && *i != along.index())
        .unwrap_or(2);
    Contact {
        rect: Aabb {
            min: centre,
            max: centre,
        },
        face_part: ip,
        face_part_face: face,
        edge_part: ip,
        edge_part_face: face,
        into_edge_part: normal.negate(),
        axis: along,
        inset_dir: normal.negate(),
        across: Some(across),
    }
}

/// A row of holes on one face of one part: the joint line is the segment
/// itself, every fastener one hole.
fn row_contact(parts: &[Part], ip: usize, from: Vec3, to: Vec3, face: Face) -> Contact {
    let p = &parts[ip];
    let normal = p.placement.world_axis(face.normal_local());
    let d = to - from;
    // A one-hole row (a shelf that does not move) has no direction.
    let axis = if d.length() < crate::units::EPS {
        Axis::PosZ
    } else {
        Axis::from_vec(d * (1.0 / d.length()))
    };
    Contact {
        rect: Aabb {
            min: Vec3(from.0.min(to.0), from.1.min(to.1), from.2.min(to.2)),
            max: Vec3(from.0.max(to.0), from.1.max(to.1), from.2.max(to.2)),
        },
        face_part: ip,
        face_part_face: face,
        edge_part: ip,
        edge_part_face: face,
        into_edge_part: normal.negate(),
        axis,
        inset_dir: normal.negate(),
        across: None,
    }
}

/// A hinge joint line: the door's hinge edge, on the door's back plane.
fn hinge_contact(parts: &[Part], door: usize, side: usize, hinge_edge: Axis) -> Contact {
    let d = &parts[door];
    let into_door = hinge_edge.negate();
    let x = if hinge_edge.sign() > 0.0 {
        d.aabb.max.component(hinge_edge.index())
    } else {
        d.aabb.min.component(hinge_edge.index())
    };
    // The door's back plane is the face looking at the carcass: local Front.
    let back_normal = d.placement.world_axis(Face::Front.normal_local());
    let y = if back_normal.sign() > 0.0 {
        d.aabb.max.component(back_normal.index())
    } else {
        d.aabb.min.component(back_normal.index())
    };
    let mut min = [d.aabb.min.0, d.aabb.min.1, d.aabb.min.2];
    let mut max = [d.aabb.max.0, d.aabb.max.1, d.aabb.max.2];
    min[hinge_edge.index()] = x;
    max[hinge_edge.index()] = x;
    min[back_normal.index()] = y;
    max[back_normal.index()] = y;
    let rect = Aabb {
        min: Vec3(min[0], min[1], min[2]),
        max: Vec3(max[0], max[1], max[2]),
    };
    // The remaining free axis is the joint line.
    let axis = [Axis::PosX, Axis::PosY, Axis::PosZ]
        .into_iter()
        .find(|a| a.index() != hinge_edge.index() && a.index() != back_normal.index())
        .unwrap();
    Contact {
        rect,
        face_part: side,
        face_part_face: parts[side].face_facing(into_door),
        edge_part: door,
        edge_part_face: d.face_facing(hinge_edge),
        into_edge_part: into_door,
        axis,
        inset_dir: back_normal,
        across: None,
    }
}

/// A slide joint line: from the front end of the drawer box side towards
/// the back, at the slide's hole height, in the plane between the two
/// parts (the exact plane does not matter: holes project onto each face).
fn slide_contact(
    parts: &[Part],
    box_side: usize,
    carcass_side: usize,
    hardware: &[String],
    libs: &Libraries,
) -> Option<Contact> {
    let slide = hardware
        .iter()
        .find_map(|id| libs.hardware.get(id).and_then(|h| h.slide.as_ref()))?;
    let b = &parts[box_side];
    let c = &parts[carcass_side];
    // The box side's outside face looks at the carcass side.
    let outward = if c.aabb.center().0 > b.aabb.center().0 {
        Axis::PosX
    } else {
        Axis::NegX
    };
    let x = if outward.sign() > 0.0 {
        b.aabb.max.0
    } else {
        b.aabb.min.0
    };
    let z = b.aabb.min.2 + slide.axis_from_box_bottom;
    let (y_front, y_back) = (b.aabb.max.1, b.aabb.max.1 - slide.length);
    Some(Contact {
        rect: Aabb {
            min: Vec3(x, y_back, z),
            max: Vec3(x, y_front, z),
        },
        face_part: carcass_side,
        face_part_face: c.face_facing(outward.negate()),
        edge_part: box_side,
        edge_part_face: b.face_facing(outward),
        into_edge_part: outward.negate(),
        axis: Axis::NegY,
        inset_dir: Axis::NegY,
        across: None,
    })
}

fn positive(a: Axis) -> Axis {
    if a.sign() > 0.0 {
        a
    } else {
        a.negate()
    }
}

/// Which part, which face and which point a hole spec lands on, for a
/// fastener at `point` on the joint line.
fn place_hole(
    contact: &Contact,
    point: Vec3,
    hole: &HoleSpec,
) -> Result<(usize, Face, Vec3), &'static str> {
    let mut along = contact.axis.vec() * hole.offset_along.unwrap_or(0.0);
    if let (Some(off), Some(i)) = (hole.offset_across, contact.across) {
        let mut v = [0.0; 3];
        v[i] = off;
        along = along + Vec3(v[0], v[1], v[2]);
    }
    let from_edge = hole.offset_from_edge.unwrap_or(0.0);
    Ok(match (hole.side, hole.location) {
        (JointSide::FacePart, HoleLocation::ContactFace) => {
            (contact.face_part, contact.face_part_face, point + along)
        }
        (JointSide::FacePart, HoleLocation::FaceInset) => (
            contact.face_part,
            contact.face_part_face,
            point + along + contact.inset_dir.vec() * from_edge,
        ),
        (JointSide::EdgePart, HoleLocation::Edge) => {
            (contact.edge_part, contact.edge_part_face, point + along)
        }
        // Face-to-face: the edge part's touching face, drilled through.
        (JointSide::EdgePart, HoleLocation::ContactFace) => {
            (contact.edge_part, contact.edge_part_face, point + along)
        }
        (JointSide::EdgePart, HoleLocation::FaceOffset) => (
            contact.edge_part,
            Face::Front,
            point + along + contact.into_edge_part.vec() * from_edge,
        ),
        _ => return Err("invalid_combination"),
    })
}

pub fn resolve(
    parts: &mut [Part],
    requests: &[JointRequest],
    libs: &Libraries,
    diags: &mut Diagnostics,
) -> Vec<Joint> {
    let mut joints = Vec::new();
    for req in requests {
        let joint_id = format!("J{:03}", joints.len() + 1);
        let Some(ia) = parts.iter().position(|p| p.id == req.part_a) else {
            continue;
        };
        let Some(ib) = parts.iter().position(|p| p.id == req.part_b) else {
            continue;
        };

        let contact = match req.kind {
            JointKind::Butt => match find_butt_contact(parts, ia, ib) {
                Ok(c) => c,
                Err(kind) => {
                    let (code, msg) = match kind {
                        "no_contact" => (
                            "JOINT-101",
                            "las piezas no se tocan, no hay dónde poner la unión",
                        ),
                        "face_to_face" => (
                            "JOINT-102",
                            "unión cara contra cara: no soportada en esta versión",
                        ),
                        _ => (
                            "JOINT-103",
                            "unión canto contra canto (inglete o tope): no soportada en esta versión",
                        ),
                    };
                    diags.push(
                        Diagnostic::new(
                            code,
                            Severity::Fatal,
                            format!("{} ↔ {}: {msg}", req.part_a, req.part_b),
                        )
                        .entity(joint_id.clone()),
                    );
                    continue;
                }
            },
            JointKind::Hinge { hinge_edge } => hinge_contact(parts, ia, ib, hinge_edge),
            JointKind::Handle { centre, along } => handle_contact(parts, ia, centre, along),
            JointKind::Fixture {
                centre,
                face,
                along,
            } => fixture_contact(parts, ia, centre, face, along),
            JointKind::Row { from, to, face } => row_contact(parts, ia, from, to, face),
            JointKind::FaceToFace => match face_to_face_contact(parts, ia, ib) {
                Ok(c) => c,
                Err(_) => {
                    diags.push(
                        Diagnostic::new(
                            "JOINT-101",
                            Severity::Fatal,
                            format!(
                                "{} ↔ {}: las caras no se tocan, no hay dónde atornillar",
                                req.part_a, req.part_b
                            ),
                        )
                        .entity(joint_id.clone()),
                    );
                    continue;
                }
            },
            JointKind::Slide => match slide_contact(parts, ia, ib, &req.hardware, libs) {
                Some(c) => c,
                None => {
                    diags.push(
                        Diagnostic::new(
                            "JOINT-105",
                            Severity::Fatal,
                            format!(
                                "{} ↔ {}: la corredera no tiene definición de longitud",
                                req.part_a, req.part_b
                            ),
                        )
                        .entity(joint_id.clone()),
                    );
                    continue;
                }
            },
        };

        let axis_index = contact.axis.index();
        let length = contact.rect.size().component(axis_index);
        let start = if contact.axis.sign() > 0.0 {
            contact.rect.min.component(axis_index)
        } else {
            contact.rect.max.component(axis_index)
        };
        let centre = contact.rect.center();

        let mut fasteners = Vec::new();
        for hw_id in &req.hardware {
            let Some(hw) = libs.hardware.get(hw_id) else {
                diags.push(
                    Diagnostic::new(
                        "LIB-102",
                        Severity::Fatal,
                        format!("herraje desconocido '{hw_id}'"),
                    )
                    .entity(joint_id.clone())
                    .suggestion("Declaralo en libraries.hardware o usá uno de la biblioteca."),
                );
                continue;
            };
            if hw.kind_matches(req.kind) {
                // fine
            } else {
                diags.push(
                    Diagnostic::new(
                        "JOINT-104",
                        Severity::Fatal,
                        format!(
                            "el herraje '{}' ({}) no sirve para una unión {}",
                            hw.name,
                            hw.kind,
                            req.kind.label()
                        ),
                    )
                    .entity(joint_id.clone()),
                );
                continue;
            }
            let placement = req.placement.as_ref().unwrap_or(&hw.placement);
            // Rows across the joint: one through the centre, or two inset
            // from the edges of a face-to-face contact when they fit.
            let rows: Vec<f64> = match contact.across {
                Some(i) => {
                    let (lo, hi) = (contact.rect.min.component(i), contact.rect.max.component(i));
                    if hi - lo > 2.0 * placement.end_offset + placement.end_offset {
                        vec![lo + placement.end_offset, hi - placement.end_offset]
                    } else {
                        vec![(lo + hi) / 2.0]
                    }
                }
                None => vec![0.0],
            };
            let mut along_positions = placement.positions(length);
            if let Some((reference, pitch)) = req.snap {
                // Nearest grid line, then clamped back inside the joint so
                // an end fastener never leaves its part.
                for pos in &mut along_positions {
                    let world = start + *pos * contact.axis.sign();
                    let snapped = reference + ((world - reference) / pitch).round() * pitch;
                    let back = (snapped - start) * contact.axis.sign();
                    *pos = back.clamp(0.0, length);
                }
                // On a short joint two fasteners can round to one grid
                // line: the later one takes the next line, or goes if
                // there is none left.
                along_positions.sort_by(f64::total_cmp);
                let mut spread: Vec<f64> = Vec::with_capacity(along_positions.len());
                for pos in along_positions {
                    let pos = match spread.last() {
                        Some(&prev) if pos < prev + pitch - crate::units::EPS => prev + pitch,
                        _ => pos,
                    };
                    if pos <= length + crate::units::EPS {
                        spread.push(pos);
                    }
                }
                along_positions = spread;
            }
            let grid = rows
                .iter()
                .flat_map(|row| along_positions.iter().map(move |offset| (*row, *offset)));
            for (index, (row, offset)) in grid.enumerate() {
                let mut p = [centre.0, centre.1, centre.2];
                p[axis_index] = start + offset * contact.axis.sign();
                if let Some(i) = contact.across {
                    p[i] = row;
                }
                let point = Vec3(p[0], p[1], p[2]);
                fasteners.push(Fastener {
                    hardware: hw_id.clone(),
                    index,
                    position: point,
                });

                for hole in &hw.holes {
                    let (part_index, face, at) = match place_hole(&contact, point, hole) {
                        Ok(p) => p,
                        Err(_) => {
                            diags.push(
                                Diagnostic::new(
                                    "LIB-103",
                                    Severity::Fatal,
                                    format!(
                                        "el herraje '{hw_id}' combina {:?} con {:?}, que no tiene sentido",
                                        hole.side, hole.location
                                    ),
                                )
                                .entity(joint_id.clone()),
                            );
                            continue;
                        }
                    };
                    let part = &mut parts[part_index];
                    let (u, v) = part.world_point_to_face_uv(face, at);
                    // A hole on a System 32 row is not drilled twice: a
                    // hinge plate's holes that land on the row are the
                    // row's holes. Only rows merge; two other fasteners at
                    // one spot stay a clash for FAB-205 to report. The
                    // existing hole keeps its source and grows to the
                    // deeper of the two.
                    let is_row =
                        |id: &str| libs.hardware.get(id).is_some_and(|h| h.kind == "pin_row");
                    let same = part.operations.iter_mut().find(|o| {
                        o.face == face
                            && (is_row(hw_id)
                                || o.source.as_ref().is_some_and(|s| is_row(&s.hardware)))
                            && matches!(&o.geometry, OpGeometry::Drill { u: ou, v: ov, diameter, .. }
                                if (ou - u).abs() < 0.05 && (ov - v).abs() < 0.05 && (diameter - hole.diameter).abs() < 0.05)
                    });
                    if let Some(existing) = same {
                        if let OpGeometry::Drill { depth, .. } = &mut existing.geometry {
                            if let (Some(d), Some(h)) = (depth.as_mut(), hole.depth) {
                                *d = d.max(h);
                            }
                        }
                        continue;
                    }
                    let op = Operation {
                        id: part.next_op_id(),
                        face,
                        geometry: OpGeometry::Drill {
                            u,
                            v,
                            diameter: hole.diameter,
                            depth: hole.depth,
                            through: hole.depth.is_none(),
                            countersink: hole.countersink.as_ref().map(|c| CountersinkGeom {
                                diameter: c.diameter,
                                depth: c.depth,
                            }),
                        },
                        source: Some(OpSource {
                            joint: joint_id.clone(),
                            hardware: hw_id.clone(),
                            fastener: index,
                            label: hole.label.clone(),
                        }),
                    };
                    part.operations.push(op);
                }
            }
        }

        joints.push(Joint {
            id: joint_id,
            kind: req.kind.label().to_string(),
            component: req.component.clone(),
            edge_part: parts[contact.edge_part].id.clone(),
            face_part: parts[contact.face_part].id.clone(),
            hardware: req.hardware.clone(),
            contact: contact.rect,
            axis: contact.axis,
            length,
            fasteners,
        });
    }
    joints
}
