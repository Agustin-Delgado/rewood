//! Shelves inside a carcass bay, joined to the two panels bounding the
//! bay. Either `count` shelves spread evenly over a zone (they clear the
//! back and sit back from the front), or shelves at explicit heights
//! (`positions`): fixed shelves that take the full inner depth — the
//! horizontal divider between, say, a drawer zone and a door zone.

use super::{BuildCtx, OccupancyKind, PartInit};
use crate::diagnostics::{Diagnostic, Severity};
use crate::geometry::{Axis, Placement, Vec3};
use crate::model::Grain;
use crate::rules::mm;
use crate::spec::ComponentSpec;

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
        joint,
        edges,
    } = spec
    else {
        unreachable!()
    };
    let id = id.as_str();

    let carcass = ctx.carcass_for(id, carcass.as_ref())?;
    let bays = ctx.bays_for(id, &carcass, bay.as_ref())?;
    let zone = ctx.zone_for(id, &carcass, zone.as_ref())?;
    ctx.occupy(id, OccupancyKind::Shelves, &carcass, &bays, zone);
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
                .suggestion("Menos estantes, o una zona más alta."),
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
    for bay in &bays {
        let length = bay.x1 - bay.x0;
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
                    Vec3(bay.x0, carcass.inner_y0, z),
                    Axis::PosX,
                    Axis::PosY,
                ),
                banded_edges: &banded,
            });
            ctx.request_joint(id, &part, &bay.left_part, joint);
            ctx.request_joint(id, &part, &bay.right_part, joint);
        }
    }
    Ok(())
}
