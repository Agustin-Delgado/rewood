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
use crate::spec::{ComponentSpec, EdgeBanding, FrontMount, HingeSide};

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
/// Edge a fixed front has to cover for a dowel into it: Ø8 with 4 mm of
/// panel on each side.
const FIXING_WIDTH: f64 = 16.0;

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
        hinge_side,
        soft_close,
        catch,
        handle,
        fixed,
        fixing,
        facing,
        edges,
        ..
    } = spec
    else {
        unreachable!()
    };
    let id = id.as_str();
    let fixed = *fixed;
    if fixed && (*mount != FrontMount::Overlay || handle.is_some() || catch.is_some()) {
        return Err(Diagnostic::new(
            "SPEC-334",
            Severity::Fatal,
            format!("'{id}' es un frente fijo: va superpuesto, sin tirador ni cierre"),
        )
        .entity(id)
        .suggestion("Sacá 'mount', 'handle' y 'catch', o sacá 'fixed'."));
    }
    let fixing = fixing.clone().unwrap_or_else(|| crate::spec::JointSpec {
        hardware: vec!["dowel_8x30".into()],
        placement: None,
    });

    let carcass = ctx.carcass_for(id, carcass.as_ref())?;
    let bays = ctx.bays_for(id, &carcass, bay.as_ref(), last_bay.as_ref())?;
    let bays = ctx.span_bays(id, &carcass, bays, span.as_ref())?;
    let run_middle = run_middle(ctx, carcass.turns);
    // The carcass origin along its own run (turned back for a turned run).
    let run_x = frame_x(carcass.origin, carcass.turns);
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
    let hinge = if fixed { None } else { hinge.as_ref() };
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
    if count > 1 && !hinge_side.is_auto() {
        return Err(Diagnostic::new(
            "SPEC-306",
            Severity::Fatal,
            format!("'{id}': con {count} puertas por bahía cada una cuelga de su panel exterior; 'hingeSide' es para una sola"),
        )
        .entity(id));
    }
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
    if gap < 0.0 {
        return Err(Diagnostic::new(
            "SPEC-304",
            Severity::Fatal,
            format!("'{id}.gap' = {} mm: no puede ser negativo", mm(gap)),
        )
        .entity(id));
    }
    let material = ctx.material_or_default(id, material.as_ref())?.to_string();
    let t = ctx.thickness_of(&material);
    // A glass door hangs on glass hinges: a board hinge's Ø35 cup does not
    // go into glass.
    let glass_door = ctx
        .libs
        .materials
        .material(&material)
        .is_some_and(|m| m.outsourced);
    if let Some(h) = hinge.as_ref().filter(|_| !fixed) {
        let glass_hinge = h
            .hardware
            .iter()
            .filter_map(|x| ctx.libs.hardware.get(x)?.hinge.as_ref())
            .any(|s| s.glass);
        if glass_door != glass_hinge {
            let alt = ctx.libs.hardware.iter().find(|d| {
                d.hinge
                    .as_ref()
                    .is_some_and(|s| s.glass == glass_door && s.mount == "overlay" && !s.soft_close)
            });
            return Err(Diagnostic::new(
                "SPEC-337",
                Severity::Fatal,
                if glass_door {
                    format!("'{id}': las puertas son de vidrio y la bisagra es para placa")
                } else {
                    format!("'{id}': la bisagra es para vidrio y las puertas son de placa")
                },
            )
            .entity(id)
            .fix_opt(alt.map(|a| {
                (
                    format!("Usar {}", a.name),
                    id.to_string(),
                    "hinge.hardware".to_string(),
                    serde_json::json!([a.id]),
                )
            })));
        }
    }
    // The facing glued on the front: its material and how far in.
    let facing = match facing {
        Some(f) => {
            let m = ctx.material_or_default(id, Some(&f.material))?.to_string();
            let inset = ctx.eval(id, "facing.inset", &f.inset)?;
            if handle.is_some() {
                return Err(Diagnostic::new(
                    "SPEC-337",
                    Severity::Fatal,
                    format!("'{id}': un tirador no se atornilla a través del espejo"),
                )
                .entity(id)
                .suggestion("Sacá el tirador y abrí con push-open ('catch' con push_latch)."));
            }
            Some((m, inset, f.adhesive.clone()))
        }
        None => None,
    };

    // Where the zone ends inside the carcass another front (or the next
    // zone) takes over: each side leaves half the gap, so fronts that meet
    // there keep one gap between them, not two.
    let (gap_lo, gap_hi) =
        super::zone_gaps(*mount == FrontMount::Overlay, zone, carcass.height, gap);
    let door_height = (zone.z1 - zone.z0) - gap_lo - gap_hi;
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
            format!(
                "{count} puertas con {gap} mm de luz no entran en {min_span} mm",
                gap = mm(gap),
                min_span = mm(min_span)
            ),
        )
        .entity(id));
    }
    ctx.publish(id, "count", count as f64);
    ctx.publish(id, "door_width", door_width);
    ctx.publish(id, "door_height", door_height);

    // What a cabinetmaker would say before cutting: none of it stops the
    // doors from being generated.
    if fixed {
        // A fixed front is as wide as the blind part it closes.
    } else if door_width > 600.0 {
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
    // On a front, `front` names its face, not an edge: nothing gets banded.
    if matches!(*edges, EdgeBanding::None | EdgeBanding::Front) {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-109",
                Severity::Warning,
                format!("'{id}': puertas sin canto; los bordes de placa quedan a la vista"),
            )
            .entity(id)
            .suggestion("Dejá 'all' (o sacá 'edges'); 'front' en un frente no cantea nada, su cara es la que mira adelante.")
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
            let (noun, stem) = if fixed {
                ("Frente fijo", "fixed_front")
            } else {
                ("Puerta", "door")
            };
            let (name, role) = if many {
                (
                    format!("{noun} {n} bahía {}", bay.index),
                    format!("bay{}_{stem}_{}", bay.index, i + 1),
                )
            } else {
                (format!("{noun} {n}"), format!("{stem}_{n}"))
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
                    Vec3(x_left + width, y_origin, zone.z0 + gap_lo),
                    Axis::PosZ,
                    Axis::NegX,
                ),
                banded_edges: &all_edges,
            });
            if let Some((m, inset, adhesive)) = &facing {
                // Glued over the door's front, a little in from its edges.
                let tm = ctx.thickness_of(m);
                let (mname, mrole) = if many {
                    (
                        format!("Espejo {n} bahía {}", bay.index),
                        format!("bay{}_mirror_{}", bay.index, i + 1),
                    )
                } else {
                    (format!("Espejo {n}"), format!("mirror_{n}"))
                };
                let front = y_origin;
                ctx.add_part(PartInit {
                    name: mname,
                    component: id,
                    role: &mrole,
                    material: m,
                    length: door_height - 2.0 * inset,
                    width: width - 2.0 * inset,
                    grain: Grain::None,
                    placement: Placement::new(
                        Vec3(x_left + width - inset, front + tm, zone.z0 + gap_lo + inset),
                        Axis::PosZ,
                        Axis::NegX,
                    ),
                    banded_edges: &[],
                });
                if let Some(h) = adhesive.first() {
                    ctx.extra_bom.push(super::ExtraBom {
                        component: id.to_string(),
                        hardware: h.clone(),
                        quantity: 1,
                        metres: 0.0,
                    });
                }
            }
            if fixed {
                // Joined to the front edges of the panels behind it (the
                // bay's sides, the top, the bottom) where the front covers
                // enough of the edge for a dowel: a divider, covered only
                // to its middle, is too narrow.
                let d = ctx.parts.iter().find(|q| q.id == door).map(|q| q.aabb);
                let candidates: Vec<String> = [bay.left_part.clone(), bay.right_part.clone()]
                    .into_iter()
                    .chain(
                        ctx.parts
                            .iter()
                            .filter(|q| {
                                q.component == carcass.id && (q.role == "top" || q.role == "bottom")
                            })
                            .map(|q| q.id.clone()),
                    )
                    .collect();
                let behind: Vec<String> = candidates
                    .into_iter()
                    .filter(|p| {
                        let (Some(a), Some(d)) =
                            (ctx.parts.iter().find(|q| q.id == *p).map(|q| q.aabb), d)
                        else {
                            return false;
                        };
                        let dx = a.max.0.min(d.max.0) - a.min.0.max(d.min.0);
                        let dz = a.max.2.min(d.max.2) - a.min.2.max(d.min.2);
                        dx.min(dz) >= FIXING_WIDTH - crate::units::EPS
                    })
                    .collect();
                if behind.is_empty() {
                    return Err(Diagnostic::new(
                        "SPEC-334",
                        Severity::Fatal,
                        format!("'{id}': el frente fijo no cubre ningún canto donde fijarse"),
                    )
                    .entity(id)
                    .suggestion("Un frente fijo tiene que tapar un lateral, la tapa o la base."));
                }
                for panel in &behind {
                    ctx.request_joint(id, panel, &door, &fixing);
                }
                continue;
            }
            // Two doors hang on their outer panels; one hangs where
            // `hingeSide` says, `auto` away from the furniture's middle.
            let on_right = if count == 1 {
                match hinge_side {
                    HingeSide::Left => false,
                    HingeSide::Right => true,
                    HingeSide::Auto => {
                        // Away from drawers next door at the door's height
                        // (they come out where the door swings); failing
                        // that, away from the furniture's middle, so
                        // neighbouring doors open away from each other.
                        let drawers_at = |index: usize| {
                            ctx.occupancy.iter().any(|o| {
                                o.carcass == carcass.id
                                    && o.bay == index
                                    && o.kind == OccupancyKind::Drawers
                                    && o.zone.z0 < zone.z1 - 1.0
                                    && o.zone.z1 > zone.z0 + 1.0
                            })
                        };
                        let first = bay.index;
                        let last = bay.index + bay.spans - 1;
                        let left = first > 1 && drawers_at(first - 1);
                        let right = drawers_at(last + 1);
                        match (left, right) {
                            (true, false) => true,
                            (false, true) => false,
                            _ => {
                                let door_mid = run_x + x_left + width / 2.0;
                                run_middle.is_some_and(|m| door_mid > m + 1.0)
                            }
                        }
                    }
                }
            } else {
                i > 0
            };
            let (hinge_edge, panel, opposite) = if on_right {
                (Axis::PosX, &bay.right_part, &bay.left_part)
            } else {
                (Axis::NegX, &bay.left_part, &bay.right_part)
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
                    let z = zone.z0 + gap_lo + door_height / 2.0;
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
                    Some(p) => zone.z0 + gap_lo + ctx.eval(id, "handle.position", p)?,
                    None => zone.z0 + gap_lo + door_height / 2.0,
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

/// X of a point in the frame of a run turned `turns` quarters: the point
/// turned back.
fn frame_x(p: Vec3, turns: u8) -> f64 {
    p.turned_about(Vec3::ZERO, (4 - turns % 4) % 4).0
}

/// The middle of the run a carcass belongs to, along the run: halfway
/// across every carcass turned the same way that the spec switches on, the
/// ones not built yet included (a run's last module may come after this
/// door in the spec).
fn run_middle(ctx: &BuildCtx<'_>, turns: u8) -> Option<f64> {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for c in &ctx.spec.components {
        let ComponentSpec::Carcass {
            id, width, origin, ..
        } = c
        else {
            continue;
        };
        if !ctx.included(c).unwrap_or(false) {
            continue;
        }
        let Ok(w) = ctx.eval(id, "width", width) else {
            continue;
        };
        let (p, q) = match origin {
            Some(o) => {
                let (Ok(x), Ok(y)) = (
                    ctx.eval(id, "origin.x", &o.x),
                    ctx.eval(id, "origin.y", &o.y),
                ) else {
                    continue;
                };
                let q = match &o.rotation {
                    Some(r) => match ctx.eval(id, "origin.rotation", r) {
                        Ok(d) => (d / 90.0).round().rem_euclid(4.0) as u8,
                        Err(_) => continue,
                    },
                    None => 0,
                };
                (Vec3(x, y, 0.0), q)
            }
            None => (Vec3::ZERO, 0),
        };
        if q != turns {
            continue;
        }
        let x = frame_x(p, q);
        lo = lo.min(x);
        hi = hi.max(x + w);
    }
    (lo < hi).then_some((lo + hi) / 2.0)
}
