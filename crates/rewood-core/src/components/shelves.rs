//! Shelves inside a carcass bay, joined to the two panels bounding the
//! bay. Either `count` shelves spread evenly over a zone (they clear the
//! back and sit back from the front), or shelves at explicit heights
//! (`positions`): fixed shelves that take the full inner depth — the
//! horizontal divider between, say, a drawer zone and a door zone.
//!
//! With `support: pins` the shelves are not joined at all: each shelf
//! drops to the nearest System 32 grid line and the bay panels get only
//! the holes it rests on (37 mm from the front edge and from the back
//! panel), plus `pins.adjust` of travel on each side when the shelf has to
//! be movable. The shelves are a millimetre short on each side and rest on
//! four pins that go to the BOM by count.

use super::{BuildCtx, ExtraBom, JointKind, OccupancyKind, PartInit, GRID_ORIGIN, ROW_INSET};
use crate::diagnostics::{Diagnostic, Severity};
use crate::geometry::{Axis, Face, Placement, Vec3};
use crate::model::Grain;
use crate::rules::mm;
use crate::spec::{ComponentSpec, JointSpec, PinsSpec, ShelfSupport};

/// How far under a shelf its front pin has to be to carry it.
const PIN_REACH: f64 = 10.0;

/// Side play of a pin-supported shelf, per side.
const PIN_PLAY: f64 = 1.0;

pub fn build(ctx: &mut BuildCtx<'_>, spec: &ComponentSpec) -> Result<(), Diagnostic> {
    let ComponentSpec::Shelves {
        id,
        carcass,
        bay,
        last_bay,
        zone,
        count,
        positions,
        setback,
        material,
        support,
        joint,
        pins,
        edges,
        ..
    } = spec
    else {
        unreachable!()
    };
    let id = id.as_str();
    let on_pins = *support == ShelfSupport::Pins;
    let joint = match (on_pins, joint) {
        (true, _) => None,
        (false, Some(j)) => Some(j),
        (false, None) => {
            return Err(Diagnostic::new(
                "SPEC-318",
                Severity::Fatal,
                format!("'{id}' no dice con qué herrajes va unido ('joint') ni que apoya en soportes ('support: pins')"),
            )
            .entity(id)
            .suggestion("Agregá joint: { hardware: [\"dowel_8x30\"] } o support: \"pins\"."));
        }
    };
    let default_pins = PinsSpec::default();
    let pins = pins.as_ref().unwrap_or(&default_pins);
    if on_pins && fixed_positions(positions) {
        return Err(Diagnostic::new(
            "SPEC-318",
            Severity::Fatal,
            format!("'{id}': un estante fijo ('positions') no puede apoyar en soportes; va unido"),
        )
        .entity(id));
    }

    let carcass = ctx.carcass_for(id, carcass.as_ref())?;
    let bays = ctx.bays_for(id, &carcass, bay.as_ref(), last_bay.as_ref())?;
    let spec_zone = zone.as_ref();
    let zone = ctx.zone_for(id, &carcass, zone.as_ref())?;
    // The shelves' front edge, for the layout check against a door plane
    // (an inset door, sliding doors); computed again below.
    let front_edge = {
        let fixed = !positions.is_empty();
        match setback {
            Some(s) => ctx.eval(id, "setback", s).unwrap_or(0.0),
            None if fixed => 0.0,
            None => 20.0,
        }
    };
    let y_edge = carcass.depth - front_edge;
    ctx.occupy_front(
        id,
        OccupancyKind::Shelves,
        &carcass,
        &bays,
        zone,
        Some((y_edge, y_edge)),
    );
    {
        let mut fields: Vec<(&str, &crate::params::ParamInput)> = Vec::new();
        if let Some(z) = spec_zone {
            fields.push(("zone.from", &z.from));
            fields.push(("zone.to", &z.to));
        }
        let names: Vec<String> = (0..positions.len())
            .map(|k| format!("positions[{k}]"))
            .collect();
        for (name, p) in names.iter().zip(positions) {
            fields.push((name.as_str(), p));
        }
        ctx.note_literals(id, &fields);
    }
    let fixed = !positions.is_empty();
    if fixed && count.is_some() {
        return Err(Diagnostic::new(
            "SPEC-310",
            Severity::Fatal,
            format!("'{id}' tiene 'count' y 'positions': es uno u otro"),
        )
        .entity(id));
    }
    let setback = match setback {
        Some(s) => ctx.eval(id, "setback", s)?,
        None if fixed => 0.0,
        None => 20.0,
    };
    if setback < 0.0 {
        return Err(Diagnostic::new(
            "SPEC-304",
            Severity::Fatal,
            format!("'{id}.setback' = {} mm: no puede ser negativo", mm(setback)),
        )
        .entity(id));
    }
    let material = ctx.material_or_default(id, material.as_ref())?.to_string();
    let t = ctx.thickness_of(&material);
    let ct = carcass.thickness;

    // Shelves live between the carcass bottom and top, inside the zone.
    let z_lo = zone.z0.max(ct);
    let z_hi = zone.z1.min(carcass.height - ct);
    let inner_height = z_hi - z_lo;
    let shelf_depth = carcass.depth - carcass.inner_y0 - setback;
    if shelf_depth <= 0.0 {
        return Err(Diagnostic::new(
            "SPEC-304",
            Severity::Fatal,
            format!(
                "el retranqueo {setback} deja a los estantes de '{id}' sin profundidad",
                setback = mm(setback)
            ),
        )
        .entity(id));
    }

    // Underside height of every shelf, from the carcass bottom face.
    let heights: Vec<f64> = if fixed {
        let mut zs = Vec::with_capacity(positions.len());
        for (k, p) in positions.iter().enumerate() {
            let z = ctx.eval(id, &format!("positions[{k}]"), p)?;
            if z < z_lo - crate::units::EPS || z + t > z_hi + crate::units::EPS {
                return Err(Diagnostic::new(
                    "SPEC-311",
                    Severity::Fatal,
                    format!(
                        "el estante fijo de '{id}' a {z} mm ({t} de espesor) no cabe entre {z_lo} y {z_hi}", z = mm(z), t = mm(t), z_lo = mm(z_lo), z_hi = mm(z_hi)),
                )
                .entity(id));
            }
            if let Some(prev) = zs.last() {
                if z < prev + t - crate::units::EPS {
                    return Err(Diagnostic::new(
                        "SPEC-311",
                        Severity::Fatal,
                        format!("las 'positions' de '{id}' tienen que ir en orden y separadas al menos {t} mm", t = mm(t)),
                    )
                    .entity(id));
                }
            }
            zs.push(z);
        }
        ctx.publish(id, "count", zs.len() as f64);
        zs
    } else {
        let count = match count {
            Some(c) => ctx.eval(id, "count", c)?,
            None => 0.0,
        };
        if count < 0.0 || count.fract() != 0.0 {
            return Err(Diagnostic::new(
                "SPEC-303",
                Severity::Fatal,
                format!("'{id}.count' tiene que ser un entero ≥ 0, es {count}"),
            )
            .entity(id));
        }
        let count = count as usize;
        let free = inner_height - count as f64 * t;
        if count > 0 && free <= 0.0 {
            return Err(Diagnostic::new(
                "SPEC-304",
                Severity::Fatal,
                format!(
                    "no entran {count} estantes de {t} mm en una altura interior de {inner_height} mm", t = mm(t), inner_height = mm(inner_height)),
            )
            .entity(id));
        }
        let bay_height = free / (count as f64 + 1.0);
        ctx.publish(id, "count", count as f64);
        ctx.publish(id, "bay_height", bay_height);
        (0..count)
            .map(|i| z_lo + bay_height * (i as f64 + 1.0) + t * i as f64)
            .collect()
    };
    // On pins a shelf can only sit on a hole: each one drops to the grid
    // line nearest its even spot, and the panels get just those holes.
    let (heights, grid) = if on_pins {
        let grid = pin_grid(ctx, id, &carcass, z_lo, z_hi, pins)?;
        let mut holes: Vec<f64> = Vec::with_capacity(heights.len());
        for z in &heights {
            let target = z - grid.seat;
            let mut h = GRID_ORIGIN + ((target - GRID_ORIGIN) / grid.pitch).round() * grid.pitch;
            h = h.clamp(grid.first, grid.last);
            if let Some(prev) = holes.last() {
                if h < prev + grid.pitch - crate::units::EPS {
                    h = prev + grid.pitch;
                }
            }
            // A line whose hole would meet one on the panel's other face
            // (a slide screw, a dowel): the nearest free line instead.
            let avoid = ctx.pin_avoid.get(id).cloned().unwrap_or_default();
            let blocked = |z: f64| {
                avoid
                    .iter()
                    .any(|a| (a - (z + carcass.origin.2)).abs() < 1.0)
            };
            if blocked(h) {
                let floor = holes.last().map_or(f64::NEG_INFINITY, |p| p + grid.pitch);
                let free = [1.0, -1.0, 2.0, -2.0, 3.0, -3.0]
                    .iter()
                    .map(|k| h + k * grid.pitch)
                    .find(|z| {
                        *z >= grid.first - crate::units::EPS
                            && *z <= grid.last + crate::units::EPS
                            && *z >= floor - crate::units::EPS
                            && !blocked(*z)
                    });
                if let Some(z) = free {
                    h = z;
                }
            }
            if h > grid.last + crate::units::EPS {
                return Err(Diagnostic::new(
                    "SPEC-318",
                    Severity::Fatal,
                    format!(
                        "'{id}': {} estantes no entran en la grilla de soportes (cada 32 mm entre {} y {})",
                        heights.len(),
                        mm(grid.first),
                        mm(grid.last)
                    ),
                )
                .entity(id)
                .suggestion("Menos estantes."));
            }
            holes.push(h);
        }
        (
            holes.iter().map(|h| h + grid.seat).collect::<Vec<f64>>(),
            Some((grid, holes)),
        )
    } else {
        (heights, None)
    };
    // The clear height between neighbours (and against the carcass top
    // and bottom): under 150 mm nothing fits on the shelf.
    if !heights.is_empty() {
        let mut floor = z_lo;
        let mut clear = f64::INFINITY;
        for z in &heights {
            clear = clear.min(z - floor);
            floor = z + t;
        }
        clear = clear.min(z_hi - floor);
        if clear < 150.0 {
            ctx.warn(
                Diagnostic::new(
                    "DESIGN-106",
                    Severity::Warning,
                    format!(
                        "'{id}': {} estantes dejan {} mm libres entre uno y otro; con menos de 150 no entra nada",
                        heights.len(),
                        mm(clear)
                    ),
                )
                .entity(id)
                .suggestion("Menos estantes, o una zona más alta.")
                .fix_opt((!fixed && heights.len() > 1).then(|| {
                    (
                        format!("{} estantes", heights.len() - 1),
                        id.to_string(),
                        "count".to_string(),
                        serde_json::json!(heights.len() - 1),
                    )
                })),
            );
        }
    }
    ctx.publish(id, "depth", shelf_depth);
    ctx.publish(
        id,
        "length",
        bays.iter().map(|b| b.x1 - b.x0).fold(0.0, f64::max),
    );

    let banded = super::banded_axes(*edges, &[Axis::PosY]);
    let many = bays.len() > 1;
    let kind = if fixed { "Estante fijo" } else { "Estante" };
    let role_kind = if fixed { "fixed_shelf" } else { "shelf" };
    let play = if on_pins { PIN_PLAY } else { 0.0 };
    for bay in &bays {
        if let Some((grid, holes)) = &grid {
            pin_rows(ctx, id, &carcass, bay, grid, holes, setback)?;
            ctx.extra_bom.push(ExtraBom {
                component: id.to_string(),
                hardware: pins.hardware.first().cloned().unwrap_or_default(),
                quantity: 4 * heights.len(),
                metres: 0.0,
            });
        }
        let length = bay.x1 - bay.x0 - 2.0 * play;
        for (i, z) in heights.iter().enumerate() {
            let z = *z;
            let n = i + 1;
            let (name, role) = if many {
                (
                    format!("{kind} {n} bahía {}", bay.index),
                    format!("bay{}_{role_kind}_{n}", bay.index),
                )
            } else {
                (format!("{kind} {n}"), format!("{role_kind}_{n}"))
            };
            let part = ctx.add_part(PartInit {
                name,
                component: id,
                role: &role,
                material: &material,
                length,
                width: shelf_depth,
                grain: Grain::Length,
                // Same frame as the carcass bottom: local +Z looks up.
                placement: Placement::new(
                    Vec3(bay.x0 + play, carcass.inner_y0, z),
                    Axis::PosX,
                    Axis::PosY,
                ),
                banded_edges: &banded,
            });
            if let Some(joint) = joint {
                ctx.request_joint(id, &part, &bay.left_part, joint);
                ctx.request_joint(id, &part, &bay.right_part, joint);
            }
        }
    }
    Ok(())
}

fn fixed_positions(positions: &[crate::params::ParamInput]) -> bool {
    !positions.is_empty()
}

/// The System 32 grid a bay's shelves can rest on: holes start at
/// [`GRID_ORIGIN`] from the carcass floor, one pitch clear of the zone ends
/// (a hole less than a pin's reach from them holds nothing), and the back
/// row stops under the hangers of a wall-hung carcass.
struct PinGrid {
    row: String,
    pitch: f64,
    /// Shelf underside above a hole's centre: the pin's top.
    seat: f64,
    first: f64,
    last: f64,
    last_back: f64,
    /// Travel each shelf keeps, holes on each side of its own.
    adjust: f64,
}

fn pin_grid(
    ctx: &mut BuildCtx<'_>,
    id: &str,
    carcass: &super::CarcassInfo,
    z_lo: f64,
    z_hi: f64,
    pins: &PinsSpec,
) -> Result<PinGrid, Diagnostic> {
    let Some(row_id) = pins.row.first() else {
        return Err(Diagnostic::new(
            "SPEC-318",
            Severity::Fatal,
            format!("'{id}.pins.row' no dice qué patrón de perforación usar"),
        )
        .entity(id));
    };
    let Some(row) = ctx.libs.hardware.get(row_id) else {
        return Err(Diagnostic::new(
            "SPEC-318",
            Severity::Fatal,
            format!("'{id}.pins.row': '{row_id}' no está en la biblioteca"),
        )
        .entity(id));
    };
    let pitch = row.placement.pitch.unwrap_or(super::PIN_PITCH);
    let seat = row.holes.first().map_or(2.5, |h| h.diameter / 2.0);
    let adjust = match &pins.adjust {
        Some(a) => ctx.eval(id, "pins.adjust", a)?,
        None => 0.0,
    };
    if adjust < 0.0 {
        return Err(Diagnostic::new(
            "SPEC-318",
            Severity::Fatal,
            format!(
                "'{id}.pins.adjust' = {adjust} mm: tiene que ser ≥ 0",
                adjust = mm(adjust)
            ),
        )
        .entity(id));
    }
    // Carcass space: the rows move with the carcass's origin afterwards,
    // like the hinges snapped to the same grid.
    let origin = GRID_ORIGIN;
    let first = origin + ((z_lo + pitch - origin) / pitch).ceil() * pitch;
    let last = origin + ((z_hi - pitch - origin) / pitch).floor() * pitch;
    if last < first {
        return Err(Diagnostic::new(
            "SPEC-318",
            Severity::Fatal,
            format!(
                "'{id}': la zona de {} mm no tiene lugar para una hilera de soportes",
                mm(z_hi - z_lo)
            ),
        )
        .entity(id));
    }
    let last_back = match carcass.hanger_clear_z {
        Some(z) => origin + ((z.min(z_hi - pitch) - origin) / pitch).floor() * pitch,
        None => last,
    };
    Ok(PinGrid {
        row: row_id.clone(),
        pitch,
        seat,
        first,
        last,
        last_back,
        adjust,
    })
}

/// The holes on each panel bounding the bay: under every shelf, in a front
/// row [`ROW_INSET`] from the carcass front and a back row the same
/// distance in front of the back panel, plus `adjust` of travel on each
/// side. Runs that touch merge into one row.
fn pin_rows(
    ctx: &mut BuildCtx<'_>,
    id: &str,
    carcass: &super::CarcassInfo,
    bay: &super::Bay,
    grid: &PinGrid,
    holes: &[f64],
    setback: f64,
) -> Result<(), Diagnostic> {
    // The front row sits under the shelf: with a deep setback it moves
    // back with the shelf's front edge (a hinge plate no longer shares it).
    let front = if setback + PIN_REACH > ROW_INSET {
        carcass.depth - setback - ROW_INSET
    } else {
        carcass.depth - ROW_INSET
    };
    let steps = (grid.adjust / grid.pitch + crate::units::EPS).floor();
    let runs = |last: f64| -> Vec<(f64, f64)> {
        let mut out: Vec<(f64, f64)> = Vec::new();
        for h in holes {
            let lo = (h - steps * grid.pitch).max(grid.first);
            let hi = (h + steps * grid.pitch).min(last);
            if hi < lo - crate::units::EPS {
                continue;
            }
            match out.last_mut() {
                Some(run) if lo <= run.1 + grid.pitch + crate::units::EPS => run.1 = run.1.max(hi),
                _ => out.push((lo, hi)),
            }
        }
        out
    };
    let ys = [
        (front, runs(grid.last)),
        (carcass.inner_y0 + ROW_INSET, runs(grid.last_back)),
    ];
    let spec = JointSpec {
        hardware: vec![grid.row.clone()],
        placement: None,
    };
    let mut per_row = 0.0;
    for (x, part_id, looks) in [
        (bay.x0, &bay.left_part, Axis::PosX),
        (bay.x1, &bay.right_part, Axis::NegX),
    ] {
        let face: Face = ctx.part_mut(part_id).face_facing(looks);
        for (y, runs) in &ys {
            for (lo, hi) in runs {
                ctx.request(
                    JointKind::Row {
                        from: Vec3(x, *y, *lo),
                        to: Vec3(x, *y, *hi),
                        face,
                    },
                    id,
                    part_id,
                    part_id,
                    &spec,
                );
            }
        }
    }
    for (lo, hi) in &ys[0].1 {
        per_row += ((hi - lo) / grid.pitch).round() + 1.0;
    }
    ctx.publish(id, "pin_rows", 4.0);
    ctx.publish(id, "pin_holes_per_row", per_row);
    Ok(())
}
