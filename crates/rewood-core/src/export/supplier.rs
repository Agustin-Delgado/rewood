//! The order for the panel supplier: what a shop that cuts, bands and
//! drills boards needs to make the parts, and nothing else. No hardware,
//! no prices, no nesting of ours (the supplier lays out its own sheets),
//! no NC in a dialect its machine may not speak.
//!
//! ```text
//! proveedor/despiece.csv   the cut list in the supplier's terms (Excel, es-AR)
//! proveedor/planos.html    conventions, cut list and one dimensioned drawing
//!                          per machined part; printed to PDF from a browser
//! proveedor/dxf/P001.dxf … one per machined part, for a CAM that imports DXF
//! ```
//!
//! The supplier's terms: sizes are finished (band included) in mm, the
//! two long edges are L1 and L2 and the short ones A1 and A2, "largo"
//! runs along the grain when the decor has one.

use std::fmt::Write;

use super::{drawing, dxf, html_esc, PackageFile};
use crate::geometry::Face;
use crate::library::material::GrainKind;
use crate::model::{Grain, OpGeometry, Part};
use crate::plan::{ManufacturingPlan, PartListRow};

/// A supplier's edge names, each bound to the face of the part it is.
struct Sides {
    /// L1, L2, A1, A2.
    faces: [(&'static str, Face); 4],
    /// Finished size along the supplier's "largo" and "ancho".
    largo: f64,
    ancho: f64,
    /// The decor has a direction: "largo" runs along it.
    grain: bool,
}

fn sides(part: &Part, plan: &ManufacturingPlan) -> Sides {
    let grain = match part.decor.as_deref() {
        Some(d) => plan.catalog.decors.get(d).is_some_and(|d| d.grain),
        None => plan
            .catalog
            .materials
            .get(&part.material)
            .is_some_and(|m| m.grain == GrainKind::Directional),
    };
    let (l, w) = (part.dims.length, part.dims.width);
    // Local X is "largo" unless the grain, or without one the longer
    // side, says local Y.
    let along_x = if grain {
        part.grain != Grain::Width
    } else {
        l >= w
    };
    if along_x {
        Sides {
            faces: [
                ("L1", Face::Bottom),
                ("L2", Face::Top),
                ("A1", Face::Left),
                ("A2", Face::Right),
            ],
            largo: l,
            ancho: w,
            grain,
        }
    } else {
        Sides {
            faces: [
                ("L1", Face::Left),
                ("L2", Face::Right),
                ("A1", Face::Bottom),
                ("A2", Face::Top),
            ],
            largo: w,
            ancho: l,
            grain,
        }
    }
}

/// Millimetres the way a supplier reads them: tenths at most, decimal
/// comma.
fn mm(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0 + 0.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r:.1}").replace('.', ",")
    }
}

/// "Melamina 18 mm (1,83 × 2,75)" → "Melamina 18 mm": the sheet size is
/// the supplier's business.
fn material_name(plan: &ManufacturingPlan, id: &str) -> String {
    let name = plan
        .catalog
        .materials
        .get(id)
        .map_or(id, |m| m.name.as_str());
    match name.find(" (") {
        Some(i) => name[..i].to_string(),
        None => name.to_string(),
    }
}

/// "Tapacanto PVC 22 × 0,45 mm" → "PVC 22 × 0,45 mm".
fn edge_name(plan: &ManufacturingPlan, id: &str) -> String {
    let name = plan
        .catalog
        .edge_materials
        .get(id)
        .map_or(id, |e| e.name.as_str());
    name.trim_start_matches("Tapacanto ")
        .trim_start_matches("Canto ")
        .to_string()
}

fn decor_name(plan: &ManufacturingPlan, part: &Part) -> (String, String) {
    match part
        .decor
        .as_deref()
        .and_then(|d| plan.catalog.decors.get(d))
    {
        Some(d) => (
            d.name.clone(),
            format!("{} {}", d.brand, d.code).trim().to_string(),
        ),
        None => (String::new(), String::new()),
    }
}

fn is_machining(g: &OpGeometry) -> bool {
    !matches!(g, OpGeometry::EdgeBand { .. })
}

/// "8 perf. cara A, 2 perf. cara B, 4 perf. de canto, ranura".
fn machining_summary(part: &Part) -> String {
    let (mut a, mut b, mut edge, mut grooves, mut cutouts) = (0, 0, 0, 0, 0);
    for op in &part.operations {
        match (&op.geometry, op.face) {
            (OpGeometry::Drill { .. }, Face::Front) => a += 1,
            (OpGeometry::Drill { .. }, Face::Back) => b += 1,
            (OpGeometry::Drill { .. }, _) => edge += 1,
            (OpGeometry::Groove { .. }, _) => grooves += 1,
            (OpGeometry::Cutout { .. }, _) => cutouts += 1,
            _ => {}
        }
    }
    let mut out = Vec::new();
    if a > 0 {
        out.push(format!("{a} perf. cara A"));
    }
    if b > 0 {
        out.push(format!("{b} perf. cara B"));
    }
    if edge > 0 {
        out.push(format!("{edge} perf. de canto"));
    }
    match grooves {
        0 => {}
        1 => out.push("ranura".into()),
        k => out.push(format!("{k} ranuras")),
    }
    match cutouts {
        0 => {}
        1 => out.push("calado".into()),
        k => out.push(format!("{k} calados")),
    }
    out.join(", ")
}

/// A row of the order: one group of equal parts from the cut list.
struct Row<'a> {
    list: &'a PartListRow,
    part: &'a Part,
    sides: Sides,
    /// 1-based drawing number, when the part is machined.
    drawing: Option<usize>,
}

fn rows(plan: &ManufacturingPlan) -> Vec<Row<'_>> {
    let mut out = Vec::new();
    let mut drawings = 0;
    for list in &plan.part_list {
        let Some(part) = plan.parts.iter().find(|p| p.id == list.part_ids[0]) else {
            continue;
        };
        // Glass and mirror go to the glazier, not to the board supplier.
        if plan
            .catalog
            .materials
            .get(&part.material)
            .is_some_and(|m| m.outsourced)
        {
            continue;
        }
        let drawing = part
            .operations
            .iter()
            .any(|op| is_machining(&op.geometry))
            .then(|| {
                drawings += 1;
                drawings
            });
        out.push(Row {
            list,
            part,
            sides: sides(part, plan),
            drawing,
        });
    }
    out
}

pub fn files(plan: &ManufacturingPlan) -> Vec<PackageFile> {
    let rows = rows(plan);
    let mut files = vec![
        PackageFile {
            path: "proveedor/despiece.csv".into(),
            contents: cut_list_csv(plan, &rows),
        },
        PackageFile {
            path: "proveedor/planos.html".into(),
            contents: order_html(plan, &rows),
        },
    ];
    for r in rows.iter().filter(|r| r.drawing.is_some()) {
        files.push(PackageFile {
            path: format!("proveedor/dxf/{}.dxf", r.part.id),
            contents: dxf::part_dxf(r.part),
        });
    }
    if rows.iter().any(|r| r.drawing.is_some()) {
        files.push(PackageFile {
            path: "proveedor/dxf/LEEME.txt".into(),
            contents: super::DXF_README.into(),
        });
    }
    files
}

fn edge_cells(plan: &ManufacturingPlan, r: &Row) -> Vec<String> {
    r.sides
        .faces
        .iter()
        .map(|(_, f)| {
            r.part
                .edges
                .get(f)
                .map(|e| edge_name(plan, e))
                .unwrap_or_default()
        })
        .collect()
}

/// Excel in Argentina reads `;` between cells and a decimal comma; the
/// BOM makes it take the file as UTF-8.
fn cut_list_csv(plan: &ManufacturingPlan, rows: &[Row]) -> String {
    let cell = |s: &str| {
        if s.contains([';', '"', '\n']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let mut out = String::from("\u{feff}");
    let header = [
        "Código",
        "Pieza",
        "Cantidad",
        "Material",
        "Color",
        "Código de color",
        "Espesor",
        "Largo",
        "Ancho",
        "Veta",
        "Canto L1",
        "Canto L2",
        "Canto A1",
        "Canto A2",
        "Mecanizado",
        "Plano",
    ];
    out += &header.join(";");
    out += "\n";
    for r in rows {
        let (color, code) = decor_name(plan, r.part);
        let mut cells = vec![
            r.list.part_ids.join(" "),
            r.list.name.clone(),
            r.list.quantity.to_string(),
            material_name(plan, &r.part.material),
            color,
            code,
            mm(r.part.dims.thickness),
            mm(r.sides.largo),
            mm(r.sides.ancho),
            if r.sides.grain { "Sí" } else { "No" }.into(),
        ];
        cells.extend(edge_cells(plan, r));
        cells.push(machining_summary(r.part));
        cells.push(r.drawing.map(|d| d.to_string()).unwrap_or_default());
        out += &cells.iter().map(|c| cell(c)).collect::<Vec<_>>().join(";");
        out += "\n";
    }
    out
}

const CSS: &str = "body{font-family:Helvetica,Arial,sans-serif;font-size:12px;color:#111;margin:0}\
.cover{padding:10mm 12mm}h1{font-size:20px;margin:0 0 2px}h2{font-size:15px;margin:18px 0 6px}\
.muted{color:#555}.box{border:1px solid #999;padding:8px 12px;margin:10px 0}.box li{margin:3px 0}\
table{border-collapse:collapse;margin:4px 0 8px}th,td{border:1px solid #bbb;padding:3px 6px;text-align:left;font-size:11px}\
th{background:#eee}td.num,th.num{text-align:right}.tables{display:flex;flex-wrap:wrap;gap:0 18px}\
.page{page-break-before:always}.page svg{display:block;width:100%;max-width:297mm;height:auto;margin:6mm auto;border:1px solid #ddd}\
@page{size:A4 landscape;margin:0}\
@media print{.page svg{width:297mm;height:210mm;margin:0;border:0}}";

fn order_html(plan: &ManufacturingPlan, rows: &[Row]) -> String {
    let f = &plan.furniture;
    let mut h = String::new();
    let _ = write!(
        h,
        "<!DOCTYPE html><html lang=\"es\"><head><meta charset=\"utf-8\"><title>Pedido de placas — {}</title><style>{CSS}{}</style></head><body><div class=\"cover\">",
        html_esc(&f.name),
        drawing::CSS
    );
    let _ = write!(
        h,
        "<h1>Pedido de corte, canteado y mecanizado</h1><div class=\"muted\">{} · referencia {}-v{}</div>",
        html_esc(&f.name),
        html_esc(&f.id),
        html_esc(&f.version)
    );
    h += "<div class=\"box\"><ul>\
<li>Cortar, cantear y mecanizar las piezas de la lista. <b>No incluye herrajes ni armado.</b></li>\
<li><b>Medidas finales en mm, con el canto incluido</b>: al cortar se descuenta el espesor del canto.</li>\
<li>Los cantos van en el mismo color que la placa. L1 y L2 son los lados largos, A1 y A2 los cortos.</li>\
<li>Veta «Sí»: el largo de la pieza va a lo largo de la veta de la placa. «No»: la pieza se puede girar.</li>\
<li>Planos según las normas de dibujo técnico (IRAM / ISO 128 y 129): rótulo, escala normalizada y cotas por coordenadas \
desde el origen 0 de cada vista. La <b>cara A</b> es la que queda hacia adentro del mueble (en puertas y frentes de cajón, la de atrás); \
la <b>cara B</b> se dibuja aparte, con la pieza dada vuelta. Las perforaciones de canto van centradas en el espesor y tienen su detalle en corte.</li>\
<li>Etiquetar cada pieza con su código.</li></ul></div>";

    // What to quote: board area and edge band, by colour.
    let mut areas: std::collections::BTreeMap<(String, String), f64> = Default::default();
    for r in rows {
        let (color, _) = decor_name(plan, r.part);
        *areas
            .entry((material_name(plan, &r.part.material), color))
            .or_default() += r.part.dims.length * r.part.dims.width / 1e6 * r.list.quantity as f64;
    }
    h += "<h2>Resumen</h2><div class=\"tables\"><table><tr><th>Placa</th><th>Color</th><th class=\"num\">Superficie de piezas</th></tr>";
    for ((m, c), a) in &areas {
        let _ = write!(
            h,
            "<tr><td>{}</td><td>{}</td><td class=\"num\">{} m²</td></tr>",
            html_esc(m),
            html_esc(c),
            format!("{a:.2}").replace('.', ",")
        );
    }
    h += "</table>";
    let bands: Vec<_> = plan
        .bom
        .consumables
        .iter()
        .filter(|c| c.length_m > 0.0)
        .collect();
    if !bands.is_empty() {
        h += "<table><tr><th>Canto</th><th>Color</th><th class=\"num\">Metros</th></tr>";
        for c in bands {
            let color = c
                .decor
                .as_deref()
                .and_then(|d| plan.catalog.decors.get(d))
                .map(|d| d.name.clone())
                .unwrap_or_default();
            let _ = write!(
                h,
                "<tr><td>{}</td><td>{}</td><td class=\"num\">{}</td></tr>",
                html_esc(&edge_name(plan, &c.material)),
                html_esc(&color),
                format!("{:.1}", c.length_m).replace('.', ",")
            );
        }
        h += "</table>";
    }
    h += "</div>";

    h += "<h2>Despiece</h2><table><tr><th>Código</th><th>Pieza</th><th class=\"num\">Cant.</th><th>Material</th><th>Color</th>\
<th class=\"num\">Esp.</th><th class=\"num\">Largo</th><th class=\"num\">Ancho</th><th>Veta</th>\
<th>L1</th><th>L2</th><th>A1</th><th>A2</th><th>Mecanizado</th><th>Plano</th></tr>";
    for r in rows {
        let (color, code) = decor_name(plan, r.part);
        let color = if code.is_empty() {
            color
        } else {
            format!("{color} ({code})")
        };
        let _ = write!(
            h,
            "<tr><td>{}</td><td>{}</td><td class=\"num\">{}</td><td>{}</td><td>{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td>",
            html_esc(&r.list.part_ids.join(", ")),
            html_esc(&r.list.name),
            r.list.quantity,
            html_esc(&material_name(plan, &r.part.material)),
            html_esc(&color),
            mm(r.part.dims.thickness),
            mm(r.sides.largo),
            mm(r.sides.ancho),
            if r.sides.grain { "Sí" } else { "No" }
        );
        for e in edge_cells(plan, r) {
            let _ = write!(
                h,
                "<td>{}</td>",
                if e.is_empty() {
                    "—".into()
                } else {
                    html_esc(&e)
                }
            );
        }
        let summary = machining_summary(r.part);
        let _ = write!(
            h,
            "<td>{}</td><td>{}</td></tr>",
            if summary.is_empty() {
                "—".into()
            } else {
                html_esc(&summary)
            },
            r.drawing
                .map(|d| d.to_string())
                .unwrap_or_else(|| "—".into())
        );
    }
    h += "</table>";

    h += "</div>";
    for r in rows {
        let Some(d) = r.drawing else {
            continue;
        };
        drawing_page(&mut h, plan, r, d);
    }
    h += "</body></html>\n";
    h
}

/// The sheets of one machined part: tables of every hole, groove and
/// cutout in the frame of the face they are on, drawn by `drawing`.
fn drawing_page(h: &mut String, plan: &ManufacturingPlan, r: &Row, number: usize) {
    let p = r.part;
    let w = p.dims.width;
    let side_name = |f: Face| {
        r.sides
            .faces
            .iter()
            .find(|(_, x)| *x == f)
            .map_or("?", |(n, _)| *n)
    };
    let mut face_a = Vec::new();
    let mut face_b = Vec::new();
    let mut edge = Vec::new();
    let mut grooves = Vec::new();
    let mut cutouts = Vec::new();
    for op in &p.operations {
        let face = match op.face {
            Face::Front => "A".to_string(),
            Face::Back => "B".to_string(),
            other => format!("canto {}", side_name(other)),
        };
        // Face B in its own frame: the part turned over, its bottom edge up.
        let y = |v: f64| if op.face == Face::Back { w - v } else { v };
        match &op.geometry {
            OpGeometry::Drill {
                u,
                v,
                diameter,
                depth,
                through,
                ..
            } => {
                let dp = if *through {
                    "pasante".to_string()
                } else {
                    mm(depth.unwrap_or(0.0))
                };
                match op.face {
                    Face::Front => face_a.push((*u, *v, *diameter, dp)),
                    Face::Back => face_b.push((*u, w - *v, *diameter, dp)),
                    f => edge.push((side_name(f), *u, *diameter, dp)),
                }
            }
            OpGeometry::Groove {
                from,
                to,
                width,
                depth,
            } => grooves.push(vec![
                face,
                format!("{}, {}", mm(from[0]), mm(y(from[1]))),
                format!("{}, {}", mm(to[0]), mm(y(to[1]))),
                mm(*width),
                mm(*depth),
            ]),
            OpGeometry::Cutout {
                u,
                v,
                width,
                height,
                radius,
            } => cutouts.push(vec![
                face,
                format!("{}, {}", mm(*u), mm(y(*v))),
                format!("{} × {}", mm(*width), mm(*height)),
                mm(*radius),
            ]),
            OpGeometry::EdgeBand { .. } => {}
        }
    }
    let by_xy = |a: &(f64, f64, f64, String), b: &(f64, f64, f64, String)| {
        a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1))
    };
    face_a.sort_by(by_xy);
    face_b.sort_by(by_xy);
    edge.sort_by(|a, b| a.0.cmp(b.0).then(a.1.total_cmp(&b.1)));

    let mut tables: Vec<drawing::Table> = Vec::new();
    let head = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    // A list column as wide as its longest cell.
    let list_width = |rows: &[Vec<String>], col: usize, min: f64| {
        (rows
            .iter()
            .map(|r| r[col].chars().count())
            .max()
            .unwrap_or(4) as f64
            * 1.35
            + 3.0)
            .max(min)
    };
    for (title, holes) in [
        ("Perforaciones cara A", &face_a),
        ("Perforaciones cara B (en la vista de la cara B)", &face_b),
    ] {
        if holes.is_empty() {
            continue;
        }
        // One row per diameter, depth and X with the Ys of that column,
        // or per Y with the Xs of that row: whichever is shorter.
        let group = |by_x: bool| {
            let mut groups: Vec<(f64, f64, &String, Vec<f64>)> = Vec::new();
            for (x, y, d, dp) in holes {
                let (key, other) = if by_x { (*x, *y) } else { (*y, *x) };
                match groups
                    .iter_mut()
                    .find(|g| (g.0 - key).abs() < 0.05 && g.1 == *d && g.2 == dp)
                {
                    Some(g) => g.3.push(other),
                    None => groups.push((key, *d, dp, vec![other])),
                }
            }
            groups
        };
        let (by_x, groups) = {
            let (gx, gy) = (group(true), group(false));
            if gy.len() < gx.len() {
                let mut gy = gy;
                gy.sort_by(|a, b| a.0.total_cmp(&b.0));
                (false, gy)
            } else {
                (true, gx)
            }
        };
        let rows: Vec<Vec<String>> = groups
            .iter()
            .map(|(key, d, dp, others)| {
                vec![
                    mm(*d),
                    dp.to_string(),
                    mm(*key),
                    others
                        .iter()
                        .map(|o| mm(*o))
                        .collect::<Vec<_>>()
                        .join(" · "),
                ]
            })
            .collect();
        let lw = list_width(&rows, 3, 12.0);
        let (k, o) = if by_x { ("X", "Y") } else { ("Y", "X") };
        tables.push((
            title.to_string(),
            vec![9.0, 13.0, 13.0, lw],
            head(&["Ø", "Prof.", k, o]),
            rows,
        ));
    }
    if !edge.is_empty() {
        let mut groups: Vec<(&str, f64, &String, Vec<f64>)> = Vec::new();
        for (side, at, d, dp) in &edge {
            match groups
                .iter_mut()
                .find(|g| g.0 == *side && g.1 == *d && g.2 == dp)
            {
                Some(g) => g.3.push(*at),
                None => groups.push((side, *d, dp, vec![*at])),
            }
        }
        let rows: Vec<Vec<String>> = groups
            .iter()
            .map(|(side, d, dp, ats)| {
                vec![
                    side.to_string(),
                    mm(*d),
                    dp.to_string(),
                    ats.iter().map(|a| mm(*a)).collect::<Vec<_>>().join(" · "),
                ]
            })
            .collect();
        let aw = list_width(&rows, 3, 16.0);
        tables.push((
            "Perforaciones de canto (posición desde 0)".into(),
            vec![11.0, 9.0, 11.0, aw],
            head(&["Canto", "Ø", "Prof.", "Posición"]),
            rows,
        ));
    }
    if !grooves.is_empty() {
        tables.push((
            "Ranuras".into(),
            vec![16.0, 22.0, 22.0, 12.0, 11.0],
            head(&["Cara", "Desde X, Y", "Hasta X, Y", "Ancho", "Prof."]),
            grooves,
        ));
    }
    if !cutouts.is_empty() {
        tables.push((
            "Calados (pasantes)".into(),
            vec![16.0, 24.0, 24.0, 12.0],
            head(&["Cara", "Centro X, Y", "Ancho × alto", "Radio"]),
            cutouts,
        ));
    }

    let (color, code) = decor_name(plan, p);
    let mut material = material_name(plan, &p.material);
    if !color.is_empty() {
        material = format!("{material} · {color}");
    }
    if !code.is_empty() {
        material = format!("{material} ({code})");
    }
    if r.sides.grain {
        material += " · con veta";
    }
    let t = &plan.profile.tolerances;
    let info = drawing::SheetInfo {
        number,
        ids: r.list.part_ids.join(", "),
        name: r.list.name.clone(),
        quantity: r.list.quantity,
        material,
        furniture: format!(
            "{} · {}-v{}",
            plan.furniture.name, plan.furniture.id, plan.furniture.version
        ),
        tolerances: format!(
            "medidas ±{} · posición de agujeros ±{} · Ø ±{}",
            mm(t.length),
            mm(t.hole_position),
            mm(t.hole_diameter)
        ),
        sides: r.sides.faces,
        bands: p
            .edges
            .iter()
            .map(|(f, e)| (*f, edge_name(plan, e)))
            .collect(),
        grain_along_x: r.sides.grain.then_some(r.sides.faces[0].1 == Face::Bottom),
    };
    let _ = write!(h, "<section class=\"plano\" data-plano=\"{number}\">");
    for sheet in drawing::part_sheets(p, &info, tables) {
        let _ = write!(h, "<div class=\"page\">{sheet}</div>");
    }
    *h += "</section>";
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(fixture: &str) -> ManufacturingPlan {
        let path = format!(
            "{}/../../fixtures/{fixture}/input.json",
            env!("CARGO_MANIFEST_DIR")
        );
        crate::compile_json(&std::fs::read_to_string(path).unwrap())
    }

    #[test]
    fn the_order_carries_boards_only_and_nothing_extra() {
        let plan = plan("wardrobe_1800");
        let files = files(&plan);
        let csv = &files
            .iter()
            .find(|f| f.path == "proveedor/despiece.csv")
            .unwrap()
            .contents;
        let html = &files
            .iter()
            .find(|f| f.path == "proveedor/planos.html")
            .unwrap()
            .contents;
        // One CSV row per cut-list row, all of them boards here.
        assert_eq!(csv.lines().count(), plan.part_list.len() + 1);
        assert!(csv.starts_with("\u{feff}Código;Pieza;Cantidad;"));
        // Finished sizes, decimal comma, colour with its code.
        assert!(
            csv.contains(";Melamina 18 mm;Blanco Nature;Faplac 135NAT;18;2100;500;"),
            "{csv}"
        );
        // Nothing the supplier does not need.
        for word in [
            "Tarugo",
            "Minifix",
            "Bisagra",
            "Corredera",
            "$",
            "precio",
            "Precio",
        ] {
            assert!(!html.contains(word), "{word}");
            assert!(!csv.contains(word), "{word}");
        }
        assert!(files.iter().all(|f| !f.path.ends_with(".nc")));
        // One drawing per machined row.
        let drawings = files.iter().filter(|f| f.path.ends_with(".dxf")).count();
        assert_eq!(html.matches("<section class=\"plano\"").count(), drawings);
        assert_eq!(files, super::files(&plan));
    }

    #[test]
    fn glass_goes_to_the_glazier_not_to_the_board_order() {
        let plan = plan("display_cabinet");
        let csv = files(&plan)
            .into_iter()
            .find(|f| f.path == "proveedor/despiece.csv")
            .unwrap()
            .contents;
        assert!(plan.parts.iter().any(|p| p.material.starts_with("glass")));
        assert!(!csv.contains("Vidrio"), "{csv}");
    }

    #[test]
    fn long_edges_are_l_and_a_plain_colour_may_turn() {
        assert_eq!(mm(263.433), "263,4");
        assert_eq!(mm(500.0), "500");
        assert_eq!(mm(0.04), "0");
        let plan = plan("wardrobe_1800");
        let side = plan.parts.iter().find(|p| p.role == "side_left").unwrap();
        let s = sides(side, &plan);
        assert_eq!(s.faces[0], ("L1", Face::Bottom));
        assert!(!s.grain, "Blanco Nature has no direction");
    }
}
