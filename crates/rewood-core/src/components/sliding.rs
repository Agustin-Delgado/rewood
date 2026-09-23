//! Sliding doors across a whole carcass, inside the opening: a double
//! track under the top and on the bottom, doors alternating between the
//! back lane and the front one and overlapping where they meet. The
//! dividers stop behind the track (`dividerSetback` on the carcass), and
//! shelves and inner drawers behind the back lane (the layout check says
//! so otherwise).
//!
//! Each door runs on two rollers at its bottom and is held by two guides
//! at its top, screwed to its back; the track is screwed to the top and
//! the bottom every so often and bought by the metre.

use super::{BuildCtx, ExtraBom, JointKind, OccupancyKind, PartInit, Zone};
use crate::diagnostics::{Diagnostic, Severity};
use crate::geometry::{Axis, Face, Placement, Vec3};
use crate::model::Grain;
use crate::rules::mm;
use crate::spec::{ComponentSpec, EdgeBanding, JointSpec};

/// Rollers and guides this far in from the door's side edges, and from its
/// bottom and top edges.
const FITTING_IN: f64 = 60.0;
const FITTING_UP: f64 = 20.0;

pub fn build(ctx: &mut BuildCtx<'_>, spec: &ComponentSpec) -> Result<(), Diagnostic> {
    let ComponentSpec::SlidingDoors {
        id,
        carcass,
        count,
        overlap,
        gap,
        material,
        track,
        screws,
        rollers,
        guides,
        edges,
        ..
    } = spec
    else {
        unreachable!()
    };
    let id = id.as_str();
    let carcass = ctx.carcass_for(id, carcass.as_ref())?;
    let track_def = track
        .iter()
        .find_map(|h| ctx.libs.hardware.get(h).and_then(|d| d.sliding.clone()))
        .ok_or_else(|| {
            Diagnostic::new(
                "SPEC-335",
                Severity::Fatal,
                format!("'{id}.track' tiene que nombrar un riel de corredizas (con su bloque 'sliding')"),
            )
            .entity(id)
        })?;
    let count = ctx.eval(id, "count", count)?;
    let lanes = f64::from(track_def.lanes);
    if count < 2.0 || count.fract() != 0.0 || count > 2.0 * lanes {
        return Err(Diagnostic::new(
            "SPEC-303",
            Severity::Fatal,
            format!(
                "'{id}.count' = {count}: van de 2 a {} puertas corredizas, enteras",
                2.0 * lanes
            ),
        )
        .entity(id));
    }
    let count = count as usize;
    let overlap = ctx.eval(id, "overlap", overlap)?;
    let gap = ctx.eval(id, "gap", gap)?;
    if overlap < 0.0 || gap < 0.0 {
        return Err(Diagnostic::new(
            "SPEC-304",
            Severity::Fatal,
            format!("'{id}': el solape y la luz no pueden ser negativos"),
        )
        .entity(id));
    }
    let material = ctx.material_or_default(id, material.as_ref())?.to_string();
    let t = ctx.thickness_of(&material);
    if t > track_def.lane_pitch - 2.0 {
        return Err(Diagnostic::new(
            "SPEC-335",
            Severity::Fatal,
            format!(
                "'{id}': puertas de {} mm no corren en carriles de {} mm",
                mm(t),
                mm(track_def.lane_pitch)
            ),
        )
        .entity(id));
    }
    // The track takes this much depth from the front; the dividers have to
    // stop behind it.
    let taken = track_def.front_inset + track_def.depth;
    if carcass.bays.len() > 1 && carcass.divider_setback < taken - crate::units::EPS {
        return Err(Diagnostic::new(
            "SPEC-335",
            Severity::Fatal,
            format!(
                "los divisores de '{}' llegan al frente y el riel de '{id}' ocupa {} mm desde ahí",
                carcass.id,
                mm(taken)
            ),
        )
        .entity(id)
        .location(carcass.id.clone())
        .suggestion("Retirá los divisores de la carcasa ('dividerSetback') detrás del riel.")
        .fix(
            format!("Divisores retirados {} mm", mm(taken)),
            carcass.id.clone(),
            "dividerSetback",
            serde_json::json!(taken),
        ));
    }

    // The opening, between the sides and between top and bottom.
    let ct = carcass.thickness;
    let (x0, x1) = (ct, carcass.width - ct);
    let opening = x1 - x0 - 2.0 * gap;
    let width = (opening + (count as f64 - 1.0) * overlap) / count as f64;
    let z0 = ct + track_def.bottom_clearance;
    let height = carcass.height - 2.0 * ct - track_def.bottom_clearance - track_def.top_clearance;
    if width <= overlap || height <= 0.0 {
        return Err(Diagnostic::new(
            "SPEC-305",
            Severity::Fatal,
            format!(
                "{count} puertas corredizas con {} mm de solape no entran en {} mm",
                mm(overlap),
                mm(opening)
            ),
        )
        .entity(id));
    }
    ctx.publish(id, "count", count as f64);
    ctx.publish(id, "door_width", width);
    ctx.publish(id, "door_height", height);
    ctx.publish(id, "track_depth", taken);

    // Door face of each lane: the front lane just inside the track's front.
    let lane_front = |lane: usize| {
        carcass.depth
            - track_def.front_inset
            - lane as f64 * track_def.lane_pitch
            - (track_def.lane_pitch - t) / 2.0
    };
    let back_lane = (track_def.lanes as usize - 1).min(1);
    let front_y = (lane_front(back_lane) - t, lane_front(0));
    ctx.occupy_front(
        id,
        OccupancyKind::Doors,
        &carcass,
        &carcass.bays,
        Zone {
            z0: 0.0,
            z1: carcass.height,
        },
        Some(front_y),
    );

    if width > 1200.0 || opening / count as f64 > 1200.0 {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-102",
                Severity::Warning,
                format!(
                    "'{id}': puertas corredizas de {} mm; más de 1200 se tuercen y pesan",
                    mm(width)
                ),
            )
            .entity(id)
            .suggestion("Más puertas.")
            .fix(
                format!("{} puertas", count + 1),
                id,
                "count",
                serde_json::json!(count + 1),
            ),
        );
    }
    if matches!(*edges, EdgeBanding::None | EdgeBanding::Front) {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-109",
                Severity::Warning,
                format!("'{id}': puertas sin canto; los bordes de placa quedan a la vista"),
            )
            .entity(id)
            .fix("Cantear las puertas", id, "edges", serde_json::Value::Null),
        );
    }
    // Two rollers carry the door.
    let per_roller = rollers
        .iter()
        .find_map(|h| ctx.libs.hardware.get(h)?.max_load_kg);
    let density = ctx
        .libs
        .materials
        .material(&material)
        .map_or(0.0, |m| m.density);
    let weight = width * height * t / 1e9 * density;
    if let Some(max) = per_roller {
        if weight > 2.0 * max {
            ctx.warn(
                Diagnostic::new(
                    "DESIGN-111",
                    Severity::Warning,
                    format!(
                        "'{id}': cada puerta pesa {} kg y sus dos ruedas cargan {} kg",
                        mm((weight * 10.0).round() / 10.0),
                        mm(2.0 * max)
                    ),
                )
                .entity(id)
                .suggestion("Más puertas (más angostas), o ruedas de más carga."),
            );
        }
    }

    let all_edges = super::banded_axes(*edges, &[Axis::PosX, Axis::NegX, Axis::PosZ, Axis::NegZ]);
    let fitting = |hardware: &[String]| JointSpec {
        hardware: hardware.to_vec(),
        placement: None,
    };
    for i in 0..count {
        let n = i + 1;
        // Alternate lanes, the first door in the back one.
        let lane = if i % 2 == 0 { back_lane } else { 0 };
        let x_left = x0 + gap + i as f64 * (width - overlap);
        let y = lane_front(lane);
        let door = ctx.add_part(PartInit {
            name: format!("Puerta corrediza {n}"),
            component: id,
            role: &format!("sliding_door_{n}"),
            material: &material,
            length: height,
            width,
            grain: Grain::Length,
            // Like a hinged door: local X up, local +Z = -Y (its back).
            placement: Placement::new(Vec3(x_left + width, y, z0), Axis::PosZ, Axis::NegX),
            banded_edges: &all_edges,
        });
        let back = y - t;
        for (hardware, z) in [
            (rollers, z0 + FITTING_UP),
            (guides, z0 + height - FITTING_UP),
        ] {
            for x in [x_left + FITTING_IN, x_left + width - FITTING_IN] {
                ctx.request(
                    JointKind::Fixture {
                        centre: Vec3(x, back, z),
                        face: Face::Front,
                        along: Axis::PosX,
                    },
                    id,
                    &door,
                    &door,
                    &fitting(hardware),
                );
            }
        }
    }

    // The track, screwed along its middle to the top's underside and the
    // bottom's upper face.
    let y_track = carcass.depth - track_def.front_inset - track_def.depth / 2.0;
    for (role, z, looks) in [
        ("top", carcass.height - ct, Axis::NegZ),
        ("bottom", ct, Axis::PosZ),
    ] {
        let Some(panel) = ctx
            .parts
            .iter()
            .find(|p| p.component == carcass.id && p.role == role)
            .map(|p| p.id.clone())
        else {
            continue;
        };
        let face = ctx.part_mut(&panel).face_facing(looks);
        ctx.request(
            JointKind::Row {
                from: Vec3(x0, y_track, z),
                to: Vec3(x1, y_track, z),
                face,
            },
            id,
            &panel,
            &panel,
            &fitting(screws),
        );
    }
    if let Some(h) = track.first() {
        ctx.extra_bom.push(ExtraBom {
            component: id.to_string(),
            hardware: h.clone(),
            quantity: 1,
            metres: (x1 - x0) / 1000.0,
        });
    }
    Ok(())
}
