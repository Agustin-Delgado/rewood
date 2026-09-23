//! A modesty panel: a vertical panel under a worktop, in each gap between
//! the carcasses and panels holding it up, near their back and right
//! under the top. Butted into the supports at both ends, it is what keeps
//! a desk on two end panels from racking sideways.

use super::{BuildCtx, PartInit};
use crate::diagnostics::{Diagnostic, Severity};
use crate::geometry::{Axis, Placement, Vec3};
use crate::model::Grain;
use crate::rules::mm;
use crate::spec::ComponentSpec;

/// A gap narrower than this between two supports is not a knee space.
const MIN_GAP: f64 = 100.0;

pub fn build(ctx: &mut BuildCtx<'_>, spec: &ComponentSpec) -> Result<(), Diagnostic> {
    let ComponentSpec::Modesty {
        id,
        worktop,
        height,
        inset,
        material,
        joint,
        edges,
        ..
    } = spec
    else {
        unreachable!()
    };
    let id = id.as_str();

    let top = match worktop {
        Some(w) => ctx.worktops.get(w).cloned().ok_or_else(|| {
            Diagnostic::new(
                "SPEC-323",
                Severity::Fatal,
                format!("el faldón '{id}' refiere a una tapa '{w}' que no existe"),
            )
            .entity(id)
        })?,
        None => match ctx.worktops.len() {
            1 => ctx.worktops.values().next().cloned().unwrap(),
            0 => {
                return Err(Diagnostic::new(
                    "SPEC-323",
                    Severity::Fatal,
                    format!("el faldón '{id}' necesita una tapa declarada antes"),
                )
                .entity(id))
            }
            _ => {
                return Err(Diagnostic::new(
                    "SPEC-323",
                    Severity::Fatal,
                    format!(
                        "hay varias tapas; el faldón '{id}' tiene que decir cuál con 'worktop'"
                    ),
                )
                .entity(id))
            }
        },
    };
    ctx.braced.insert(top.id.clone());
    let height = ctx.eval(id, "height", height)?;
    let inset = ctx.eval(id, "inset", inset)?;
    if height <= 0.0 || height >= top.z_under {
        return Err(Diagnostic::new(
            "SPEC-323",
            Severity::Fatal,
            format!(
                "el faldón '{id}' mide {} de alto y bajo la tapa hay {}",
                mm(height),
                mm(top.z_under)
            ),
        )
        .entity(id));
    }
    let material = ctx.material_or_default(id, material.as_ref())?.to_string();
    let t = ctx.thickness_of(&material);
    let banded = super::banded_axes(*edges, &[Axis::NegZ]);

    let gaps: Vec<_> = top
        .supports
        .windows(2)
        .filter(|w| w[1].x0 - w[0].x1 >= MIN_GAP)
        .map(|w| (w[0].clone(), w[1].clone()))
        .collect();
    if gaps.is_empty() {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-117",
                Severity::Warning,
                format!(
                    "'{id}': entre los apoyos de '{}' no queda hueco para un faldón",
                    top.id
                ),
            )
            .entity(id),
        );
        return Ok(());
    }
    if inset < 0.0 {
        return Err(Diagnostic::new(
            "SPEC-323",
            Severity::Fatal,
            format!(
                "el faldón '{id}' tiene 'inset' {} mm: va entre los apoyos, no detrás",
                mm(inset)
            ),
        )
        .entity(id));
    }
    let many = gaps.len() > 1;
    for (n, (a, b)) in gaps.iter().enumerate() {
        let back = a.y0.max(b.y0) + inset;
        if back + t > a.y1.min(b.y1) {
            return Err(Diagnostic::new(
                "SPEC-323",
                Severity::Fatal,
                format!(
                    "el faldón '{id}' queda a {} del fondo y los apoyos miden {} de profundidad",
                    mm(inset),
                    mm(a.y1.min(b.y1) - a.y0.max(b.y0))
                ),
            )
            .entity(id));
        }
        // Local +Z looks back (-Y): the fastener holes go on the face
        // nobody sees from the chair.
        let part = ctx.add_part(PartInit {
            name: if many {
                format!("Faldón {}", n + 1)
            } else {
                "Faldón".into()
            },
            component: id,
            role: "modesty",
            material: &material,
            length: b.x0 - a.x1,
            width: height,
            grain: Grain::Length,
            placement: Placement::new(
                Vec3(a.x1, back + t, top.z_under - height),
                Axis::PosX,
                Axis::PosZ,
            ),
            banded_edges: &banded,
        });
        ctx.request_joint(id, &part, &a.right_part, joint);
        ctx.request_joint(id, &part, &b.left_part, joint);
    }
    ctx.publish(id, "height", height);
    Ok(())
}
