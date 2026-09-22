//! Overlay doors across the front of a carcass bay. Local +Z (the `Front`
//! face) looks at the carcass: that is where hinge cups are bored.
//!
//! With one door per bay the hinge side is the bay's left panel; with two,
//! each door hangs on its outer panel. More than two per bay would need
//! something to hang the inner ones on.
//!
//! A door over a side overlays the whole side (minus the gap); a door next
//! to a divider overlays half of it, so two neighbouring doors meet in the
//! middle of the divider with one gap between them. That decides the
//! hinge arm: full overlay on a side, half overlay on a divider, inset
//! inside the opening; the spec's hinge only names the family.
//!
//! A catch (magnetic, push-open) goes on the panel opposite the hinge,
//! on its inner face, flush with the door's back at the door's mid
//! height, and its plate on the door's back in front of it. Two doors in
//! one bay meet in the middle, away from any panel: their catches go
//! under the top (or on the bottom) at each door's opening edge.

use super::{BuildCtx, JointKind, OccupancyKind, PartInit};
use crate::diagnostics::{Diagnostic, Severity};
use crate::geometry::{Axis, Face, Placement, Vec3};
use crate::model::Grain;
use crate::rules::mm;
use crate::spec::{ComponentSpec, EdgeBanding, FrontMount};

/// Catch body centre behind the door's back plane: its striking face is
/// flush with the panel's front edge (or the door's back), the screws
/// behind it.
const CATCH_SETBACK: f64 = 22.0;
/// The door plate sits just inside the panel's inner face, where the
/// catch body reaches.
const STRIKE_INSET: f64 = 8.0;
/// Under the top: catch this far in from the door's opening edge, plate
/// this far below the top's underside.
const CATCH_FROM_EDGE: f64 = 25.0;
const STRIKE_DROP: f64 = 10.0;

pub fn build(ctx: &mut BuildCtx<'_>, spec: &ComponentSpec) -> Result<(), Diagnostic> {
    let ComponentSpec::Doors {
        id,
        carcass,
        bay,
        last_bay,
        span,
        zone,
        count,
        gap,
        mount,
        material,
        hinge,
        soft_close,
        catch,
        handle,
        edges,
        ..
    } = spec
    else {
        unreachable!()
    };
    let id = id.as_str();

    let carcass = ctx.carcass_for(id, carcass.as_ref())?;
    let bays = ctx.bays_for(id, &carcass, bay.as_ref(), last_bay.as_ref())?;
    let bays = ctx.span_bays(id, &carcass, bays, span.as_ref())?;
    let spec_zone = zone.as_ref();
    let zone = ctx.zone_for(id, &carcass, zone.as_ref())?;
    // An inset door lives inside the opening: between the top and bottom
    // panels, not over them.
    let zone = match mount {
        FrontMount::Overlay => zone,
        FrontMount::Inset => super::Zone {
            z0: zone.z0.max(carcass.thickness),
            z1: zone.z1.min(carcass.height - carcass.thickness),
        },
    };
    if *mount == FrontMount::Inset && bays.iter().any(|b| b.spans > 1) {
        return Err(Diagnostic::new(
            "SPEC-317",
            Severity::Fatal,
            format!("'{id}': una puerta embutida no puede cubrir varias bahías; el divisor queda en su plano"),
        )
        .entity(id)
        .suggestion("Usá 'mount: overlay' para el juego que cruza bahías, o una puerta por bahía."));
    }
    // Needed below for the front plane; the material decides the thickness.
    let material_id = ctx.material_or_default(id, material.as_ref())?.to_string();
    let t_front = ctx.thickness_of(&material_id);
    let front_y = match mount {
        FrontMount::Overlay => (carcass.depth, carcass.depth + t_front),
        FrontMount::Inset => (carcass.depth - t_front, carcass.depth),
    };
    for b in &bays {
        let covered: Vec<super::Bay> = carcass
            .bays
            .iter()
            .filter(|cb| cb.index >= b.index && cb.index < b.index + b.spans)
            .cloned()
            .collect();
        ctx.occupy_front(
            id,
            OccupancyKind::Doors,
            &carcass,
            &covered,
            zone,
            Some(front_y),
        );
    }
    let hinge = hinge.as_ref();
    // The catch and the plate it meets on the door.
    let catch = catch.as_ref().map(|c| {
        let strikes: Vec<String> = c
            .hardware
            .iter()
            .filter_map(|h| ctx.libs.hardware.get(h))
            .filter_map(|d| d.catch.as_ref().and_then(|c| c.strike.clone()))
            .collect();
        let push = c
            .hardware
            .iter()
            .filter_map(|h| ctx.libs.hardware.get(h))
            .any(|d| d.catch.as_ref().is_some_and(|c| c.push));
        (c.hardware.clone(), strikes, push)
    });
    if let Some(z) = spec_zone {
        ctx.note_literals(id, &[("zone.from", &z.from), ("zone.to", &z.to)]);
    }
    let count = ctx.eval(id, "count", count)?;
    if count < 1.0 || count.fract() != 0.0 {
        return Err(Diagnostic::new(
            "SPEC-303",
            Severity::Fatal,
            format!("'{id}.count' tiene que ser un entero ≥ 1, es {count}"),
        )
        .entity(id));
    }
    let count = count as usize;
    if count > 2 && hinge.is_some() {
        return Err(Diagnostic::new(
            "SPEC-306",
            Severity::Fatal,
            format!(
                "'{id}': {count} puertas por bahía no tienen de dónde colgarse; usá más bahías"
            ),
        )
        .entity(id)
        .suggestion("Subí 'bays' en la carcasa, o declará hinge: null."));
    }
    let gap = ctx.eval(id, "gap", gap)?;
    let material = ctx.material_or_default(id, material.as_ref())?.to_string();
    let t = ctx.thickness_of(&material);

    let door_height = (zone.z1 - zone.z0) - 2.0 * gap;
    let cover = |b: &super::Bay| match mount {
        FrontMount::Overlay => (b.cover_x0, b.cover_x1),
        FrontMount::Inset => (b.x0, b.x1),
    };
    let min_span = bays
        .iter()
        .map(|b| {
            let (a, z) = cover(b);
            z - a
        })
        .fold(f64::INFINITY, f64::min);
    let door_width = (min_span - gap * (count as f64 + 1.0)) / count as f64;
    if door_width <= 0.0 || door_height <= 0.0 {
        return Err(Diagnostic::new(
            "SPEC-305",
            Severity::Fatal,
            format!("{count} puertas con {gap} mm de luz no entran en {min_span} mm"),
        )
        .entity(id));
    }
    ctx.publish(id, "count", count as f64);
    ctx.publish(id, "door_width", door_width);
    ctx.publish(id, "door_height", door_height);

    // What a cabinetmaker would say before cutting: none of it stops the
    // doors from being generated.
    if door_width > 600.0 {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-102",
                Severity::Warning,
                format!(
                    "'{id}': puertas de {} mm de ancho; más de 600 fuerza las bisagras y barre mucho al abrir",
                    mm(door_width)
                ),
            )
            .entity(id)
            .suggestion("Poné dos puertas por bahía, o más bahías.")
            .fix_opt(if count == 1 {
                Some((
                    "Dos puertas por bahía".to_string(),
                    id.to_string(),
                    "count".to_string(),
                    serde_json::json!(2),
                ))
            } else {
                Some((
                    format!("Carcasa en {} bahías", carcass.bays.len() + 1),
                    carcass.id.clone(),
                    "bays".to_string(),
                    serde_json::json!(carcass.bays.len() + 1),
                ))
            }),
        );
    } else if door_width < 200.0 {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-102",
                Severity::Warning,
                format!(
                    "'{id}': puertas de {} mm de ancho, demasiado angostas para bisagra y manija",
                    mm(door_width)
                ),
            )
            .entity(id)
            .suggestion("Una puerta por bahía, o una bahía más ancha.")
            .fix("Una puerta por bahía", id, "count", serde_json::json!(1)),
        );
    }
    if gap < 1.5 {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-103",
                Severity::Warning,
                format!(
                    "'{id}': {} mm de luz entre frentes; con menos de 1,5 mm rozan al abrir",
                    mm(gap)
                ),
            )
            .entity(id)
            .suggestion("Usá 2–3 mm de luz.")
            .fix("Luz de 2 mm", id, "gap", serde_json::json!(2)),
        );
    }
    if let Some(handle) = handle {
        let from_edge = ctx.eval(id, "handle.fromEdge", &handle.from_edge)?;
        if from_edge > door_width / 2.0 {
            ctx.warn(
                Diagnostic::new(
                    "DESIGN-110",
                    Severity::Warning,
                    format!(
                        "'{id}': la manija a {} mm del borde pasa la mitad de una puerta de {} mm y queda del lado de la bisagra",
                        mm(from_edge),
                        mm(door_width)
                    ),
                )
                .entity(id)
                .suggestion("Bajá 'handle.fromEdge' (40–60 mm es lo usual).")
                .fix("Manija a 40 mm del borde", id, "handle.fromEdge", serde_json::json!(40)),
            );
        }
    }
    if *edges == EdgeBanding::None {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-109",
                Severity::Warning,
                format!("'{id}': puertas sin canto; los bordes de placa quedan a la vista"),
            )
            .entity(id)
            .suggestion("Sacá 'edges: none' o dejá 'all'.")
            .fix("Cantear las puertas", id, "edges", serde_json::Value::Null),
        );
    }
    if let Some((_, _, true)) = &catch {
        // A push latch throws the door open: a damper pulls it shut
        // again, and a handle makes it pointless.
        let damped = hinge.is_some_and(|h| {
            soft_close.unwrap_or_else(|| {
                h.hardware
                    .iter()
                    .filter_map(|x| ctx.libs.hardware.get(x))
                    .any(|d| d.hinge.as_ref().is_some_and(|s| s.soft_close))
            })
        });
        if damped {
            ctx.warn(
                Diagnostic::new(
                    "DESIGN-116",
                    Severity::Warning,
                    format!("'{id}': push-open con bisagras de cierre suave; el amortiguador vuelve a cerrar la puerta y anula el push"),
                )
                .entity(id)
                .suggestion("Sacá 'softClose' (el push-open va con bisagras sin resorte) o usá un cierre magnético.")
                .fix("Bisagras sin cierre suave", id, "softClose", serde_json::json!(false)),
            );
        }
        if handle.is_some() {
            ctx.warn(
                Diagnostic::new(
                    "DESIGN-116",
                    Severity::Info,
                    format!("'{id}': push-open y tirador en la misma puerta; el push-open es para frentes sin tirador"),
                )
                .entity(id)
                .suggestion("Sacá 'handle' o cambiá el cierre por uno magnético.")
                .fix("Sin tirador", id, "handle", serde_json::Value::Null),
            );
        }
    }

    let all_edges = super::banded_axes(*edges, &[Axis::PosX, Axis::NegX, Axis::PosZ, Axis::NegZ]);
    let many = bays.len() > 1;
    let mut n = 0;
    // Overlay: the door's back is on the carcass front (y = depth) and the
    // panel stands in front of it. Inset: its front is flush with the
    // carcass front and the panel sits inside.
    let y_origin = match mount {
        FrontMount::Overlay => carcass.depth + t,
        FrontMount::Inset => carcass.depth,
    };
    for bay in &bays {
        let (c0, c1) = cover(bay);
        let span = c1 - c0;
        let width = (span - gap * (count as f64 + 1.0)) / count as f64;
        for i in 0..count {
            n += 1;
            let x_left = c0 + gap + i as f64 * (width + gap);
            let (name, role) = if many {
                (
                    format!("Puerta {n} bahía {}", bay.index),
                    format!("bay{}_door_{}", bay.index, i + 1),
                )
            } else {
                (format!("Puerta {n}"), format!("door_{n}"))
            };
            let door = ctx.add_part(PartInit {
                name,
                component: id,
                role: &role,
                material: &material,
                length: door_height,
                width,
                grain: Grain::Length,
                // Local X up, local Y towards -X, so local +Z = -Y (faces the carcass).
                placement: Placement::new(
                    Vec3(x_left + width, y_origin, zone.z0 + gap),
                    Axis::PosZ,
                    Axis::NegX,
                ),
                banded_edges: &all_edges,
            });
            let (hinge_edge, panel, opposite) = if i == 0 {
                (Axis::NegX, &bay.left_part, &bay.right_part)
            } else {
                (Axis::PosX, &bay.right_part, &bay.left_part)
            };
            if let Some(hinge) = hinge {
                // The arm follows how the door sits on its panel: a side
                // is covered whole, a divider only to its middle.
                let on_divider = *panel != carcass.side_left && *panel != carcass.side_right;
                let arm = match (mount, on_divider) {
                    (FrontMount::Inset, _) => "inset",
                    (FrontMount::Overlay, true) => "half_overlay",
                    (FrontMount::Overlay, false) => "overlay",
                };
                let hinge = ctx.hinge_variant(id, hinge, arm, *soft_close)?;
                // Cups half a pitch off the System 32 grid: the plate's two
                // holes, 16 above and below, land on the shelf-pin rows.
                ctx.request_snapped(
                    JointKind::Hinge { hinge_edge },
                    id,
                    &door,
                    panel,
                    &hinge,
                    carcass.origin.2 + super::GRID_ORIGIN + super::PIN_PITCH / 2.0,
                    super::PIN_PITCH,
                );
            }
            if let Some((catch_hw, strikes, _)) = &catch {
                let y_back = y_origin - t;
                // Where the body goes and where the plate meets it.
                let placed: Option<(String, Face, Vec3, Vec3)> = if count == 1 {
                    // On the panel opposite the hinge, on the face that
                    // looks into the bay, at the door's mid height; the
                    // plate just inside that panel's face.
                    let z = zone.z0 + gap + door_height / 2.0;
                    let door_cx = x_left + width / 2.0;
                    let p = ctx.part_mut(opposite);
                    let into_bay = if door_cx < p.aabb.center().0 {
                        -1.0
                    } else {
                        1.0
                    };
                    let x_face = if into_bay < 0.0 {
                        p.aabb.min.0
                    } else {
                        p.aabb.max.0
                    };
                    let normal = p.placement.world_axis(Face::Front.normal_local());
                    let face = if normal.vec().0 * into_bay > 0.0 {
                        Face::Front
                    } else {
                        Face::Back
                    };
                    Some((
                        opposite.clone(),
                        face,
                        Vec3(x_face, y_back - CATCH_SETBACK, z),
                        Vec3(x_face + into_bay * STRIKE_INSET, y_back, z),
                    ))
                } else {
                    // Doors meeting in the middle: under the top at the
                    // opening edge, or on the bottom when the set does not
                    // reach the top.
                    let x = if hinge_edge == Axis::NegX {
                        x_left + width - CATCH_FROM_EDGE
                    } else {
                        x_left + CATCH_FROM_EDGE
                    };
                    let at_top = (zone.z1 - carcass.height).abs() < crate::units::EPS
                        || (*mount == FrontMount::Inset
                            && (zone.z1 - (carcass.height - carcass.thickness)).abs()
                                < crate::units::EPS);
                    let at_bottom = zone.z0 < crate::units::EPS
                        || (*mount == FrontMount::Inset
                            && (zone.z0 - carcass.thickness).abs() < crate::units::EPS);
                    let role = if at_top {
                        Some("top")
                    } else if at_bottom {
                        Some("bottom")
                    } else {
                        None
                    };
                    let panel = role.and_then(|r| {
                        ctx.parts
                            .iter()
                            .find(|p| p.component == carcass.id && p.role == r)
                    });
                    match panel {
                        Some(p) => {
                            let (z_face, dir) = if at_top {
                                (p.aabb.min.2, -1.0)
                            } else {
                                (p.aabb.max.2, 1.0)
                            };
                            let normal = p.placement.world_axis(Face::Front.normal_local());
                            let face = if normal.vec().2 * dir > 0.0 {
                                Face::Front
                            } else {
                                Face::Back
                            };
                            Some((
                                p.id.clone(),
                                face,
                                Vec3(x, y_back - CATCH_SETBACK, z_face),
                                Vec3(x, y_back, z_face + dir * STRIKE_DROP),
                            ))
                        }
                        None => {
                            if n == 1 {
                                ctx.warn(
                                    Diagnostic::new(
                                        "DESIGN-116",
                                        Severity::Warning,
                                        format!("'{id}': dos puertas por bahía se encuentran lejos de los laterales; el cierre va bajo la tapa o sobre la base, y la zona no llega a ninguna de las dos"),
                                    )
                                    .entity(id)
                                    .suggestion("Llevá la zona hasta la tapa o la base, o usá un estante fijo como apoyo del cierre (no modelado)."),
                                );
                            }
                            None
                        }
                    }
                };
                if let Some((panel, face, body, plate)) = placed {
                    let spec = crate::spec::JointSpec {
                        hardware: catch_hw.clone(),
                        placement: None,
                    };
                    ctx.request(
                        JointKind::Fixture {
                            centre: body,
                            face,
                            along: Axis::NegY,
                        },
                        id,
                        &panel,
                        &panel,
                        &spec,
                    );
                    if !strikes.is_empty() {
                        let spec = crate::spec::JointSpec {
                            hardware: strikes.clone(),
                            placement: None,
                        };
                        ctx.request(
                            JointKind::Fixture {
                                centre: plate,
                                face: Face::Front,
                                along: Axis::PosZ,
                            },
                            id,
                            &door,
                            &door,
                            &spec,
                        );
                    }
                }
            }
            if let Some(handle) = handle {
                // Vertical, on the opening edge (opposite the hinge).
                let from_edge = ctx.eval(id, "handle.fromEdge", &handle.from_edge)?;
                let z = match &handle.position {
                    Some(p) => zone.z0 + gap + ctx.eval(id, "handle.position", p)?,
                    None => zone.z0 + gap + door_height / 2.0,
                };
                let x = if hinge_edge == Axis::NegX {
                    x_left + width - from_edge
                } else {
                    x_left + from_edge
                };
                let centre = Vec3(x, y_origin - t, z);
                let spec = crate::spec::JointSpec {
                    hardware: handle.hardware.clone(),
                    placement: None,
                };
                ctx.request(
                    JointKind::Handle {
                        centre,
                        along: Axis::PosZ,
                    },
                    id,
                    &door,
                    &door,
                    &spec,
                );
            }
        }
    }
    Ok(())
}
