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

use super::{dxf, html_esc, PackageFile};
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
        Some(d) => (d.name.clone(), format!("{} {}", d.brand, d.code)),
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

const CSS: &str = "body{font-family:Helvetica,Arial,sans-serif;font-size:12px;color:#111;margin:24px}\
h1{font-size:20px;margin:0 0 2px}h2{font-size:15px;margin:18px 0 6px}h3{font-size:12px;margin:10px 0 4px}\
.muted{color:#555}.box{border:1px solid #999;padding:8px 12px;margin:10px 0}.box li{margin:3px 0}\
table{border-collapse:collapse;margin:4px 0 8px}th,td{border:1px solid #bbb;padding:3px 6px;text-align:left;font-size:11px}\
th{background:#eee}td.num,th.num{text-align:right}.tables{display:flex;flex-wrap:wrap;gap:0 18px}\
.sheet{page-break-before:always}svg{width:100%;height:auto;max-height:120mm}\
@page{size:A4 landscape;margin:10mm}@media print{body{margin:0}}";

fn order_html(plan: &ManufacturingPlan, rows: &[Row]) -> String {
    let f = &plan.furniture;
    let mut h = String::new();
    let _ = write!(
        h,
        "<!DOCTYPE html><html lang=\"es\"><head><meta charset=\"utf-8\"><title>Pedido de placas — {}</title><style>{CSS}</style></head><body>",
        html_esc(&f.name)
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
<li>Las perforaciones se miden desde la esquina 0,0 de cada plano, mirando la <b>cara A</b>: la de los mecanizados principales, \
la que queda hacia adentro del mueble (en puertas y frentes de cajón, la de atrás). La cara B es la opuesta. \
Las perforaciones de canto van centradas en el espesor.</li>\
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

    for r in rows {
        let Some(d) = r.drawing else {
            continue;
        };
        drawing_page(&mut h, plan, r, d);
    }
    h += "</body></html>\n";
    h
}

fn drawing_page(h: &mut String, plan: &ManufacturingPlan, r: &Row, number: usize) {
    let p = r.part;
    let (color, code) = decor_name(plan, p);
    let qty = if r.list.quantity > 1 {
        format!(" (×{})", r.list.quantity)
    } else {
        String::new()
    };
    let _ = write!(
        h,
        "<section class=\"sheet\"><h2>Plano {number} · {}{qty} — {}</h2><div class=\"muted\">{} {} {}· {} × {} × {} mm · veta {}</div>",
        html_esc(&r.list.part_ids.join(", ")),
        html_esc(&r.list.name),
        html_esc(&material_name(plan, &p.material)),
        html_esc(&color),
        if code.is_empty() { String::new() } else { format!("({}) ", html_esc(&code)) },
        mm(r.sides.largo),
        mm(r.sides.ancho),
        mm(p.dims.thickness),
        if r.sides.grain { "sí" } else { "no" },
    );
    *h += &drawing_svg(r);

    // X, Y from the 0,0 corner seen from face A; face B holes as seen from
    // face B after turning the part over its X axis (L1 goes up).
    let w = p.dims.width;
    let mut face_a = Vec::new();
    let mut face_b = Vec::new();
    let mut edge = Vec::new();
    let mut grooves = Vec::new();
    let mut cutouts = Vec::new();
    let side_name = |f: Face| {
        r.sides
            .faces
            .iter()
            .find(|(_, x)| *x == f)
            .map_or("?", |(n, _)| *n)
    };
    let face_name = |f: Face| match f {
        Face::Front => "A".to_string(),
        Face::Back => "B".to_string(),
        other => format!("canto {}", side_name(other)),
    };
    for op in &p.operations {
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
            } => grooves.push((face_name(op.face), *from, *to, *width, *depth)),
            OpGeometry::Cutout {
                u,
                v,
                width,
                height,
                radius,
            } => cutouts.push((face_name(op.face), *u, *v, *width, *height, *radius)),
            OpGeometry::EdgeBand { .. } => {}
        }
    }
    let by_xy = |a: &(f64, f64, f64, String), b: &(f64, f64, f64, String)| {
        a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1))
    };
    face_a.sort_by(by_xy);
    face_b.sort_by(by_xy);
    edge.sort_by(|a, b| a.0.cmp(b.0).then(a.1.total_cmp(&b.1)));

    *h += "<div class=\"tables\">";
    for (title, holes) in [
        ("Perforaciones cara A", &face_a),
        (
            "Perforaciones cara B (vistas desde la cara B, con la pieza girada sobre su largo: L1 arriba)",
            &face_b,
        ),
    ] {
        if holes.is_empty() {
            continue;
        }
        // One row per diameter, depth and X; the Ys of that column together.
        let mut groups: Vec<(f64, f64, &String, Vec<f64>)> = Vec::new();
        for (x, y, d, dp) in holes {
            match groups
                .iter_mut()
                .find(|g| (g.0 - x).abs() < 0.05 && g.1 == *d && g.2 == dp)
            {
                Some(g) => g.3.push(*y),
                None => groups.push((*x, *d, dp, vec![*y])),
            }
        }
        let _ = write!(
            h,
            "<div><h3>{title}</h3><table><tr><th class=\"num\">Ø</th><th class=\"num\">Prof.</th><th class=\"num\">X</th><th>Y</th></tr>"
        );
        for (x, d, dp, ys) in &groups {
            let ys: Vec<String> = ys.iter().map(|y| mm(*y)).collect();
            let _ = write!(
                h,
                "<tr><td class=\"num\">{}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td></tr>",
                mm(*d),
                dp,
                mm(*x),
                ys.join(" · ")
            );
        }
        *h += "</table></div>";
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
        *h += "<div><h3>Perforaciones de canto (centradas en el espesor)</h3><table><tr><th>Canto</th><th class=\"num\">Ø</th><th class=\"num\">Prof.</th><th>A (desde la esquina 0,0)</th></tr>";
        for (side, d, dp, ats) in &groups {
            let ats: Vec<String> = ats.iter().map(|a| mm(*a)).collect();
            let _ = write!(
                h,
                "<tr><td>{side}</td><td class=\"num\">{}</td><td class=\"num\">{}</td><td>{}</td></tr>",
                mm(*d),
                dp,
                ats.join(" · ")
            );
        }
        *h += "</table></div>";
    }
    if !grooves.is_empty() {
        *h += "<div><h3>Ranuras</h3><table><tr><th>Cara</th><th>Desde (X, Y)</th><th>Hasta (X, Y)</th><th class=\"num\">Ancho</th><th class=\"num\">Prof.</th></tr>";
        for (face, from, to, width, depth) in &grooves {
            let _ = write!(
                h,
                "<tr><td>{face}</td><td>{}, {}</td><td>{}, {}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
                mm(from[0]),
                mm(from[1]),
                mm(to[0]),
                mm(to[1]),
                mm(*width),
                mm(*depth)
            );
        }
        *h += "</table></div>";
    }
    if !cutouts.is_empty() {
        *h += "<div><h3>Calados (pasantes)</h3><table><tr><th>Cara</th><th>Centro (X, Y)</th><th>Ancho × alto</th><th class=\"num\">Radio</th></tr>";
        for (face, u, v, width, height, radius) in &cutouts {
            let _ = write!(
                h,
                "<tr><td>{face}</td><td>{}, {}</td><td>{} × {}</td><td class=\"num\">{}</td></tr>",
                mm(*u),
                mm(*v),
                mm(*width),
                mm(*height),
                mm(*radius)
            );
        }
        *h += "</table></div>";
    }
    *h += "</div></section>";
}

/// The part seen from face A: outline, banded edges and their names,
/// every hole (face B dashed, edge holes as a line as deep as the hole),
/// ordinate dimensions from the 0,0 corner and the overall size.
fn drawing_svg(r: &Row) -> String {
    let p = r.part;
    let (len, wid) = (p.dims.length, p.dims.width);
    let scale = (780.0 / len).min(300.0 / wid);
    let (pw, ph) = (len * scale, wid * scale);
    let (ox, oy) = (130.0, 40.0);
    let (vw, vh) = (ox + pw + 70.0, oy + ph + 130.0);
    let x = |u: f64| ox + u * scale;
    let y = |v: f64| oy + ph - v * scale;
    let f = |v: f64| format!("{:.1}", v);
    let mut s = String::new();
    let _ = write!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" font-family=\"Helvetica,Arial,sans-serif\">",
        f(vw),
        f(vh)
    );
    let _ = write!(
        s,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#fff\" stroke=\"#111\" stroke-width=\"1.2\"/>",
        f(ox),
        f(oy),
        f(pw),
        f(ph)
    );

    // Edges: a thick line where there is band, the supplier's name outside.
    for (name, face) in &r.sides.faces {
        let banded = p.edges.contains_key(face);
        let (x0, y0, x1, y1) = match face {
            Face::Bottom => (ox, oy + ph, ox + pw, oy + ph),
            Face::Top => (ox, oy, ox + pw, oy),
            Face::Left => (ox, oy, ox, oy + ph),
            _ => (ox + pw, oy, ox + pw, oy + ph),
        };
        if banded {
            let _ = write!(
                s,
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#0a7d2c\" stroke-width=\"4\"/>",
                f(x0),
                f(y0),
                f(x1),
                f(y1)
            );
        }
        let label = format!(
            "{name} · {}",
            if banded { "con canto" } else { "sin canto" }
        );
        let colour = if banded { "#0a7d2c" } else { "#666" };
        let _ = match face {
            Face::Bottom => write!(
                s,
                "<text x=\"{}\" y=\"{}\" font-size=\"12\" text-anchor=\"middle\" fill=\"{colour}\" font-weight=\"bold\">{label}</text>",
                f(ox + pw / 2.0),
                f(oy + ph + 100.0)
            ),
            Face::Top => write!(
                s,
                "<text x=\"{}\" y=\"{}\" font-size=\"12\" text-anchor=\"middle\" fill=\"{colour}\" font-weight=\"bold\">{label}</text>",
                f(ox + pw / 2.0),
                f(oy - 10.0)
            ),
            Face::Left => write!(
                s,
                "<text transform=\"translate({} {}) rotate(-90)\" font-size=\"12\" text-anchor=\"middle\" fill=\"{colour}\" font-weight=\"bold\">{label}</text>",
                f(ox - 112.0),
                f(oy + ph / 2.0)
            ),
            _ => write!(
                s,
                "<text transform=\"translate({} {}) rotate(90)\" font-size=\"12\" text-anchor=\"middle\" fill=\"{colour}\" font-weight=\"bold\">{label}</text>",
                f(ox + pw + 16.0),
                f(oy + ph / 2.0)
            ),
        };
    }

    // Features, and the coordinates worth a dimension.
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    for op in &p.operations {
        match &op.geometry {
            OpGeometry::Drill {
                u,
                v,
                diameter,
                depth,
                ..
            } => {
                let rr = (diameter / 2.0 * scale).max(2.0);
                match op.face {
                    Face::Front | Face::Back => {
                        let dash = if op.face == Face::Back {
                            " stroke-dasharray=\"3 2\""
                        } else {
                            ""
                        };
                        let colour = if op.face == Face::Back {
                            "#0659b5"
                        } else {
                            "#c00"
                        };
                        let _ = write!(
                            s,
                            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"none\" stroke=\"{colour}\" stroke-width=\"1\"{dash}/>",
                            f(x(*u)),
                            f(y(*v)),
                            f(rr)
                        );
                        xs.push(*u);
                        ys.push(*v);
                    }
                    edge_face => {
                        let dp = depth.unwrap_or(0.0) * scale;
                        let (x0, y0, x1, y1) = match edge_face {
                            Face::Left => (x(0.0), y(*u), x(0.0) + dp, y(*u)),
                            Face::Right => (x(len), y(*u), x(len) - dp, y(*u)),
                            Face::Bottom => (x(*u), y(0.0), x(*u), y(0.0) - dp),
                            _ => (x(*u), y(wid), x(*u), y(wid) + dp),
                        };
                        let _ = write!(
                            s,
                            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#c00\" stroke-width=\"{}\"/>",
                            f(x0),
                            f(y0),
                            f(x1),
                            f(y1),
                            f((diameter * scale).max(1.5))
                        );
                        match edge_face {
                            Face::Left | Face::Right => ys.push(*u),
                            _ => xs.push(*u),
                        }
                    }
                }
            }
            OpGeometry::Groove {
                from, to, width, ..
            } if matches!(op.face, Face::Front | Face::Back) => {
                let hw = width / 2.0;
                let (u0, u1) = (from[0].min(to[0]), from[0].max(to[0]));
                let (v0, v1) = (from[1].min(to[1]), from[1].max(to[1]));
                let (u0, u1, v0, v1) = if (v1 - v0).abs() < (u1 - u0).abs() {
                    (u0, u1, v0 - hw, v1 + hw)
                } else {
                    (u0 - hw, u1 + hw, v0, v1)
                };
                let _ = write!(
                    s,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#f6d9ea\" stroke=\"#a0336b\" stroke-width=\"0.8\"/>",
                    f(x(u0)),
                    f(y(v1)),
                    f((u1 - u0) * scale),
                    f((v1 - v0) * scale)
                );
                for u in [from[0], to[0]] {
                    if u > 0.05 && u < len - 0.05 {
                        xs.push(u);
                    }
                }
                for v in [from[1], to[1]] {
                    if v > 0.05 && v < wid - 0.05 {
                        ys.push(v);
                    }
                }
            }
            OpGeometry::Cutout {
                u,
                v,
                width,
                height,
                radius,
            } => {
                let _ = write!(
                    s,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" fill=\"#eee\" stroke=\"#a0336b\" stroke-width=\"1\" stroke-dasharray=\"4 2\"/>",
                    f(x(u - width / 2.0)),
                    f(y(v + height / 2.0)),
                    f(width * scale),
                    f(height * scale),
                    f(radius * scale)
                );
                xs.push(*u);
                ys.push(*v);
            }
            _ => {}
        }
    }

    // Ordinate dimensions: a tick per distinct coordinate and its value;
    // values too close to read apart move aside on a leader line.
    let distinct = |mut v: Vec<f64>| {
        v.iter_mut().for_each(|a| *a = (*a * 10.0).round() / 10.0);
        v.sort_by(f64::total_cmp);
        v.dedup_by(|a, b| (*a - *b).abs() < 0.05);
        v
    };
    let xs = distinct(xs);
    let at: Vec<f64> = xs.iter().map(|u| x(*u)).collect();
    for ((u, px), lx) in xs.iter().zip(&at).zip(spread(&at, 10.0)) {
        let (b0, b1) = (oy + ph, oy + ph + 16.0);
        let _ = write!(
            s,
            "<polyline points=\"{},{} {},{} {},{} {},{}\" fill=\"none\" stroke=\"#888\" stroke-width=\"0.6\"/><text transform=\"translate({} {}) rotate(-90)\" font-size=\"9\" text-anchor=\"end\">{}</text>",
            f(*px),
            f(b0),
            f(*px),
            f(b0 + 5.0),
            f(lx),
            f(b1 - 3.0),
            f(lx),
            f(b1),
            f(lx + 3.0),
            f(b1 + 2.0),
            mm(*u)
        );
    }
    // Y grows downwards on the page: spread upwards from the lowest value.
    let ys = distinct(ys);
    let at: Vec<f64> = ys.iter().map(|v| -y(*v)).collect();
    for ((v, py), ly) in ys.iter().zip(&at).zip(spread(&at, 11.0)) {
        let (py, ly) = (-py, -ly);
        let (b0, b1) = (ox, ox - 16.0);
        let _ = write!(
            s,
            "<polyline points=\"{},{} {},{} {},{} {},{}\" fill=\"none\" stroke=\"#888\" stroke-width=\"0.6\"/><text x=\"{}\" y=\"{}\" font-size=\"9\" text-anchor=\"end\">{}</text>",
            f(b0),
            f(py),
            f(b0 - 5.0),
            f(py),
            f(b1 + 3.0),
            f(ly),
            f(b1),
            f(ly),
            f(b1 - 2.0),
            f(ly + 3.0),
            mm(*v)
        );
    }

    // Overall size and the origin.
    let _ = write!(
        s,
        "<line x1=\"{0}\" y1=\"{2}\" x2=\"{1}\" y2=\"{2}\" stroke=\"#111\" stroke-width=\"0.8\"/>\
<text x=\"{3}\" y=\"{4}\" font-size=\"13\" text-anchor=\"middle\" font-weight=\"bold\">{5}</text>",
        f(ox),
        f(ox + pw),
        f(oy + ph + 70.0),
        f(ox + pw / 2.0),
        f(oy + ph + 84.0),
        mm(len)
    );
    let _ = write!(
        s,
        "<line x1=\"{0}\" y1=\"{1}\" x2=\"{0}\" y2=\"{2}\" stroke=\"#111\" stroke-width=\"0.8\"/>\
<text transform=\"translate({3} {4}) rotate(-90)\" font-size=\"13\" text-anchor=\"middle\" font-weight=\"bold\">{5}</text>",
        f(ox - 75.0),
        f(oy),
        f(oy + ph),
        f(ox - 82.0),
        f(oy + ph / 2.0),
        mm(wid)
    );
    let _ = write!(
        s,
        "<circle cx=\"{}\" cy=\"{}\" r=\"3.5\" fill=\"#111\"/><text x=\"{}\" y=\"{}\" font-size=\"11\" font-weight=\"bold\">0,0</text>",
        f(ox),
        f(oy + ph),
        f(ox - 28.0),
        f(oy + ph + 22.0)
    );
    // Legend under the drawing, the grain beside the L2 name.
    let _ = write!(
        s,
        "<text x=\"{}\" y=\"{}\" font-size=\"11\" fill=\"#444\">Vista desde la cara A (X →, Y ↑). <tspan fill=\"#c00\">○ perforación cara A</tspan> · <tspan fill=\"#0659b5\">◌ cara B (vista a través)</tspan> · <tspan fill=\"#c00\">▬ perforación de canto</tspan> · <tspan fill=\"#a0336b\">▭ ranura</tspan></text>",
        f(8.0),
        f(vh - 6.0)
    );
    if r.sides.grain {
        let arrow = if r.sides.faces[0].1 == Face::Bottom {
            "veta ⟷"
        } else {
            "veta ↕"
        };
        let _ = write!(
            s,
            "<text x=\"{}\" y=\"{}\" font-size=\"11\" fill=\"#8a5a2b\" text-anchor=\"end\">{arrow}</text>",
            f(ox + pw),
            f(oy - 10.0)
        );
    }
    s += "</svg>";
    s
}

/// Label positions for sorted `targets`, at least `gap` apart, each as
/// close to its target as the others let it be: pushed forward where
/// they crowd, then pulled back so the group stays centred on its
/// targets instead of drifting one way.
fn spread(targets: &[f64], gap: f64) -> Vec<f64> {
    let mut p: Vec<f64> = targets.to_vec();
    for i in 1..p.len() {
        p[i] = p[i].max(p[i - 1] + gap);
    }
    // Clusters that were pushed move back by half of how far they went.
    let mut i = 0;
    while i < p.len() {
        let mut j = i;
        while j + 1 < p.len() && p[j + 1] - p[j] <= gap + 1e-9 {
            j += 1;
        }
        let shift = (i..=j).map(|k| p[k] - targets[k]).sum::<f64>() / (j - i + 1) as f64;
        let floor = if i == 0 {
            f64::NEG_INFINITY
        } else {
            p[i - 1] + gap
        };
        let shift = shift.min(p[i] - floor);
        for q in &mut p[i..=j] {
            *q -= shift;
        }
        i = j + 1;
    }
    p
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
        assert_eq!(html.matches("<section class=\"sheet\">").count(), drawings);
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
