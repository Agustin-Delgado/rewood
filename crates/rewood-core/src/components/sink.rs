//! A sink on a carcass: the basin (a library item) sits on the top, which
//! gets the openings it asks for — the bowl of an inset basin, or just the
//! drain of a countertop one — and takes the height its bowl hangs into
//! the bay. With `passage`, the back panel gets an opening for the pipes.

use super::{BuildCtx, JointKind, OccupancyKind, Zone};
use crate::diagnostics::{Diagnostic, Severity};
use crate::geometry::{Axis, Vec3};
use crate::rules::mm;
use crate::spec::{ComponentSpec, JointSpec};

/// Room between an opening and the panels bounding its bay: the top's
/// cam holes (Ø15, 34 mm in) sit right there.
const CLEAR: f64 = 45.0;

pub fn build(ctx: &mut BuildCtx<'_>, spec: &ComponentSpec) -> Result<(), Diagnostic> {
    let ComponentSpec::Sink {
        id,
        carcass,
        bay,
        hardware,
        from_front,
        offset,
        passage,
        ..
    } = spec
    else {
        unreachable!()
    };
    let id = id.as_str();
    let carcass = ctx.carcass_for(id, carcass.as_ref())?;
    let bays = ctx.bays_for(id, &carcass, bay.as_ref(), None)?;
    let [bay] = bays.as_slice() else {
        return Err(Diagnostic::new(
            "SPEC-336",
            Severity::Fatal,
            format!("'{id}': una bacha va en una bahía; decí cuál con 'bay'"),
        )
        .entity(id));
    };
    let def = hardware
        .iter()
        .find_map(|h| ctx.libs.hardware.get(h).filter(|d| d.kind == "sink"))
        .ok_or_else(|| {
            Diagnostic::new(
                "SPEC-336",
                Severity::Fatal,
                format!(
                    "'{id}.hardware' tiene que nombrar una bacha de la biblioteca (kind: sink)"
                ),
            )
            .entity(id)
        })?
        .clone();
    let from_front = match from_front {
        Some(v) => ctx.eval(id, "fromFront", v)?,
        None => (carcass.depth - carcass.inner_y0) / 2.0,
    };
    let offset = match offset {
        Some(v) => ctx.eval(id, "offset", v)?,
        None => 0.0,
    };
    let t = carcass.thickness;
    let x = (bay.x0 + bay.x1) / 2.0 + offset;
    let y = carcass.depth - from_front;
    // Every opening has to fall inside the bay, clear of the panels that
    // bound it (the bowl hangs between them) and of the joint holes the
    // top has near them.
    for c in &def.cutouts {
        let (c0, c1) = (
            x + c.offset_along - c.width / 2.0,
            x + c.offset_along + c.width / 2.0,
        );
        if c0 < bay.x0 + CLEAR || c1 > bay.x1 - CLEAR {
            return Err(Diagnostic::new(
                "SPEC-336",
                Severity::Fatal,
                format!(
                    "'{id}': el recorte '{}' de {} ({} mm) no entra en la bahía de {} mm",
                    c.label,
                    def.name,
                    mm(c.width),
                    mm(bay.x1 - bay.x0)
                ),
            )
            .entity(id)
            .suggestion("Una bacha más chica o una bahía más ancha."));
        }
    }
    let top = ctx
        .parts
        .iter()
        .find(|p| p.component == carcass.id && p.role == "top")
        .map(|p| p.id.clone())
        .ok_or_else(|| {
            Diagnostic::new(
                "SPEC-336",
                Severity::Fatal,
                format!(
                    "'{id}': la carcasa '{}' no tiene tapa donde apoyar la bacha",
                    carcass.id
                ),
            )
            .entity(id)
        })?;
    let face = ctx.part_mut(&top).face_facing(Axis::PosZ);
    ctx.request(
        JointKind::Fixture {
            centre: Vec3(x, y, carcass.height),
            face,
            along: Axis::PosX,
        },
        id,
        &top,
        &top,
        &JointSpec {
            hardware: hardware.clone(),
            placement: None,
        },
    );
    // The bowl under the top: nothing else in that height of the bay.
    let below = def.sink.as_ref().map_or(0.0, |s| s.below);
    if below > 0.0 {
        ctx.occupy(
            id,
            OccupancyKind::Sink,
            &carcass,
            std::slice::from_ref(bay),
            Zone {
                z0: carcass.height - t - below,
                z1: carcass.height - t,
            },
        );
    }
    ctx.publish(id, "below", below);

    if let Some(p) = passage {
        let back = ctx
            .parts
            .iter()
            .find(|q| q.component == carcass.id && q.role == "back")
            .map(|q| q.id.clone())
            .ok_or_else(|| {
                Diagnostic::new(
                    "SPEC-336",
                    Severity::Fatal,
                    format!("'{id}.passage': la carcasa '{}' no tiene fondo", carcass.id),
                )
                .entity(id)
            })?;
        let z = match &p.height {
            Some(v) => ctx.eval(id, "passage.height", v)?,
            None => carcass.height / 2.0,
        };
        let face = ctx.part_mut(&back).face_facing(Axis::PosY);
        let y_back = ctx
            .parts
            .iter()
            .find(|q| q.id == back)
            .map_or(0.0, |q| q.aabb.max.1);
        ctx.request(
            JointKind::Fixture {
                centre: Vec3((bay.x0 + bay.x1) / 2.0, y_back, z),
                face,
                along: Axis::PosX,
            },
            id,
            &back,
            &back,
            &JointSpec {
                hardware: p.hardware.clone(),
                placement: None,
            },
        );
    }
    Ok(())
}
