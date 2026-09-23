//! Assembly instructions derived from the joint graph: which parts meet,
//! with what hardware, in an order a person can follow. The order is a
//! rule of thumb over component kinds (carcass box first, then what hangs
//! inside it, drawers, doors) — not a physical simulation.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::plan::ManufacturingPlan;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub number: usize,
    pub title: String,
    pub parts: Vec<String>,
    /// Hardware name → quantity used in this step.
    pub hardware: BTreeMap<String, usize>,
    pub notes: Vec<String>,
}

fn part_label(plan: &ManufacturingPlan, id: &str) -> String {
    plan.parts
        .iter()
        .find(|p| p.id == id)
        .map(|p| format!("{} ({})", p.name, p.id))
        .unwrap_or_else(|| id.to_string())
}

fn hardware_name(plan: &ManufacturingPlan, id: &str) -> String {
    plan.bom
        .hardware
        .iter()
        .find(|h| h.hardware == id)
        .map(|h| h.name.clone())
        .unwrap_or_else(|| id.to_string())
}

/// Component kind from its parts' roles.
fn kind_of(plan: &ManufacturingPlan, component: &str) -> &'static str {
    let roles: Vec<&str> = plan
        .parts
        .iter()
        .filter(|p| p.component == component)
        .map(|p| p.role.as_str())
        .collect();
    if roles
        .iter()
        .any(|r| r.contains("side_left") && !r.contains("drawer"))
    {
        "carcass"
    } else if roles.iter().any(|r| r.contains("drawer")) {
        "drawers"
    } else if roles.iter().any(|r| r.contains("door")) {
        "doors"
    } else if roles.iter().any(|r| r.contains("shelf")) {
        "shelves"
    } else {
        "other"
    }
}

pub fn steps(plan: &ManufacturingPlan) -> Vec<Step> {
    let mut components: Vec<String> = Vec::new();
    for p in &plan.parts {
        if !components.contains(&p.component) {
            components.push(p.component.clone());
        }
    }
    // Carcasses first, then shelves, drawers, doors, keeping spec order
    // inside each group.
    let rank = |c: &String| match kind_of(plan, c) {
        "carcass" => 0,
        "shelves" => 1,
        "drawers" => 2,
        "doors" => 3,
        _ => 4,
    };
    components.sort_by_key(|c| rank(c));

    let mut steps = Vec::new();
    let mut push = |title: String,
                    parts: Vec<String>,
                    hardware: BTreeMap<String, usize>,
                    notes: Vec<String>| {
        steps.push(Step {
            number: steps.len() + 1,
            title,
            parts,
            hardware,
            notes,
        });
    };
    let count_hw = |ids: &[&str]| -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        for j in plan.joints.iter().filter(|j| ids.contains(&j.id.as_str())) {
            for f in &j.fasteners {
                *m.entry(hardware_name(plan, &f.hardware)).or_default() += 1;
            }
        }
        m
    };

    for component in &components {
        let kind = kind_of(plan, component);
        let parts: Vec<&crate::model::Part> = plan
            .parts
            .iter()
            .filter(|p| &p.component == component)
            .collect();
        let joints: Vec<&crate::model::Joint> = plan
            .joints
            .iter()
            .filter(|j| &j.component == component)
            .collect();
        let joint_ids: Vec<&str> = joints.iter().map(|j| j.id.as_str()).collect();
        match kind {
            "carcass" => {
                let named =
                    |role: &str| parts.iter().find(|p| p.role == role).map(|p| p.id.clone());
                let dividers: Vec<String> = parts
                    .iter()
                    .filter(|p| p.role.starts_with("divider"))
                    .map(|p| p.id.clone())
                    .collect();
                let back = named("back");
                // 1. Bottom to the sides and dividers.
                let bottom = named("bottom");
                let top = named("top");
                let mut box_parts: Vec<String> =
                    [named("side_left"), bottom.clone(), named("side_right")]
                        .into_iter()
                        .flatten()
                        .collect();
                box_parts.extend(dividers.iter().cloned());
                // Bottom to the sides, and dividers to the bottom.
                let bottom_joints: Vec<&str> = joints
                    .iter()
                    .filter(|j| {
                        j.kind == "butt"
                            && (Some(&j.edge_part) == bottom.as_ref()
                                || (dividers.contains(&j.edge_part)
                                    && Some(&j.face_part) == bottom.as_ref()))
                    })
                    .map(|j| j.id.as_str())
                    .collect();
                let mut notes = vec![
                    "Trabajá sobre una superficie plana y limpia, con los frentes (canto con tapacanto) hacia vos.".into(),
                    "Colocá primero los herrajes en los cantos (pernos Minifix, tarugos) de la base y de los divisores.".into(),
                ];
                if !dividers.is_empty() {
                    notes.push(format!("Los divisores ({}) van entre base y tapa; apoyalos sobre la base antes de cerrar.", dividers.join(", ")));
                }
                push(
                    format!("Armar la carcasa '{component}': base, laterales y divisores"),
                    box_parts.iter().map(|id| part_label(plan, id)).collect(),
                    count_hw(&bottom_joints),
                    notes,
                );
                if let Some(back_id) = &back {
                    push(
                        format!("Deslizar el fondo en las ranuras de '{component}'"),
                        vec![part_label(plan, back_id)],
                        BTreeMap::new(),
                        vec![
                            "Entra por arriba, antes de colocar la tapa, en la ranura de laterales y base.".into(),
                            "No lleva herrajes: queda encerrado por la tapa.".into(),
                        ],
                    );
                }
                if let Some(top_id) = &top {
                    let top_joints: Vec<&str> = joints
                        .iter()
                        .filter(|j| {
                            j.kind == "butt" && (&j.edge_part == top_id || &j.face_part == top_id)
                        })
                        .map(|j| j.id.as_str())
                        .collect();
                    push(
                        format!("Cerrar '{component}' con la tapa"),
                        vec![part_label(plan, top_id)],
                        count_hw(&top_joints),
                        vec!["Alineá los tarugos, bajá la tapa y ajustá las excéntricas hasta el tope.".into(), "Verificá la escuadra midiendo las dos diagonales del frente.".into()],
                    );
                }
                // Legs and plinth: fixtures on the bottom's underside and on
                // the plinth, once the box is closed and can be turned over.
                let bottom = named("bottom");
                let leg_joints: Vec<&str> = joints
                    .iter()
                    .filter(|j| j.kind == "fixture" && bottom.as_ref() == Some(&j.face_part))
                    .map(|j| j.id.as_str())
                    .collect();
                if !leg_joints.is_empty() {
                    push(
                        format!("Atornillar las patas de '{component}'"),
                        bottom.iter().map(|b| part_label(plan, b)).collect(),
                        count_hw(&leg_joints),
                        vec![
                            "Dá vuelta la carcasa y presentá cada pata sobre sus perforaciones piloto de la cara exterior de la base.".into(),
                            "Regulá la altura recién con el mueble parado y nivelado.".into(),
                        ],
                    );
                }
                // Hangers: the carcass's other fixtures (on its sides), for a
                // wall-hung cabinet, once it stands closed.
                let plinth = named("plinth");
                let hanger_joints: Vec<&str> = joints
                    .iter()
                    .filter(|j| {
                        j.kind == "fixture"
                            && bottom.as_ref() != Some(&j.face_part)
                            && plinth.as_ref() != Some(&j.face_part)
                    })
                    .map(|j| j.id.as_str())
                    .collect();
                if !hanger_joints.is_empty() {
                    let mut sides: Vec<String> = hanger_joints
                        .iter()
                        .filter_map(|id| joints.iter().find(|j| j.id == *id))
                        .map(|j| part_label(plan, &j.face_part))
                        .collect();
                    sides.dedup();
                    push(
                        format!("Atornillar los colgadores de '{component}'"),
                        sides,
                        count_hw(&hanger_joints),
                        vec![
                            "Cada colgador va en la esquina superior trasera de su lateral, sobre sus perforaciones piloto, con el gancho hacia atrás.".into(),
                            "En la pared, el riel a la altura que dé el mueble; colgalo y regulá altura y profundidad con los tornillos del colgador.".into(),
                        ],
                    );
                }
                if let Some(plinth_id) = plinth {
                    let clip_joints: Vec<&str> = joints
                        .iter()
                        .filter(|j| j.kind == "fixture" && j.face_part == plinth_id)
                        .map(|j| j.id.as_str())
                        .collect();
                    push(
                        format!("Presentar el zócalo de '{component}'"),
                        vec![part_label(plan, &plinth_id)],
                        count_hw(&clip_joints),
                        vec![
                            "Atornillá los clips en la cara interior del zócalo, sobre las perforaciones piloto; enganchalos en las patas delanteras.".into(),
                            "El zócalo se saca sin herramientas para limpiar debajo.".into(),
                        ],
                    );
                }
            }
            "shelves" => {
                // Adjustable shelves: the panels carry the rows, the
                // shelves only rest on pins, after everything is closed.
                if joints.iter().all(|j| j.kind == "row") && !joints.is_empty() {
                    let pins: usize = 4 * parts.len();
                    let mut hardware = BTreeMap::new();
                    hardware.insert(hardware_name(plan, "shelf_pin_5"), pins);
                    let per_row = plan
                        .derived
                        .get(&format!("{component}.pin_holes_per_row"))
                        .copied()
                        .unwrap_or(0.0);
                    // One hole per shelf and row: the shelf only goes there.
                    let movable = per_row > parts.len() as f64;
                    let (title, where_) = if movable {
                        (
                            "Colocar los estantes regulables",
                            format!(
                                "Con el mueble cerrado: cuatro soportes por estante en las hileras de Ø5 ({} agujeros por hilera, cada 32 mm), a la misma altura los cuatro.",
                                per_row
                            ),
                        )
                    } else {
                        (
                            "Colocar los estantes sobre soportes",
                            "Con el mueble cerrado: un soporte en cada uno de los cuatro agujeros de Ø5 que lleva cada estante.".to_string(),
                        )
                    };
                    push(
                        format!("{title} de '{component}'"),
                        parts.iter().map(|p| part_label(plan, &p.id)).collect(),
                        hardware,
                        vec![
                            where_,
                            "El estante entra con 1 mm de juego por lado; el frente con tapacanto mira adelante.".into(),
                        ],
                    );
                    continue;
                }
                let fixed = parts.iter().all(|p| p.role.contains("fixed_shelf"));
                let mut notes = vec![
                    "Van ahora, con la carcasa todavía abierta: los tarugos entran en el canto del estante y en la cara interior de los paneles que lo limitan.".into(),
                    "El frente con tapacanto mira adelante.".into(),
                ];
                if fixed {
                    notes.insert(
                        1,
                        "Es un estante fijo a altura de plano: ajustá las excéntricas hasta el tope antes de seguir, porque sostiene lo que va arriba y abajo.".into(),
                    );
                }
                push(
                    format!(
                        "Fijar {} de '{component}'",
                        if fixed {
                            "el estante fijo"
                        } else {
                            "los estantes"
                        }
                    ),
                    parts.iter().map(|p| part_label(plan, &p.id)).collect(),
                    count_hw(&joint_ids),
                    notes,
                );
            }
            "drawers" => {
                let mut boxes: BTreeMap<String, Vec<String>> = BTreeMap::new();
                for p in &parts {
                    // role: [bayN_]drawer_K_...
                    let key = p.role.split("_side").next().unwrap_or(&p.role);
                    let key = key.split("_box").next().unwrap_or(key);
                    let key = key.split("_front").next().unwrap_or(key);
                    let key = key.split("_bottom").next().unwrap_or(key);
                    boxes.entry(key.to_string()).or_default().push(p.id.clone());
                }
                for (k, ids) in boxes {
                    let own_joints: Vec<&str> = joints
                        .iter()
                        .filter(|j| ids.contains(&j.edge_part) || ids.contains(&j.face_part))
                        .map(|j| j.id.as_str())
                        .collect();
                    let label = k
                        .replace("bay", "bahía ")
                        .replace("_drawer_", " cajón ")
                        .replace("drawer_", "cajón ");
                    // A slide joint names a second hardware only for the
                    // spacer under it.
                    let spaced = joints.iter().any(|j| {
                        own_joints.contains(&j.id.as_str())
                            && j.kind == "slide"
                            && j.hardware.len() > 1
                    });
                    let mut notes: Vec<String> = vec![
                        "Armá la caja: frente interior y trasera entre los laterales, con el fondo deslizado en la ranura antes de cerrar.".into(),
                    ];
                    if spaced {
                        notes.push("Del lado de la bisagra de la puerta, atornillá primero el distanciador en el panel (sobre los pilotos de la corredera) y la corredera sobre él: así el cajón sale sin tocar la puerta abierta.".into());
                    }
                    notes.push("Atornillá la mitad fija de la corredera en el panel de la carcasa (sobre sus pilotos, desde el frente) y la móvil en el lateral de la caja; encastrá.".into());
                    notes.push("Con la caja colocada, atornillá el frente desde adentro y montá el tirador.".into());
                    push(
                        format!("Armar y montar {label} ('{component}')"),
                        ids.iter().map(|id| part_label(plan, id)).collect(),
                        count_hw(&own_joints),
                        notes,
                    );
                }
            }
            "doors" => {
                let mut notes = vec![
                    "Cazoletas en el dorso de la puerta (Ø35), bases en la cara interior del panel a 37 mm del frente; enganchá y regulá en las tres direcciones.".into(),
                ];
                if joints.iter().any(|j| j.kind == "handle") {
                    notes.push("Montá el tirador por los pasantes del frente.".into());
                }
                if joints.iter().any(|j| j.kind == "fixture") {
                    notes.push("Cierre: el cuerpo va en la cara interior del panel opuesto a la bisagra, a ras del frente y a media altura de la puerta; la placa en el dorso de la puerta, enfrentada. Regulá hasta que la puerta cierre sin golpear.".into());
                }
                push(
                    format!("Colgar las puertas de '{component}'"),
                    parts.iter().map(|p| part_label(plan, &p.id)).collect(),
                    count_hw(&joint_ids),
                    notes,
                );
            }
            _ => {
                push(
                    format!("Montar '{component}'"),
                    parts.iter().map(|p| part_label(plan, &p.id)).collect(),
                    count_hw(&joint_ids),
                    Vec::new(),
                );
            }
        }
    }
    // Components with joints but no panels: a rail is supports screwed to
    // panels that are already standing, so it goes last, with the doors.
    let mut joint_only: Vec<String> = Vec::new();
    for j in &plan.joints {
        if !components.contains(&j.component) && !joint_only.contains(&j.component) {
            joint_only.push(j.component.clone());
        }
    }
    for component in &joint_only {
        let ids: Vec<&str> = plan
            .joints
            .iter()
            .filter(|j| &j.component == component)
            .map(|j| j.id.as_str())
            .collect();
        let panels: Vec<String> = plan
            .joints
            .iter()
            .filter(|j| &j.component == component)
            .map(|j| part_label(plan, &j.face_part))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let rail = plan.bom.hardware.iter().find(|h| {
            plan.joints.iter().any(|j| {
                &j.component == component && j.hardware.iter().any(|x| x.starts_with("rail"))
            }) && h.hardware.starts_with("rail_")
                && !h.hardware.contains("support")
        });
        let bar = rail
            .and_then(|h| h.items.first())
            .map(|i| format!("{} ({} m)", i.name.replace(" (por metro)", ""), i.quantity))
            .unwrap_or_else(|| "el barral".into());
        push(
            format!("Colocar el barral de '{component}'"),
            panels,
            count_hw(&ids),
            vec![
                format!("Atornillar un soporte en cada panel, a la altura marcada por las perforaciones, y calzar {bar} cortado al ancho de la bahía."),
                "Con la carcasa parada: el barral se mide entre caras interiores, no entre ejes.".into(),
            ],
        );
    }

    // Dowelled shelves sit between panels that are still loose: they go in
    // before the back and the top close the box.
    let shelf_steps: Vec<Step> = steps
        .iter()
        .filter(|s| {
            s.title.starts_with("Fijar los estantes")
                || s.title.starts_with("Fijar el estante fijo")
        })
        .cloned()
        .collect();
    if !shelf_steps.is_empty() {
        steps.retain(|s| {
            !(s.title.starts_with("Fijar los estantes")
                || s.title.starts_with("Fijar el estante fijo"))
        });
        let at = steps
            .iter()
            .position(|s| s.title.starts_with("Deslizar el fondo") || s.title.starts_with("Cerrar"))
            .unwrap_or(steps.len());
        for (k, st) in shelf_steps.into_iter().enumerate() {
            steps.insert(at + k, st);
        }
        for (k, st) in steps.iter_mut().enumerate() {
            st.number = k + 1;
        }
    }
    // Adjustable shelves go in last, once the doors hang: nothing else
    // needs the bays clear after them.
    let loose: Vec<Step> = steps
        .iter()
        .filter(|s| s.title.starts_with("Colocar los estantes regulables"))
        .cloned()
        .collect();
    if !loose.is_empty() {
        steps.retain(|s| !s.title.starts_with("Colocar los estantes regulables"));
        steps.extend(loose);
        for (k, st) in steps.iter_mut().enumerate() {
            st.number = k + 1;
        }
    }
    steps
}

pub fn manual_txt(plan: &ManufacturingPlan) -> String {
    let mut s = format!(
        "Manual de armado — {} ({})\n\n",
        plan.furniture.name, plan.furniture.id
    );
    for st in steps(plan) {
        s.push_str(&format!("Paso {} — {}\n", st.number, st.title));
        for p in &st.parts {
            s.push_str(&format!("  · {p}\n"));
        }
        if !st.hardware.is_empty() {
            s.push_str("  Herrajes: ");
            s.push_str(
                &st.hardware
                    .iter()
                    .map(|(k, v)| format!("{v} × {k}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            s.push('\n');
        }
        for n in &st.notes {
            s.push_str(&format!("  – {n}\n"));
        }
        s.push('\n');
    }
    s
}

pub fn manual_html(plan: &ManufacturingPlan) -> String {
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let mut h = String::from("<ol class=steps>");
    for st in steps(plan) {
        h.push_str(&format!("<li><b>{}</b><ul>", esc(&st.title)));
        for p in &st.parts {
            h.push_str(&format!("<li>{}</li>", esc(p)));
        }
        h.push_str("</ul>");
        if !st.hardware.is_empty() {
            h.push_str("<div class=muted>Herrajes: ");
            h.push_str(&esc(&st
                .hardware
                .iter()
                .map(|(k, v)| format!("{v} × {k}"))
                .collect::<Vec<_>>()
                .join(", ")));
            h.push_str("</div>");
        }
        for n in &st.notes {
            h.push_str(&format!("<div>– {}</div>", esc(n)));
        }
        h.push_str("</li>");
    }
    h.push_str("</ol>");
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wardrobe_manual_orders_carcass_back_top_shelves_drawers_doors() {
        let spec = include_str!("../../../../fixtures/wardrobe_1800/input.json");
        let plan = crate::compile_json(spec);
        let steps = steps(&plan);
        let titles: Vec<&str> = steps.iter().map(|s| s.title.as_str()).collect();
        assert!(titles[0].starts_with("Armar la carcasa"));
        assert!(titles[1].starts_with("Fijar los estantes"));
        assert!(titles[2].starts_with("Fijar los estantes"));
        assert!(titles[3].starts_with("Deslizar el fondo"));
        assert!(titles[4].starts_with("Cerrar"));
        let first_drawer = titles
            .iter()
            .position(|t| t.starts_with("Armar y montar"))
            .unwrap();
        let first_door = titles.iter().position(|t| t.starts_with("Colgar")).unwrap();
        assert!(first_drawer < first_door);
        // Three drawer steps, one per drawer.
        assert_eq!(
            titles
                .iter()
                .filter(|t| t.starts_with("Armar y montar"))
                .count(),
            3
        );
        // Closing the carcass uses the top's hardware: 2 joints × (2 minifix + 2 dowels) + 2 dividers × (2 + 2).
        let close = &steps[4];
        assert_eq!(close.hardware.values().sum::<usize>(), 16);
        let txt = manual_txt(&plan);
        assert!(txt.contains("Paso 1 —"));
        assert_eq!(txt, manual_txt(&plan));
    }
}
