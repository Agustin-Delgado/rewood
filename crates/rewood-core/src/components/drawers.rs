//! Drawers stacked from the bottom of a zone of a carcass bay. Each drawer is an overlay
//! front plus a box: two sides running on slides, a front and a back butted
//! between the sides, and a thin bottom housed in a groove.
//!
//! Box sides have their local `Front` facing *outwards* (towards the
//! carcass side): that is where the slide is screwed. The box front and
//! back have it facing the inside of the box, like a carcass.
//!
//! What is not modelled yet: fixing the front panel to the box front (a
//! face-to-face joint) and the handle.

use super::{BuildCtx, JointKind, OccupancyKind, PartInit};
use crate::diagnostics::{Diagnostic, Severity};
use crate::geometry::{Axis, Placement, Vec3};
use crate::model::Grain;
use crate::rules::mm;
use crate::spec::{ComponentSpec, EdgeBanding, FrontMount};

/// How far a handle stands off its front (a bar with its posts, a knob).
const HANDLE_REACH: f64 = 30.0;

pub fn build(ctx: &mut BuildCtx<'_>, spec: &ComponentSpec) -> Result<(), Diagnostic> {
    let ComponentSpec::Drawers {
        id,
        carcass,
        bay,
        last_bay,
        zone,
        count,
        front_height,
        gap,
        mount,
        setback,
        box_height,
        material,
        box_material,
        bottom_material,
        bottom_groove,
        joint,
        slide,
        soft_close,
        front_fixing,
        handle,
        files,
        edges,
        ..
    } = spec
    else {
        unreachable!()
    };
    let id = id.as_str();
    // The damped (or plain) variant of the slide the spec names.
    let slide = &ctx.slide_variant(id, slide, *soft_close)?;

    let carcass = ctx.carcass_for(id, carcass.as_ref())?;
    let bays = ctx.bays_for(id, &carcass, bay.as_ref(), last_bay.as_ref())?;
    let spec_zone = zone.as_ref();
    let zone = ctx.zone_for(id, &carcass, zone.as_ref())?;
    // Inner drawers sit inside the opening, between top and bottom.
    let inner = *mount == FrontMount::Inset;
    let zone = if inner {
        super::Zone {
            z0: zone.z0.max(carcass.thickness),
            z1: zone.z1.min(carcass.height - carcass.thickness),
        }
    } else {
        zone
    };
    let setback = match setback {
        Some(s) if inner => ctx.eval(id, "setback", s)?,
        _ => 0.0,
    };
    // An inner drawer sits behind a door: a handle on it has to fit in the
    // setback, or it runs into the door as it closes.
    if inner && handle.is_some() && setback < HANDLE_REACH {
        return Err(Diagnostic::new(
            "SPEC-324",
            Severity::Fatal,
            format!(
                "los cajones interiores de '{id}' llevan tirador, que sobresale unos {} mm, y el retiro es de {}: choca con la puerta",
                mm(HANDLE_REACH),
                mm(setback)
            ),
        )
        .entity(id)
        .suggestion("Sacale el tirador (se abren tomando el canto del frente) o retiralos más."));
    }
    if setback < 0.0 {
        return Err(Diagnostic::new(
            "SPEC-304",
            Severity::Fatal,
            format!("'{id}.setback' = {} mm: no puede ser negativo", mm(setback)),
        )
        .entity(id));
    }
    // Front plane, from the carcass front: overlay fronts stand in front
    // of it, inner fronts inside, set back or not.
    let front_material_id = ctx.material_or_default(id, material.as_ref())?.to_string();
    let tf_plane = ctx.thickness_of(&front_material_id);
    let front_y = if inner {
        (carcass.depth - setback - tf_plane, carcass.depth - setback)
    } else {
        (carcass.depth, carcass.depth + tf_plane)
    };
    {
        let mut fields: Vec<(&str, &crate::params::ParamInput)> = Vec::new();
        if let Some(z) = spec_zone {
            fields.push(("zone.from", &z.from));
            fields.push(("zone.to", &z.to));
        }
        if let Some(v) = front_height {
            fields.push(("frontHeight", v));
        }
        if let Some(v) = box_height {
            fields.push(("boxHeight", v));
        }
        ctx.note_literals(id, &fields);
    }
    let zone_height = zone.z1 - zone.z0;
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
    let gap = ctx.eval(id, "gap", gap)?;
    if gap < 0.0 {
        return Err(Diagnostic::new(
            "SPEC-304",
            Severity::Fatal,
            format!("'{id}.gap' = {} mm: no puede ser negativo", mm(gap)),
        )
        .entity(id));
    }
    let front_material = ctx.material_or_default(id, material.as_ref())?.to_string();
    let box_material = ctx
        .material_or_default(id, box_material.as_ref())?
        .to_string();
    let bottom_material = ctx
        .material_or_default(id, Some(bottom_material))?
        .to_string();
    let tf = ctx.thickness_of(&front_material);
    let tb = ctx.thickness_of(&box_material);
    let tbot = ctx.thickness_of(&bottom_material);

    // The slide decides the box depth and the clearance to the carcass.
    let slide_def = slide
        .hardware
        .iter()
        .find_map(|h| ctx.libs.hardware.get(h).and_then(|d| d.slide.clone()))
        .ok_or_else(|| {
            Diagnostic::new(
                "SPEC-308",
                Severity::Fatal,
                format!("'{id}.slide' tiene que nombrar un herraje de tipo corredera con su bloque 'slide'"),
            )
            .entity(id)
        })?;

    // Half a gap where the zone meets another front inside the carcass.
    let (gap_lo, gap_hi) = super::zone_gaps(!inner, zone, carcass.height, gap);
    let front_height_spec = front_height.as_ref();
    let front_height = match front_height {
        Some(v) => ctx.eval(id, "frontHeight", v)?,
        None => (zone_height - gap_lo - gap_hi - gap * (count as f64 - 1.0)) / count as f64,
    };
    // What the fronts cover, not the whole zone: with an explicit
    // `frontHeight` the rest of the zone stays open (and free for
    // something else), and the layout check sees it.
    let occupied = super::Zone {
        z0: zone.z0,
        z1: (zone.z0 + gap_lo + count as f64 * front_height + (count as f64 - 1.0) * gap + gap_hi)
            .min(zone.z1),
    };
    ctx.occupy_front(
        id,
        if inner {
            OccupancyKind::InnerDrawers
        } else {
            OccupancyKind::Drawers
        },
        &carcass,
        &bays,
        occupied,
        Some(front_y),
    );
    let box_height = match box_height {
        Some(v) => ctx.eval(id, "boxHeight", v)?,
        None => front_height - 40.0,
    };
    let cover = |b: &super::Bay| {
        if inner {
            (b.x0, b.x1)
        } else {
            (b.cover_x0, b.cover_x1)
        }
    };
    let min_span = bays
        .iter()
        .map(|b| {
            let (a, z) = cover(b);
            z - a
        })
        .fold(f64::INFINITY, f64::min);
    let front_width = min_span - 2.0 * gap;
    if front_height <= 0.0 || box_height <= 2.0 * tbot + 20.0 || front_width <= 0.0 {
        return Err(Diagnostic::new(
            "SPEC-305",
            Severity::Fatal,
            format!(
                "{count} cajones con frentes de {front_height} mm no entran en {zone_height} mm",
                front_height = mm(front_height),
                zone_height = mm(zone_height)
            ),
        )
        .entity(id));
    }
    if gap_lo + gap_hi + count as f64 * front_height + (count as f64 - 1.0) * gap
        > zone_height + crate::units::EPS
    {
        return Err(Diagnostic::new(
            "SPEC-305",
            Severity::Fatal,
            format!(
                "{count} frentes de {front_height} mm con {gap} mm de luz suman más que la zona ({zone_height} mm)", front_height = mm(front_height), gap = mm(gap), zone_height = mm(zone_height)),
        )
        .entity(id));
    }

    let min_bay = bays
        .iter()
        .map(|b| b.x1 - b.x0)
        .fold(f64::INFINITY, f64::min);
    let box_outer_width = min_bay - 2.0 * slide_def.side_clearance;
    let box_depth = slide_def.length;
    // An inner drawer's front takes its own thickness off the depth.
    let available_depth = carcass.depth - carcass.inner_y0 - if inner { tf + setback } else { 0.0 };
    if box_depth + 10.0 > available_depth {
        return Err(Diagnostic::new(
            "SPEC-307",
            Severity::Fatal,
            format!(
                "la corredera de {box_depth} mm no entra en los {available_depth} mm útiles de profundidad (hacen falta 10 mm de holgura atrás)", box_depth = mm(box_depth), available_depth = mm(available_depth)),
        )
        .entity(id)
        .suggestion("Elegí una corredera más corta o una carcasa más profunda.")
        .fix_opt(
            // The longest slide of the same style and damping in the
            // library that fits, if any.
            ctx.libs
                .hardware
                .iter()
                .filter(|h| h.kind == "slide")
                .filter(|h| {
                    h.slide.as_ref().is_some_and(|s| {
                        s.length + 10.0 <= available_depth
                            && s.style == slide_def.style
                            && s.soft_close == slide_def.soft_close
                    })
                })
                .max_by(|a, b| {
                    a.slide
                        .as_ref()
                        .unwrap()
                        .length
                        .partial_cmp(&b.slide.as_ref().unwrap().length)
                        .unwrap()
                })
                .map(|h| {
                    (
                        format!("Usar {}", h.name),
                        id.to_string(),
                        "slide.hardware".to_string(),
                        serde_json::json!([h.id]),
                    )
                }),
        ));
    }
    let inner_length = box_outer_width - 2.0 * tb;
    if inner_length <= 0.0 {
        return Err(Diagnostic::new(
            "SPEC-305",
            Severity::Fatal,
            format!(
                "la carcasa de {} mm es muy angosta para un cajón con correderas",
                carcass.width
            ),
        )
        .entity(id));
    }

    let inset = ctx.eval(id, "bottomGroove.inset", &bottom_groove.inset)?;
    let groove_depth = ctx.eval(id, "bottomGroove.depth", &bottom_groove.depth)?;
    let clearance = ctx.eval(id, "bottomGroove.clearance", &bottom_groove.clearance)?;
    let groove_width = tbot + clearance;
    if inset < 0.0 || groove_depth <= 0.0 || inset + groove_width + 10.0 > box_height {
        return Err(Diagnostic::new(
            "SPEC-302",
            Severity::Fatal,
            format!(
                "la ranura del fondo del cajón de '{id}' (a {} del canto, {} de ancho, {} de profundidad) no cae dentro de una caja de {} mm de alto",
                mm(inset),
                mm(groove_width),
                mm(groove_depth),
                mm(box_height)
            ),
        )
        .entity(id));
    }
    if groove_depth >= tb {
        return Err(Diagnostic::new(
            "SPEC-302",
            Severity::Fatal,
            format!(
                "la ranura del fondo del cajón ({groove_depth} mm) atraviesa paneles de {tb} mm",
                groove_depth = mm(groove_depth),
                tb = mm(tb)
            ),
        )
        .entity(id));
    }

    // Hanging files: the box as wide inside as the folders' hooks reach
    // and as tall as a folder.
    let files = match files {
        Some(f) => {
            let def = f
                .hardware
                .iter()
                .find_map(|h| ctx.libs.hardware.get(h)?.files.clone())
                .ok_or_else(|| {
                    Diagnostic::new(
                        "SPEC-339",
                        Severity::Fatal,
                        format!("'{id}.files' tiene que nombrar rieles para carpetas colgantes (kind: file_rails)"),
                    )
                    .entity(id)
                })?;
            if inner_length < def.min_inner - crate::units::EPS
                || inner_length > def.max_inner + crate::units::EPS
            {
                // How wide the carcass has to be for the box to come out
                // in the middle of the folders' span.
                let want = (def.min_inner + def.max_inner) / 2.0;
                let fix = (!carcass.bays.is_empty() && carcass.bays.len() == 1).then(|| {
                    (
                        format!(
                            "Carcasa de {} mm",
                            mm((carcass.width + want - inner_length).round())
                        ),
                        carcass.id.clone(),
                        "width".to_string(),
                        serde_json::json!((carcass.width + want - inner_length).round()),
                    )
                });
                return Err(Diagnostic::new(
                    "SPEC-339",
                    Severity::Fatal,
                    format!(
                        "'{id}': la caja mide {} por dentro y las carpetas colgantes piden entre {} y {}",
                        mm(inner_length),
                        mm(def.min_inner),
                        mm(def.max_inner)
                    ),
                )
                .entity(id)
                .suggestion("Ajustá el ancho de la carcasa (o de la bahía).")
                .fix_opt(fix));
            }
            if box_height < def.min_height - crate::units::EPS {
                return Err(Diagnostic::new(
                    "SPEC-339",
                    Severity::Fatal,
                    format!(
                        "'{id}': cajas de {} mm de alto; una carpeta colgante pide {}",
                        mm(box_height),
                        mm(def.min_height)
                    ),
                )
                .entity(id)
                .suggestion("Menos cajones, o una caja más alta ('boxHeight').")
                .fix(
                    format!("Caja de {} mm", mm(def.min_height)),
                    id,
                    "boxHeight",
                    serde_json::json!(def.min_height),
                ));
            }
            Some(f.clone())
        }
        None => None,
    };
    ctx.publish(id, "count", count as f64);
    ctx.publish(id, "front_height", front_height);
    ctx.publish(id, "front_width", front_width);
    ctx.publish(id, "box_height", box_height);
    ctx.publish(id, "box_depth", box_depth);
    ctx.publish(id, "box_width", box_outer_width);

    if front_height < 100.0 {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-104",
                Severity::Warning,
                format!(
                    "'{id}': frentes de {} mm; con menos de 100 no entra la mano ni la manija",
                    mm(front_height)
                ),
            )
            .entity(id)
            .suggestion("Menos cajones, o una zona más alta.")
            .fix_opt((count > 1 && front_height_spec.is_none()).then(|| {
                (
                    format!("{} cajones", count - 1),
                    id.to_string(),
                    "count".to_string(),
                    serde_json::json!(count - 1),
                )
            })),
        );
    }
    let slide_room = 2.0 * slide_def.axis_from_box_bottom + 15.0;
    if box_height < slide_room {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-104",
                Severity::Warning,
                format!(
                    "'{id}': cajas de {} mm de alto; la corredera lleva su eje a {} mm del fondo y pide laterales de al menos {} mm",
                    mm(box_height),
                    mm(slide_def.axis_from_box_bottom),
                    mm(slide_room)
                ),
            )
            .entity(id)
            .suggestion("Subí 'boxHeight' o el alto de los frentes."),
        );
    }
    if box_outer_width > 900.0 {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-105",
                Severity::Warning,
                format!(
                    "'{id}': cajones de {} mm de ancho; más de 900 pandean y traban las correderas",
                    mm(box_outer_width)
                ),
            )
            .entity(id)
            .suggestion("Partí la carcasa en bahías.")
            .fix(
                "Carcasa en 2 bahías",
                carcass.id.clone(),
                "bays",
                serde_json::json!(2),
            ),
        );
    }
    if gap < 1.5 {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-103",
                Severity::Warning,
                format!(
                    "'{id}': {} mm de luz entre frentes; con menos de 1,5 mm rozan al abrir",
                    mm(gap)
                ),
            )
            .entity(id)
            .suggestion("Usá 2–3 mm de luz.")
            .fix("Luz de 2 mm", id, "gap", serde_json::json!(2)),
        );
    }
    // On a front, `front` names its face, not an edge: nothing gets banded.
    if matches!(*edges, EdgeBanding::None | EdgeBanding::Front) {
        ctx.warn(
            Diagnostic::new(
                "DESIGN-109",
                Severity::Warning,
                format!("'{id}': frentes sin canto; los bordes de placa quedan a la vista"),
            )
            .entity(id)
            .suggestion("Dejá 'all' (o sacá 'edges'); 'front' en un frente no cantea nada, su cara es la que mira adelante.")
            .fix("Cantear los frentes", id, "edges", serde_json::Value::Null),
        );
    }

    let front_edges = super::banded_axes(*edges, &[Axis::PosX, Axis::NegX, Axis::PosZ, Axis::NegZ]);
    let box_top = [Axis::PosZ];
    // Box front face: flush with the carcass front, or behind an inner
    // drawer's front panel.
    let y_front = if inner {
        carcass.depth - setback - tf
    } else {
        carcass.depth
    };
    let y_back = y_front - box_depth;
    // Front panel origin (its local +Z looks at the box, so it extends
    // towards -Y from here).
    let y_panel = if inner {
        carcass.depth - setback
    } else {
        carcass.depth + tf
    };
    let play = 0.5;
    let many = bays.len() > 1;

    // Spacers the library has, thinnest first.
    let mut spacers: Vec<(f64, String)> = ctx
        .libs
        .hardware
        .iter()
        .filter(|h| h.kind == "spacer")
        .filter_map(|h| Some((h.spacer.as_ref()?.thickness, h.id.clone())))
        .collect();
    spacers.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    for bay in &bays {
        // Inner drawers behind a door hung on this bay's side panel sit on
        // spacers there, so their fronts clear the door swung open: the
        // thinnest that does, or the thickest there is (and DESIGN-118
        // still says it is not enough).
        let spacer_on = |ctx: &BuildCtx<'_>, panel: &str| -> Option<(f64, String)> {
            if !inner {
                return None;
            }
            let panel = ctx.parts.iter().find(|p| p.id == panel)?;
            let need = *ctx.spacers.get(&super::spacer_key(id, panel))?;
            spacers
                .iter()
                .find(|(t, _)| *t >= need - crate::units::EPS)
                .or(spacers.last())
                .cloned()
        };
        let spacer_left = spacer_on(ctx, &bay.left_part);
        let spacer_right = spacer_on(ctx, &bay.right_part);
        let (s0, s1) = (
            spacer_left.as_ref().map_or(0.0, |s| s.0),
            spacer_right.as_ref().map_or(0.0, |s| s.0),
        );
        // The spacer goes into the slide joint: one per slide, in the BOM
        // and in the plan next to the slide it carries.
        let mut slide_on = [slide.clone(), slide.clone()];
        for ((side, spacer), joint) in [("left", &spacer_left), ("right", &spacer_right)]
            .into_iter()
            .zip(&mut slide_on)
        {
            if let Some((t, hardware)) = spacer {
                ctx.publish(id, &format!("spacer_{side}"), *t);
                joint.hardware.push(hardware.clone());
            }
        }
        let x0 = bay.x0 + s0 + slide_def.side_clearance; // box left outer face
        let box_outer_width = (bay.x1 - bay.x0) - s0 - s1 - 2.0 * slide_def.side_clearance;
        let inner_length = box_outer_width - 2.0 * tb;
        // Each bay's front covers that bay: an end bay reaches over the
        // carcass side, a middle one only to the dividers' centres.
        let (c0, c1) = cover(bay);
        let (c0, c1) = (c0 + s0, c1 - s1);
        let front_width = c1 - c0 - 2.0 * gap;
        if inner_length <= 0.0 || front_width <= 0.0 {
            return Err(Diagnostic::new(
                "SPEC-305",
                Severity::Fatal,
                format!(
                    "la bahía {} es muy angosta para cajones interiores con distanciadores de {} mm",
                    bay.index,
                    mm(s0.max(s1))
                ),
            )
            .entity(id));
        }
        let tag = if many {
            format!(" bahía {}", bay.index)
        } else {
            String::new()
        };
        let prefix = if many {
            format!("bay{}_", bay.index)
        } else {
            String::new()
        };
        for i in 0..count {
            let n = i + 1;
            let z_front = zone.z0 + gap_lo + i as f64 * (front_height + gap);
            let z_box = z_front + (front_height - box_height) / 2.0;

            let front = ctx.add_part(PartInit {
                name: format!("Frente cajón {n}{tag}"),
                component: id,
                role: &format!("{prefix}drawer_{n}_front"),
                material: &front_material,
                length: front_width,
                width: front_height,
                grain: Grain::Length,
                // Local +Z = -Y: the back of the front panel looks at the box.
                placement: Placement::new(Vec3(c0 + gap, y_panel, z_front), Axis::PosX, Axis::PosZ),
                banded_edges: &front_edges,
            });

            // Box sides: length along the depth, width = box height, Front outwards.
            let side_left = ctx.add_part(PartInit {
                name: format!("Lateral cajón {n} izq.{tag}"),
                component: id,
                role: &format!("{prefix}drawer_{n}_side_left"),
                material: &box_material,
                length: box_depth,
                width: box_height,
                grain: Grain::Length,
                // X = -Y (front→back), Y = +Z, so +Z = -X: outwards on the left.
                placement: Placement::new(Vec3(x0 + tb, y_front, z_box), Axis::NegY, Axis::PosZ),
                banded_edges: &box_top,
            });
            let side_right = ctx.add_part(PartInit {
                name: format!("Lateral cajón {n} der.{tag}"),
                component: id,
                role: &format!("{prefix}drawer_{n}_side_right"),
                material: &box_material,
                length: box_depth,
                width: box_height,
                grain: Grain::Length,
                // X = +Y (back→front), Y = +Z, so +Z = +X: outwards on the right.
                placement: Placement::new(
                    Vec3(x0 + box_outer_width - tb, y_back, z_box),
                    Axis::PosY,
                    Axis::PosZ,
                ),
                banded_edges: &box_top,
            });
            // Box front and back between the sides, Front facing the inside.
            let box_front = ctx.add_part(PartInit {
                name: format!("Frente interior cajón {n}{tag}"),
                component: id,
                role: &format!("{prefix}drawer_{n}_box_front"),
                material: &box_material,
                length: inner_length,
                width: box_height,
                grain: Grain::Length,
                // X = +X, Y = +Z, so +Z = -Y: looks into the box.
                placement: Placement::new(Vec3(x0 + tb, y_front, z_box), Axis::PosX, Axis::PosZ),
                banded_edges: &box_top,
            });
            let box_back = ctx.add_part(PartInit {
                name: format!("Trasera cajón {n}{tag}"),
                component: id,
                role: &format!("{prefix}drawer_{n}_box_back"),
                material: &box_material,
                length: inner_length,
                width: box_height,
                grain: Grain::Length,
                // X = -X, Y = +Z, so +Z = +Y: looks into the box.
                placement: Placement::new(
                    Vec3(x0 + box_outer_width - tb, y_back, z_box),
                    Axis::NegX,
                    Axis::PosZ,
                ),
                banded_edges: &box_top,
            });
            for panel in [&box_front, &box_back] {
                for side in [&side_left, &side_right] {
                    ctx.request_joint(id, panel, side, joint);
                }
            }

            // Bottom in a groove around the four box panels, `inset` above the
            // box bottom edge.
            let bottom_len = inner_length + 2.0 * groove_depth - 2.0 * play;
            let bottom_wid = (box_depth - 2.0 * tb) + 2.0 * groove_depth - 2.0 * play;
            let z_groove = z_box + inset + groove_width / 2.0;
            let bottom = ctx.add_part(PartInit {
                name: format!("Fondo cajón {n}{tag}"),
                component: id,
                role: &format!("{prefix}drawer_{n}_bottom"),
                material: &bottom_material,
                length: bottom_len,
                width: bottom_wid,
                grain: Grain::None,
                // X = +X, Y = +Y, so +Z = +Z: looks up into the box.
                placement: Placement::new(
                    Vec3(
                        x0 + tb - groove_depth + play,
                        y_back + tb - groove_depth + play,
                        z_groove - tbot / 2.0,
                    ),
                    Axis::PosX,
                    Axis::PosY,
                ),
                banded_edges: &[],
            });
            let xl = x0 + tb; // inside face of the left side
            let xr = x0 + box_outer_width - tb; // inside face of the right side
            let grooved: [(&str, Vec3, Vec3); 4] = [
                (
                    &side_left,
                    Vec3(xl, y_back, z_groove),
                    Vec3(xl, y_front, z_groove),
                ),
                (
                    &side_right,
                    Vec3(xr, y_back, z_groove),
                    Vec3(xr, y_front, z_groove),
                ),
                (
                    &box_front,
                    Vec3(xl, y_front - tb, z_groove),
                    Vec3(xr, y_front - tb, z_groove),
                ),
                (
                    &box_back,
                    Vec3(xl, y_back + tb, z_groove),
                    Vec3(xr, y_back + tb, z_groove),
                ),
            ];
            for (part_id, from, to) in grooved {
                // Box sides have Front outwards: the groove goes on their Back.
                groove_on_inside(ctx, part_id, from, to, groove_width, groove_depth);
                ctx.part_mut(part_id).overlap_exempt.push(bottom.clone());
            }
            ctx.part_mut(&bottom).overlap_exempt = vec![
                side_left.clone(),
                side_right.clone(),
                box_front.clone(),
                box_back.clone(),
            ];

            if let Some(fixing) = front_fixing {
                // Screwed from inside the box: the box front is drilled
                // through, the front panel takes the thread.
                ctx.request(JointKind::FaceToFace, id, &box_front, &front, fixing);
            }
            if let Some(handle) = handle {
                let z = match &handle.position {
                    Some(p) => z_front + ctx.eval(id, "handle.position", p)?,
                    None => z_front + front_height / 2.0,
                };
                let centre = Vec3(c0 + gap + front_width / 2.0, y_front, z);
                let spec = crate::spec::JointSpec {
                    hardware: handle.hardware.clone(),
                    placement: None,
                };
                ctx.request(
                    JointKind::Handle {
                        centre,
                        along: Axis::PosX,
                    },
                    id,
                    &front,
                    &front,
                    &spec,
                );
            }

            if let Some(f) = &files {
                // A rail along the top of each side, inside, screwed from
                // the front to the back; the pair goes in the BOM.
                let z = z_box + box_height - 12.0;
                for (side, x, looks) in
                    [(&side_left, xl, Axis::PosX), (&side_right, xr, Axis::NegX)]
                {
                    let face = ctx.part_mut(side).face_facing(looks);
                    ctx.request(
                        JointKind::Row {
                            from: Vec3(x, y_front - 20.0, z),
                            to: Vec3(x, y_back + 20.0, z),
                            face,
                        },
                        id,
                        side,
                        side,
                        &crate::spec::JointSpec {
                            hardware: f.screws.clone(),
                            placement: None,
                        },
                    );
                }
                if let Some(h) = f.hardware.first() {
                    ctx.extra_bom.push(super::ExtraBom {
                        component: id.to_string(),
                        hardware: h.clone(),
                        quantity: 1,
                        metres: 0.0,
                    });
                }
            }

            // Slides: box side ↔ the panel bounding the bay on that side.
            ctx.request(
                JointKind::Slide,
                id,
                &side_left,
                &bay.left_part,
                &slide_on[0],
            );
            ctx.request(
                JointKind::Slide,
                id,
                &side_right,
                &bay.right_part,
                &slide_on[1],
            );
        }
    }

    Ok(())
}

/// Groove on whichever large face of `part_id` looks at the segment's
/// inside: the box sides are oriented outwards, the others inwards.
fn groove_on_inside(
    ctx: &mut BuildCtx<'_>,
    part_id: &str,
    from: Vec3,
    to: Vec3,
    width: f64,
    depth: f64,
) {
    use crate::geometry::Face;
    use crate::model::{OpGeometry, Operation};
    let part = ctx.part_mut(part_id);
    // The segment lies on one of the two large faces' planes; pick the one
    // whose plane contains it.
    let local = part.placement.to_local(from);
    let face = if local.2.abs() < crate::units::EPS {
        Face::Back
    } else {
        Face::Front
    };
    let (u0, v0) = part.world_point_to_face_uv(face, from);
    let (u1, v1) = part.world_point_to_face_uv(face, to);
    let op = Operation {
        id: part.next_op_id(),
        face,
        geometry: OpGeometry::Groove {
            from: [u0, v0],
            to: [u1, v1],
            width,
            depth,
        },
        source: None,
    };
    part.operations.push(op);
}
