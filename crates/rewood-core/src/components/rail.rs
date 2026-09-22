//! A hanging rail across a carcass bay: two supports screwed to the inner
//! faces of the panels bounding the bay, a bar cut to the bay's width
//! between them. No panel is generated; the supports are fixtures on the
//! sides and the bar goes to the BOM by the metre.

use super::{BuildCtx, ExtraBom, JointKind, OccupancyKind, Zone};
use crate::diagnostics::{Diagnostic, Severity};
use crate::geometry::{Axis, Vec3};
use crate::rules::mm;
use crate::spec::{ComponentSpec, JointSpec};

/// Height a hanging garment needs under the rail: short clothes.
const HANG_SHORT: f64 = 900.0;
/// Coats and dresses.
const HANG_LONG: f64 = 1500.0;

pub fn build(ctx: &mut BuildCtx<'_>, spec: &ComponentSpec) -> Result<(), Diagnostic> {
    let ComponentSpec::Rail {
        id,
        carcass,
        bay,
        last_bay,
        zone,
        from_top,
        hardware,
        supports,
        ..
    } = spec
    else {
        unreachable!()
    };
    let id = id.as_str();

    let carcass = ctx.carcass_for(id, carcass.as_ref())?;
    let bays = ctx.bays_for(id, &carcass, bay.as_ref(), last_bay.as_ref())?;
    let spec_zone = zone.as_ref();
    let zone = ctx.zone_for(id, &carcass, zone.as_ref())?;
    if let Some(z) = spec_zone {
        ctx.note_literals(id, &[("zone.from", &z.from), ("zone.to", &z.to)]);
    }
    let from_top = ctx.eval(id, "fromTop", from_top)?;
    let ct = carcass.thickness;
    let z_lo = zone.z0.max(ct);
    let z = zone.z1.min(carcass.height - ct) - from_top;
    let hang = z - z_lo;
    if hang < 100.0 {
        return Err(Diagnostic::new(
            "SPEC-315",
            Severity::Fatal,
            format!(
                "el barral de '{id}' quedaría a {} mm del piso de la bahía: no hay dónde colgar",
                mm(hang)
            ),
        )
        .entity(id));
    }
    let bar = hardware
        .first()
        .and_then(|h| ctx.libs.hardware.get(h))
        .filter(|h| h.kind == "rail")
        .ok_or_else(|| {
            Diagnostic::new(
                "SPEC-316",
                Severity::Fatal,
                format!(
                    "'{id}.hardware' tiene que nombrar un barral (kind 'rail') de la biblioteca"
                ),
            )
            .entity(id)
        })?;
    let bar_id = bar.id.clone();
    if supports.is_empty() {
        return Err(Diagnostic::new(
            "SPEC-316",
            Severity::Fatal,
            format!("'{id}.supports' no dice qué soporte de barral usar"),
        )
        .entity(id));
    }

    // Whatever hangs from the rail takes the bay down to the floor of the
    // hanging height: shelves or drawers in that range collide with it.
    ctx.occupy(
        id,
        OccupancyKind::Rail,
        &carcass,
        &bays,
        Zone {
            z0: (z - HANG_LONG).max(z_lo),
            z1: z,
        },
    );
    if hang < HANG_SHORT {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-114",
                Severity::Warning,
                format!(
                    "'{id}': barral a {} mm del piso de la bahía; una camisa necesita {} y un tapado {}",
                    mm(hang),
                    mm(HANG_SHORT),
                    mm(HANG_LONG)
                ),
            )
            .entity(id)
            .suggestion("Subí el barral (menos 'fromTop') o bajá lo que tiene debajo."),
        );
    }

    let y = (carcass.inner_y0 + carcass.depth) / 2.0;
    let support = JointSpec {
        hardware: supports.clone(),
        placement: None,
    };
    let mut metres = 0.0;
    let mut shortest = f64::INFINITY;
    for bay in &bays {
        let len = bay.x1 - bay.x0;
        metres += len / 1000.0;
        shortest = shortest.min(len);
        for (x, part_id, looks) in [
            (bay.x0, &bay.left_part, Axis::PosX),
            (bay.x1, &bay.right_part, Axis::NegX),
        ] {
            let face = ctx.part_mut(part_id).face_facing(looks);
            ctx.request(
                JointKind::Fixture {
                    centre: Vec3(x, y, z),
                    face,
                    along: Axis::PosZ,
                },
                id,
                part_id,
                part_id,
                &support,
            );
        }
    }
    ctx.extra_bom.push(ExtraBom {
        component: id.to_string(),
        hardware: bar_id,
        quantity: bays.len(),
        metres,
    });
    ctx.publish(id, "height", z);
    ctx.publish(id, "length", shortest);
    ctx.publish(id, "hang", hang);
    Ok(())
}
