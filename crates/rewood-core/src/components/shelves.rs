//! Shelves inside a carcass bay, joined to the two panels bounding the
//! bay. Either `count` shelves spread evenly over a zone (they clear the
//! back and sit back from the front), or shelves at explicit heights
//! (`positions`): fixed shelves that take the full inner depth — the
//! horizontal divider between, say, a drawer zone and a door zone.
//!
//! With `support: pins` the shelves are not joined at all: each bay panel
//! gets two System 32 rows over the zone (37 mm from the front edge and
//! from the back panel), the shelves are a millimetre short on each side
//! and rest on four pins that go to the BOM by count.

use super::{BuildCtx, ExtraBom, JointKind, OccupancyKind, PartInit, GRID_ORIGIN, ROW_INSET};
use crate::diagnostics::{Diagnostic, Severity};
use crate::geometry::{Axis, Face, Placement, Vec3};
use crate::model::Grain;
use crate::rules::mm;
use crate::spec::{ComponentSpec, JointSpec, PinsSpec, ShelfSupport};

/// Side play of a pin-supported shelf, per side.
const PIN_PLAY: f64 = 1.0;

pub fn build(ctx: &mut BuildCtx<'_>, spec: &ComponentSpec) -> Result<(), Diagnostic> {
    let ComponentSpec::Shelves {
        id,
        carcass,
        bay,
        zone,
        count,
        positions,
        setback,
        material,
        support,
        joint,
        pins,
        edges,
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
    let bays = ctx.bays_for(id, &carcass, bay.as_ref())?;
    let spec_zone = zone.as_ref();
    let zone = ctx.zone_for(id, &carcass, zone.as_ref())?;
    ctx.occupy(id, OccupancyKind::Shelves, &carcass, &bays, zone);
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
            format!("el retranqueo {setback} deja a los estantes de '{id}' sin profundidad"),
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
                        "el estante fijo de '{id}' a {z} mm ({t} de espesor) no cabe entre {z_lo} y {z_hi}"
                    ),
                )
                .entity(id));
            }
            if let Some(prev) = zs.last() {
                if z < prev + t - crate::units::EPS {
                    return Err(Diagnostic::new(
                        "SPEC-311",
                        Severity::Fatal,
                        format!("las 'positions' de '{id}' tienen que ir en orden y separadas al menos {t} mm"),
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
                    "no entran {count} estantes de {t} mm en una altura interior de {inner_height} mm"
                ),
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
        if on_pins {
            pin_rows(ctx, id, &carcass, bay, z_lo, z_hi, pins)?;
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

/// Two System 32 rows on each panel bounding the bay, over the zone: the
/// front row [`ROW_INSET`] from the carcass front, the back row the same
/// distance in front of the back panel. Holes start at [`GRID_ORIGIN`]
/// from the carcass floor and repeat every pitch while a shelf's pin can
/// still sit on them (a hole less than a pin's reach from the zone ends
/// holds nothing).
fn pin_rows(
    ctx: &mut BuildCtx<'_>,
    id: &str,
    carcass: &super::CarcassInfo,
    bay: &super::Bay,
    z_lo: f64,
    z_hi: f64,
    pins: &PinsSpec,
) -> Result<(), Diagnostic> {
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
    // First and last hole on the grid inside the zone, one pitch clear
    // of its ends so the lowest shelf still has room under it.
    let origin = carcass.origin.2 + GRID_ORIGIN;
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
    // The back row stops under the hangers of a wall-hung carcass.
    let last_back = match carcass.hanger_clear_z {
        Some(z) => origin + ((z.min(z_hi - pitch) - origin) / pitch).floor() * pitch,
        None => last,
    };
    let ys = [
        (carcass.depth - ROW_INSET, last),
        (carcass.inner_y0 + ROW_INSET, last_back),
    ];
    let spec = JointSpec {
        hardware: vec![row_id.clone()],
        placement: None,
    };
    for (x, part_id, looks) in [
        (bay.x0, &bay.left_part, Axis::PosX),
        (bay.x1, &bay.right_part, Axis::NegX),
    ] {
        let face: Face = ctx.part_mut(part_id).face_facing(looks);
        for (y, last) in ys {
            if last < first {
                continue;
            }
            ctx.request(
                JointKind::Row {
                    from: Vec3(x, y, first),
                    to: Vec3(x, y, last),
                    face,
                },
                id,
                part_id,
                part_id,
                &spec,
            );
        }
    }
    ctx.publish(id, "pin_rows", 4.0);
    ctx.publish(
        id,
        "pin_holes_per_row",
        ((last - first) / pitch).round() + 1.0,
    );
    Ok(())
}
