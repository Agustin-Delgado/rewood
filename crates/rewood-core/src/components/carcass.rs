//! Carcass: left side, right side, top, bottom, optional back in a groove.
//!
//! Frames are chosen so every panel's local +Z (its `Front` face) looks
//! into the cabinet — that is where the fastener holes go — and the grain
//! runs vertically on the sides and horizontally on top and bottom.

use super::{Bay, BuildCtx, CarcassInfo, JointKind, PartInit};
use crate::diagnostics::{Diagnostic, Severity};
use crate::geometry::{Axis, Face, Placement, Vec3};
use crate::model::Grain;
use crate::rules::mm;
use crate::spec::{ComponentSpec, JointSpec, LegsSpec};

pub fn build(ctx: &mut BuildCtx<'_>, spec: &ComponentSpec) -> Result<(), Diagnostic> {
    let ComponentSpec::Carcass {
        id,
        width,
        height,
        depth,
        material,
        joint,
        back,
        bays,
        bay_widths,
        edges,
        legs,
        hanging,
        origin,
        ..
    } = spec
    else {
        unreachable!()
    };
    let id = id.as_str();

    ctx.note_literals(
        id,
        &[("width", width), ("height", height), ("depth", depth)],
    );
    let width = ctx.eval(id, "width", width)?;
    let height = ctx.eval(id, "height", height)?;
    let depth = ctx.eval(id, "depth", depth)?;
    let material = ctx.material_or_default(id, material.as_ref())?.to_string();
    let t = ctx.thickness_of(&material);

    if width <= 2.0 * t || height <= 2.0 * t || depth <= 0.0 {
        return Err(Diagnostic::new(
            "SPEC-301",
            Severity::Fatal,
            format!("la carcasa '{id}' no tiene espacio interior: {width}×{height}×{depth} con paneles de {t} mm"),
        )
        .entity(id));
    }

    if width < 150.0 || height < 100.0 || depth < 100.0 {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-108",
                Severity::Warning,
                format!(
                    "la carcasa '{id}' mide {}×{}×{} mm: ¿los valores están en milímetros?",
                    mm(width),
                    mm(height),
                    mm(depth)
                ),
            )
            .entity(id)
            .suggestion("El motor trabaja en mm: 1,8 m se escribe 1800."),
        );
    }
    if depth > 1000.0 {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-108",
                Severity::Warning,
                format!(
                    "la carcasa '{id}' tiene {} mm de profundidad: no se llega al fondo con el brazo",
                    mm(depth)
                ),
            )
            .entity(id)
            .suggestion("Placares 550–650 mm, bajomesadas 560–600, estanterías 300–400."),
        );
    }
    if back.is_none() {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-107",
                Severity::Warning,
                format!(
                    "la carcasa '{id}' no tiene fondo: sin él no queda a escuadra y se deforma al cargarla"
                ),
            )
            .entity(id)
            .suggestion("Agregá back: { material: \"hdf_3\" }, salvo que otra cosa la arriostre.")
            .fix(
                "Agregar fondo HDF 3 mm",
                id,
                "back",
                serde_json::json!({ "material": "hdf_3" }),
            ),
        );
    }

    let inner_width = width - 2.0 * t;
    let inner_height = height - 2.0 * t;
    // Bay layout: explicit widths win over the count; one "auto" entry
    // absorbs whatever the others leave.
    let widths: Vec<f64> = if bay_widths.is_empty() {
        let bay_count = ctx.eval(id, "bays", bays)?;
        if bay_count < 1.0 || bay_count.fract() != 0.0 {
            return Err(Diagnostic::new(
                "SPEC-303",
                Severity::Fatal,
                format!("'{id}.bays' tiene que ser un entero ≥ 1, es {bay_count}"),
            )
            .entity(id));
        }
        let n = bay_count as usize;
        vec![(inner_width - (n as f64 - 1.0) * t) / n as f64; n]
    } else {
        let n = bay_widths.len();
        let mut fixed = Vec::with_capacity(n);
        let mut auto: Option<usize> = None;
        for (k, w) in bay_widths.iter().enumerate() {
            match w {
                crate::params::ParamInput::Expr(s) if s.trim() == "auto" => {
                    if auto.replace(k).is_some() {
                        return Err(Diagnostic::new(
                            "SPEC-309",
                            Severity::Fatal,
                            format!("'{id}.bayWidths' admite un solo \"auto\""),
                        )
                        .entity(id));
                    }
                    fixed.push(0.0);
                }
                other => fixed.push(ctx.eval(id, &format!("bayWidths[{k}]"), other)?),
            }
        }
        let dividers = (n as f64 - 1.0) * t;
        let used: f64 = fixed.iter().sum::<f64>() + dividers;
        match auto {
            Some(k) => fixed[k] = inner_width - used,
            None if (used - inner_width).abs() > 0.01 => {
                return Err(Diagnostic::new(
                    "SPEC-309",
                    Severity::Fatal,
                    format!(
                        "'{id}.bayWidths' suma {} mm más {} de divisores = {}, y el interior mide {inner_width} mm",
                        crate::units::round3(fixed.iter().sum::<f64>()),
                        crate::units::round3(dividers),
                        crate::units::round3(used)
                    ),
                )
                .entity(id)
                .suggestion("Poné \"auto\" en una bahía para que absorba la diferencia."));
            }
            None => {}
        }
        fixed
    };
    let bay_count = widths.len();
    if widths.iter().any(|w| *w <= 0.0) {
        return Err(Diagnostic::new(
            "SPEC-301",
            Severity::Fatal,
            format!("{bay_count} bahías no entran en {inner_width} mm interiores"),
        )
        .entity(id));
    }
    ctx.publish(id, "bays", bay_count as f64);
    // Top and bottom rest on the sides and on every divider: their
    // unsupported span is the widest bay, not the whole carcass.
    let widest = widths.iter().cloned().fold(0.0, f64::max);
    if let Some(m) = ctx.libs.materials.material(&material) {
        let max_span = m.max_span.unwrap_or(50.0 * m.nominal_thickness);
        if widest > max_span + crate::units::EPS {
            ctx.warn(
                Diagnostic::new(
                    "DESIGN-101",
                    Severity::Warning,
                    format!(
                        "tapa y base de '{id}': {} mm de luz en {}, que aguanta {} sin pandear",
                        mm(widest),
                        m.name,
                        mm(max_span)
                    ),
                )
                .entity(id)
                .suggestion("Partí el ancho con un divisor (bays: 2) o usá una placa más gruesa.")
                .fix(
                    format!("Dividir en {} bahías", bay_count + 1),
                    id,
                    "bays",
                    serde_json::json!(bay_count + 1),
                ),
            );
        }
    }
    ctx.publish(
        id,
        "bay_width",
        widths.iter().cloned().fold(f64::INFINITY, f64::min),
    );
    for (k, w) in widths.iter().enumerate() {
        ctx.publish(id, &format!("bay_{}_width", k + 1), *w);
    }
    ctx.publish(id, "width", width);
    ctx.publish(id, "height", height);
    ctx.publish(id, "depth", depth);
    ctx.publish(id, "thickness", t);
    ctx.publish(id, "inner_width", inner_width);
    ctx.publish(id, "inner_height", inner_height);

    let front = super::banded_axes(*edges, &[Axis::PosY]);

    let side_left = ctx.add_part(PartInit {
        name: "Lateral izquierdo".into(),
        component: id,
        role: "side_left",
        material: &material,
        length: height,
        width: depth,
        grain: Grain::Length,
        // Local +Z = +X (inside). Local +Y = -Y, so v = 0 is the front edge.
        placement: Placement::new(Vec3(0.0, depth, 0.0), Axis::PosZ, Axis::NegY),
        banded_edges: &front,
    });
    let side_right = ctx.add_part(PartInit {
        name: "Lateral derecho".into(),
        component: id,
        role: "side_right",
        material: &material,
        length: height,
        width: depth,
        grain: Grain::Length,
        // Local +Z = -X (inside).
        placement: Placement::new(Vec3(width, 0.0, 0.0), Axis::PosZ, Axis::PosY),
        banded_edges: &front,
    });
    let top = ctx.add_part(PartInit {
        name: "Tapa".into(),
        component: id,
        role: "top",
        material: &material,
        length: inner_width,
        width: depth,
        grain: Grain::Length,
        // Local +Z = -Z (inside, looking down).
        placement: Placement::new(Vec3(t, depth, height), Axis::PosX, Axis::NegY),
        banded_edges: &front,
    });
    let bottom = ctx.add_part(PartInit {
        name: "Base".into(),
        component: id,
        role: "bottom",
        material: &material,
        length: inner_width,
        width: depth,
        grain: Grain::Length,
        // Local +Z = +Z (inside, looking up).
        placement: Placement::new(Vec3(t, 0.0, 0.0), Axis::PosX, Axis::PosY),
        banded_edges: &front,
    });

    for horizontal in [&top, &bottom] {
        for side in [&side_left, &side_right] {
            ctx.request_joint(id, horizontal, side, joint);
        }
    }

    let mut inner_y0 = 0.0;
    if let Some(back) = back {
        let back_material = ctx
            .material_or_default(id, Some(&back.material))?
            .to_string();
        let tb = ctx.thickness_of(&back_material);
        let inset = ctx.eval(id, "back.groove.inset", &back.groove.inset)?;
        let groove_depth = ctx.eval(id, "back.groove.depth", &back.groove.depth)?;
        let clearance = ctx.eval(id, "back.groove.clearance", &back.groove.clearance)?;
        let groove_width = tb + clearance;
        if groove_depth >= t {
            return Err(Diagnostic::new(
                "SPEC-302",
                Severity::Fatal,
                format!(
                    "la ranura del fondo de '{id}' ({groove_depth} mm) atraviesa paneles de {t} mm"
                ),
            )
            .entity(id));
        }
        inner_y0 = inset + groove_width;

        // The back sits in the groove with 0.5 mm of play on each side so
        // it never forces the carcass open.
        let play = 0.5;
        let back_len = inner_width + 2.0 * groove_depth - 2.0 * play;
        let back_wid = inner_height + 2.0 * groove_depth - 2.0 * play;
        let back_id = ctx.add_part(PartInit {
            name: "Fondo".into(),
            component: id,
            role: "back",
            material: &back_material,
            length: back_len,
            width: back_wid,
            grain: Grain::None,
            // Local +Z = -Y (outside, looking backwards).
            placement: Placement::new(
                Vec3(
                    t - groove_depth + play,
                    inset + clearance / 2.0 + tb,
                    t - groove_depth + play,
                ),
                Axis::PosX,
                Axis::PosZ,
            ),
            banded_edges: &[],
        });
        ctx.publish(id, "back_length", back_len);
        ctx.publish(id, "back_width", back_wid);

        // Through groove on the inside face of the four carcass panels, at
        // `inset` from the back edge, running the full length of each.
        let y_groove = inset + groove_width / 2.0;
        let grooved: [(&str, Vec3, Vec3); 4] = [
            (
                &side_left,
                Vec3(t, y_groove, 0.0),
                Vec3(t, y_groove, height),
            ),
            (
                &side_right,
                Vec3(width - t, y_groove, 0.0),
                Vec3(width - t, y_groove, height),
            ),
            (
                &top,
                Vec3(t, y_groove, height - t),
                Vec3(width - t, y_groove, height - t),
            ),
            (&bottom, Vec3(t, y_groove, t), Vec3(width - t, y_groove, t)),
        ];
        for (part_id, from, to) in grooved {
            ctx.groove(part_id, from, to, groove_width, groove_depth);
            ctx.part_mut(part_id).overlap_exempt.push(back_id.clone());
        }
        ctx.part_mut(&back_id).overlap_exempt = vec![
            side_left.clone(),
            side_right.clone(),
            top.clone(),
            bottom.clone(),
        ];
    }

    // Dividers between the bays: full inner height, stopping short of the
    // back panel, joined to top and bottom like the sides are.
    let mut bays = Vec::new();
    let mut left_part = side_left.clone();
    let mut x = t;
    for k in 1..=bay_count {
        let is_last = k == bay_count;
        let x1 = x + widths[k - 1];
        let (right_part, cover_x1) = if is_last {
            (side_right.clone(), width)
        } else {
            let divider = ctx.add_part(PartInit {
                name: format!("Divisor {k}"),
                component: id,
                role: &format!("divider_{k}"),
                material: &material,
                length: inner_height,
                width: depth - inner_y0,
                grain: Grain::Length,
                // Same frame as the left side: local +Z looks to +X, so the
                // bay on its right sees `front` and the one on its left `back`.
                placement: Placement::new(Vec3(x1, depth, t), Axis::PosZ, Axis::NegY),
                banded_edges: &front,
            });
            ctx.request_joint(id, &divider, &top, joint);
            ctx.request_joint(id, &divider, &bottom, joint);
            (divider, x1 + t / 2.0)
        };
        bays.push(Bay {
            index: k,
            x0: x,
            x1,
            cover_x0: if k == 1 { 0.0 } else { x - t / 2.0 },
            cover_x1,
            left_part: left_part.clone(),
            right_part: right_part.clone(),
            spans: 1,
        });
        left_part = right_part;
        x = x1 + t;
    }

    if let Some(h) = hanging {
        build_hangers(ctx, id, h, height, t, inner_y0, &side_left, &side_right)?;
    }
    if let Some(legs) = legs {
        let dividers: Vec<f64> = bays.windows(2).map(|w| (w[0].x1 + w[1].x0) / 2.0).collect();
        build_legs(ctx, id, legs, width, depth, t, &bottom, &dividers)?;
    }

    let origin = match origin {
        Some(o) => Vec3(
            ctx.eval(id, "origin.x", &o.x)?,
            ctx.eval(id, "origin.y", &o.y)?,
            ctx.eval(id, "origin.z", &o.z)?,
        ),
        None => Vec3(0.0, 0.0, 0.0),
    };
    ctx.publish(id, "origin_x", origin.0);
    ctx.publish(id, "origin_y", origin.1);
    ctx.publish(id, "origin_z", origin.2);
    ctx.carcass_of.insert(id.to_string(), id.to_string());
    ctx.carcasses.insert(
        id.to_string(),
        CarcassInfo {
            id: id.to_string(),
            origin,
            width,
            height,
            depth,
            thickness: t,
            material,
            inner_y0,
            hanger_clear_z: hanging.as_ref().map(|_| height - t - HANGER_ZONE),
            side_left,
            side_right,
            bays,
        },
    );
    Ok(())
}

/// Height a cabinet hanger takes on the side's inner face, from the top
/// panel down: its body and screws, with room to reach the wall rail.
const HANGER_ZONE: f64 = 70.0;

/// Cabinet hangers: one on each side's inner face, its body up against
/// the top panel and the back, so the hook reaches the wall rail through
/// the back. Nothing else needs that corner. The rail itself goes to the
/// BOM by the metre, one length per cabinet.
#[allow(clippy::too_many_arguments)]
fn build_hangers(
    ctx: &mut BuildCtx<'_>,
    id: &str,
    hanging: &crate::spec::HangingSpec,
    height: f64,
    t: f64,
    inner_y0: f64,
    side_left: &str,
    side_right: &str,
) -> Result<(), Diagnostic> {
    let Some(hw_id) = hanging.hardware.first() else {
        return Err(Diagnostic::new(
            "SPEC-320",
            Severity::Fatal,
            format!("'{id}.hanging' no dice qué colgador usar"),
        )
        .entity(id));
    };
    let Some(hw) = ctx.libs.hardware.get(hw_id).cloned() else {
        return Err(Diagnostic::new(
            "SPEC-320",
            Severity::Fatal,
            format!("'{id}.hanging': el herraje '{hw_id}' no existe en la biblioteca"),
        )
        .entity(id));
    };
    if hw.kind != "hanger" {
        return Err(Diagnostic::new(
            "SPEC-320",
            Severity::Fatal,
            format!("'{id}.hanging': '{hw_id}' no es un colgador (kind 'hanger')"),
        )
        .entity(id));
    }
    // Body centre: 30 mm under the top panel, 40 mm in front of the back.
    let z = height - t - 30.0;
    let y = inner_y0 + 40.0;
    let joint = JointSpec {
        hardware: vec![hw_id.clone()],
        placement: None,
    };
    for (side, looks) in [(side_left, Axis::PosX), (side_right, Axis::NegX)] {
        let face = ctx.part_mut(side).face_facing(looks);
        let x = ctx.part_mut(side).placement.origin.0 + if looks == Axis::PosX { t } else { -t };
        ctx.request(
            JointKind::Fixture {
                centre: Vec3(x, y, z),
                face,
                along: Axis::PosZ,
            },
            id,
            side,
            side,
            &joint,
        );
    }
    ctx.publish(id, "hangers", 2.0);
    Ok(())
}

/// Legs under the bottom panel, in two rows (front and back) at `inset`
/// from the carcass edges: one under each side, one under each divider
/// (that is where the load comes down, and a leg spread evenly instead
/// lands its screws on the divider's joint holes), and as many between
/// as the spec's `maxSpacing` asks. Each leg is a fixture joint on the
/// bottom's underside; the plinth, when asked for, is a panel clipped to
/// the front legs.
#[allow(clippy::too_many_arguments)]
fn build_legs(
    ctx: &mut BuildCtx<'_>,
    id: &str,
    legs: &LegsSpec,
    width: f64,
    depth: f64,
    t: f64,
    bottom: &str,
    dividers: &[f64],
) -> Result<(), Diagnostic> {
    let inset = ctx.eval(id, "legs.inset", &legs.inset)?;
    let max_spacing = ctx.eval(id, "legs.maxSpacing", &legs.max_spacing)?;
    let Some(leg_id) = legs.hardware.first() else {
        return Err(Diagnostic::new(
            "SPEC-312",
            Severity::Fatal,
            format!("'{id}.legs' no dice qué pata usar"),
        )
        .entity(id));
    };
    let Some(leg) = ctx.libs.hardware.get(leg_id).cloned() else {
        return Err(Diagnostic::new(
            "LIB-102",
            Severity::Fatal,
            format!("'{id}.legs': el herraje '{leg_id}' no existe en la biblioteca"),
        )
        .entity(id));
    };
    let Some(leg_data) = leg.leg.clone() else {
        return Err(Diagnostic::new(
            "LIB-103",
            Severity::Fatal,
            format!("'{id}.legs': '{leg_id}' no es una pata (no tiene 'leg')"),
        )
        .entity(id));
    };
    let base_r = leg_data.base_diameter / 2.0;
    if inset < base_r || inset > depth / 2.0 || inset > width / 2.0 {
        return Err(Diagnostic::new(
            "SPEC-312",
            Severity::Fatal,
            format!(
                "'{id}.legs.inset' = {inset} mm: tiene que ser al menos el radio de la base ({base_r}) y menos de media carcasa"
            ),
        )
        .entity(id));
    }
    // The bottom panel runs from t to width − t: a leg has to sit under it.
    if inset - base_r < t {
        return Err(Diagnostic::new(
            "SPEC-312",
            Severity::Fatal,
            format!(
                "'{id}.legs.inset' = {inset} mm deja la base de la pata (Ø{}) fuera de la base de la carcasa, que empieza en {t}",
                leg_data.base_diameter
            ),
        )
        .entity(id));
    }
    let mut anchors = vec![inset];
    anchors.extend(dividers.iter().copied());
    anchors.push(width - inset);
    let mut xs = Vec::new();
    for pair in anchors.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        xs.push(a);
        let gaps = ((b - a) / max_spacing).ceil().max(1.0) as usize;
        for i in 1..gaps {
            xs.push(a + (b - a) * i as f64 / gaps as f64);
        }
    }
    xs.push(width - inset);
    // A plinth stands in front of the front row: the legs step back
    // behind it (base plate against the plinth's inner face) instead of
    // standing through it.
    let plinth_at = match &legs.plinth {
        Some(plinth) => {
            let setback = ctx.eval(id, "legs.plinth.setback", &plinth.setback)?;
            let material = ctx
                .material_or_default(id, plinth.material.as_ref())?
                .to_string();
            let tp = ctx.thickness_of(&material);
            Some((setback, material, tp))
        }
        None => None,
    };
    let front_inset = match &plinth_at {
        Some((setback, _, tp)) => inset.max(setback + tp + base_r),
        None => inset,
    };
    if front_inset + inset + leg_data.base_diameter > depth {
        return Err(Diagnostic::new(
            "SPEC-313",
            Severity::Fatal,
            format!(
                "'{id}': con el zócalo, las patas delanteras van a {front_inset} mm del frente y las traseras a {inset} del fondo; en {depth} mm de profundidad no entran las dos filas"
            ),
        )
        .entity(id));
    }
    let ys = [inset, depth - front_inset];
    let joint = JointSpec {
        hardware: vec![leg_id.clone()],
        placement: None,
    };
    for y in ys {
        for x in &xs {
            ctx.request(
                JointKind::Fixture {
                    centre: Vec3(*x, y, 0.0),
                    face: Face::Back,
                    along: Axis::PosX,
                },
                id,
                bottom,
                bottom,
                &joint,
            );
        }
    }
    ctx.publish(id, "legs", (xs.len() * 2) as f64);
    ctx.publish(id, "leg_height", leg_data.height);

    if let (Some(plinth), Some((setback, material, tp))) = (&legs.plinth, plinth_at) {
        if setback < 0.0 {
            return Err(Diagnostic::new(
                "SPEC-313",
                Severity::Fatal,
                format!("'{id}.legs.plinth.setback' = {setback} mm: el zócalo no puede sobresalir del frente"),
            )
            .entity(id));
        }
        let h = leg_data.height;
        // Between the sides, its inner face looking back at the legs:
        // origin at the bottom-left-front, local +Z = −Y.
        let banded = super::banded_axes(plinth.edges, &[]);
        let plinth_id = ctx.add_part(PartInit {
            name: "Zócalo".into(),
            component: id,
            role: "plinth",
            material: &material,
            length: width - 2.0 * t,
            width: h,
            grain: Grain::Length,
            placement: Placement::new(Vec3(t, depth - setback, -h), Axis::PosX, Axis::PosZ),
            banded_edges: &banded,
        });
        let Some(clip_id) = plinth.clips.first() else {
            return Err(Diagnostic::new(
                "SPEC-313",
                Severity::Fatal,
                format!("'{id}.legs.plinth' no dice qué clip usar"),
            )
            .entity(id));
        };
        let clips = JointSpec {
            hardware: vec![clip_id.clone()],
            placement: None,
        };
        let y_inner = depth - setback - tp;
        for x in &xs {
            ctx.request(
                JointKind::Fixture {
                    centre: Vec3(*x, y_inner, -h / 2.0),
                    face: Face::Front,
                    along: Axis::PosX,
                },
                id,
                &plinth_id,
                &plinth_id,
                &clips,
            );
        }
        ctx.publish(id, "plinth_length", width - 2.0 * t);
        ctx.publish(id, "plinth_height", h);
    }
    Ok(())
}
