//! Overlay doors across the front of a carcass bay. Local +Z (the `Front`
//! face) looks at the carcass: that is where hinge cups are bored.
//!
//! With one door per bay the hinge side is the bay's left panel; with two,
//! each door hangs on its outer panel. More than two per bay would need
//! something to hang the inner ones on.
//!
//! A door over a side overlays the whole side (minus the gap); a door next
//! to a divider overlays half of it, so two neighbouring doors meet in the
//! middle of the divider with one gap between them.

use super::{BuildCtx, JointKind, PartInit};
use crate::diagnostics::{Diagnostic, Severity};
use crate::geometry::{Axis, Placement, Vec3};
use crate::model::Grain;
use crate::spec::ComponentSpec;

pub fn build(ctx: &mut BuildCtx<'_>, spec: &ComponentSpec) -> Result<(), Diagnostic> {
    let ComponentSpec::Doors {
        id,
        carcass,
        bay,
        zone,
        count,
        gap,
        material,
        hinge,
        handle,
        edges,
    } = spec
    else {
        unreachable!()
    };
    let id = id.as_str();

    let carcass = ctx.carcass_for(id, carcass.as_ref())?;
    let bays = ctx.bays_for(id, &carcass, bay.as_ref())?;
    let zone = ctx.zone_for(id, &carcass, zone.as_ref())?;
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
    let min_span = bays
        .iter()
        .map(|b| b.cover_x1 - b.cover_x0)
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

    let all_edges = super::banded_axes(*edges, &[Axis::PosX, Axis::NegX, Axis::PosZ, Axis::NegZ]);
    let many = bays.len() > 1;
    let mut n = 0;
    for bay in &bays {
        let span = bay.cover_x1 - bay.cover_x0;
        let width = (span - gap * (count as f64 + 1.0)) / count as f64;
        for i in 0..count {
            n += 1;
            let x_left = bay.cover_x0 + gap + i as f64 * (width + gap);
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
                    Vec3(x_left + width, carcass.depth + t, zone.z0 + gap),
                    Axis::PosZ,
                    Axis::NegX,
                ),
                banded_edges: &all_edges,
            });
            let (hinge_edge, panel) = if i == 0 {
                (Axis::NegX, &bay.left_part)
            } else {
                (Axis::PosX, &bay.right_part)
            };
            if let Some(hinge) = hinge {
                ctx.request(JointKind::Hinge { hinge_edge }, id, &door, panel, hinge);
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
                let centre = Vec3(x, carcass.depth, z);
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
