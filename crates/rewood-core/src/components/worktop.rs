//! A worktop over one or more carcasses: a desk top on two pedestals, a
//! counter over a run of base units. One panel spanning from the leftmost
//! carcass to the rightmost, overhanging as asked, screwed from inside
//! each carcass through its top panel (face to face). The gap between two
//! pedestals is just open space under the top: nothing is generated
//! there.

use super::{BuildCtx, JointKind, PartInit};
use crate::diagnostics::{Diagnostic, Severity};
use crate::geometry::{Axis, Placement, Vec3};
use crate::model::Grain;
use crate::rules::mm;
use crate::spec::ComponentSpec;

pub fn build(ctx: &mut BuildCtx<'_>, spec: &ComponentSpec) -> Result<(), Diagnostic> {
    let ComponentSpec::Worktop {
        id,
        carcasses,
        overhang,
        material,
        fixing,
        edges,
    } = spec
    else {
        unreachable!()
    };
    let id = id.as_str();

    let ids: Vec<String> = if carcasses.is_empty() {
        ctx.carcasses.keys().cloned().collect()
    } else {
        carcasses.clone()
    };
    if ids.is_empty() {
        return Err(Diagnostic::new(
            "SPEC-202",
            Severity::Fatal,
            format!("la tapa '{id}' necesita al menos una carcasa declarada antes"),
        )
        .entity(id));
    }
    let mut under = Vec::new();
    for cid in &ids {
        under.push(ctx.carcass_for(id, Some(cid))?);
    }
    // Built in furniture space from the carcasses' origins: it must not
    // move again with any one of them.
    ctx.carcass_of.remove(id);
    let front = ctx.eval(id, "overhang.front", &overhang.front)?;
    let back = ctx.eval(id, "overhang.back", &overhang.back)?;
    let sides = ctx.eval(id, "overhang.sides", &overhang.sides)?;
    let material = ctx.material_or_default(id, material.as_ref())?.to_string();
    let t = ctx.thickness_of(&material);

    // Extents in furniture space, over every carcass named.
    let x0 = under
        .iter()
        .map(|c| c.origin.0)
        .fold(f64::INFINITY, f64::min)
        - sides;
    let x1 = under
        .iter()
        .map(|c| c.origin.0 + c.width)
        .fold(f64::NEG_INFINITY, f64::max)
        + sides;
    let y0 = under
        .iter()
        .map(|c| c.origin.1)
        .fold(f64::INFINITY, f64::min)
        - back;
    let y1 = under
        .iter()
        .map(|c| c.origin.1 + c.depth)
        .fold(f64::NEG_INFINITY, f64::max)
        + front;
    let z_top = under
        .iter()
        .map(|c| c.origin.2 + c.height)
        .fold(f64::NEG_INFINITY, f64::max);
    let uneven: Vec<&str> = under
        .iter()
        .filter(|c| (c.origin.2 + c.height - z_top).abs() > crate::units::EPS)
        .map(|c| c.id.as_str())
        .collect();
    if !uneven.is_empty() {
        return Err(Diagnostic::new(
            "SPEC-319",
            Severity::Fatal,
            format!(
                "la tapa '{id}' no apoya en todas las carcasas: {} terminan más abajo que {}",
                uneven.join(", "),
                mm(z_top)
            ),
        )
        .entity(id)
        .suggestion("Igualá las alturas, o dejá afuera de 'carcasses' las que no llegan."));
    }
    let length = x1 - x0;
    let width = y1 - y0;
    if length <= 0.0 || width <= 0.0 {
        return Err(Diagnostic::new(
            "SPEC-319",
            Severity::Fatal,
            format!("la tapa '{id}' queda sin medida ({length}×{width})"),
        )
        .entity(id));
    }
    let banded = super::banded_axes(*edges, &[Axis::PosY, Axis::PosX, Axis::NegX]);
    // Front face (local +Z) looks down at the carcasses: origin at the
    // top-front-left corner, local Y running towards the back.
    let top = ctx.add_part(PartInit {
        name: "Tapa de trabajo".into(),
        component: id,
        role: "worktop",
        material: &material,
        length,
        width,
        grain: Grain::Length,
        placement: Placement::new(Vec3(x0, y1, z_top + t), Axis::PosX, Axis::NegY),
        banded_edges: &banded,
    });
    // Screwed from inside each carcass: its top is drilled through, the
    // worktop takes the thread.
    for c in &under {
        let Some(carcass_top) = ctx
            .parts
            .iter()
            .find(|p| p.component == c.id && p.role == "top")
            .map(|p| p.id.clone())
        else {
            continue;
        };
        ctx.request(JointKind::FaceToFace, id, &carcass_top, &top, fixing);
    }
    ctx.publish(id, "length", length);
    ctx.publish(id, "width", width);
    ctx.publish(id, "height", z_top + t);
    if length > 2400.0 {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-115",
                Severity::Warning,
                format!(
                    "'{id}': tapa de {} mm de largo; más de 2400 no sale de una placa entera",
                    mm(length)
                ),
            )
            .entity(id)
            .suggestion("Partila en dos tapas, una por grupo de carcasas."),
        );
    }
    let free = under
        .windows(2)
        .map(|w| w[1].origin.0 - (w[0].origin.0 + w[0].width))
        .fold(0.0_f64, f64::max);
    if free > 1200.0 {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-115",
                Severity::Warning,
                format!(
                    "'{id}': {} mm de luz entre carcasas bajo la tapa; más de 1200 en {} mm pandea",
                    mm(free),
                    mm(t)
                ),
            )
            .entity(id)
            .suggestion("Acercá las carcasas o poné un travesaño."),
        );
    }
    Ok(())
}
