//! A free-standing support panel: the end of a desk that has no pedestal
//! on that side. One vertical panel on the floor, full depth; the worktop
//! resting on it is joined to its top edge. It holds nothing sideways by
//! itself: a modesty panel (or a carcass) has to brace it.

use super::{BuildCtx, PanelInfo, PartInit};
use crate::diagnostics::{Diagnostic, Severity};
use crate::geometry::{Axis, Placement, Vec3};
use crate::model::Grain;
use crate::rules::mm;
use crate::spec::{ComponentSpec, PanelFacing};

pub fn build(ctx: &mut BuildCtx<'_>, spec: &ComponentSpec) -> Result<(), Diagnostic> {
    let ComponentSpec::Panel {
        id,
        x,
        facing,
        y,
        z,
        depth,
        height,
        material,
        joint,
        edges,
        ..
    } = spec
    else {
        unreachable!()
    };
    let id = id.as_str();

    ctx.note_literals(id, &[("depth", depth), ("height", height)]);
    let x = ctx.eval(id, "x", x)?;
    let y = ctx.eval(id, "y", y)?;
    let z = ctx.eval(id, "z", z)?;
    let depth = ctx.eval(id, "depth", depth)?;
    let height = ctx.eval(id, "height", height)?;
    if depth <= 0.0 || height <= 0.0 {
        return Err(Diagnostic::new(
            "SPEC-322",
            Severity::Fatal,
            format!(
                "el lateral '{id}' queda sin medida ({depth}×{height})",
                depth = mm(depth),
                height = mm(height)
            ),
        )
        .entity(id));
    }
    let material = ctx.material_or_default(id, material.as_ref())?.to_string();
    let t = ctx.thickness_of(&material);
    let banded = super::banded_axes(*edges, &[Axis::PosY]);

    // Same frames as a carcass's sides: local +Z (the drilled face) looks
    // at the inside of the desk.
    let (x0, name, placement) = match facing {
        PanelFacing::Right => (
            x,
            "Lateral de apoyo izquierdo",
            Placement::new(Vec3(x, y + depth, z), Axis::PosZ, Axis::NegY),
        ),
        PanelFacing::Left => (
            x - t,
            "Lateral de apoyo derecho",
            Placement::new(Vec3(x, y, z), Axis::PosZ, Axis::PosY),
        ),
    };
    let part = ctx.add_part(PartInit {
        name: name.into(),
        component: id,
        role: "panel",
        material: &material,
        length: height,
        width: depth,
        grain: Grain::Length,
        placement,
        banded_edges: &banded,
    });
    ctx.panels.insert(
        id.to_string(),
        PanelInfo {
            part,
            x0,
            x1: x0 + t,
            y0: y,
            y1: y + depth,
            z_top: z + height,
            joint: joint.clone(),
        },
    );
    ctx.publish(id, "height", height);
    ctx.publish(id, "depth", depth);
    ctx.publish(id, "thickness", t);
    Ok(())
}
