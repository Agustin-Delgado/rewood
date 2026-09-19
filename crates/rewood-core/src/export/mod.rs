//! The manufacturing package: what leaves the engine and reaches the
//! workshop. Pure functions producing file contents; the CLI writes them.
//!
//! ```text
//! <order>/
//! ├── parts/P001.dxf …        one CAM drawing per part
//! ├── cnc/P001_A.nc …         NC program per part and setup (A front up, B back up)
//! ├── cnc/toolpaths.json      the machine-independent programs
//! ├── bom/parts.csv           cut list
//! ├── bom/hardware.csv        fasteners and their sub-items
//! ├── bom/consumables.csv     edge band metres, sheet estimate
//! ├── documentation/report.html      printable: cut list, BOM, views, drawings, labels
//! ├── documentation/assembly.svg     front / side / top views
//! ├── documentation/nesting.svg      sheet layouts
//! ├── documentation/parts/P001.svg … dimensioned drawing per part
//! ├── documentation/assembly.txt     step-by-step manual from the joint graph
//! ├── documentation/cutlist.txt
//! ├── documentation/operations.csv   every hole and groove, one row each
//! ├── labels/labels.svg       one label with QR per part
//! ├── plan.json               the full plan, canonical JSON
//! └── manifest.json           versions, status, file list
//! ```

pub mod assembly;
pub mod dxf;
pub mod explode;
pub mod svg;

use std::fmt::Write;

use crate::model::OpGeometry;
use crate::plan::ManufacturingPlan;
use crate::units::round3;

#[derive(Debug, Clone, PartialEq)]
pub struct PackageFile {
    /// Relative path inside the package, with `/` separators.
    pub path: String,
    pub contents: String,
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn csv_row(cells: &[String]) -> String {
    cells
        .iter()
        .map(|c| csv_cell(c))
        .collect::<Vec<_>>()
        .join(",")
        + "\n"
}

fn n(v: f64) -> String {
    let r = round3(v);
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

pub fn parts_csv(plan: &ManufacturingPlan) -> String {
    let mut out = csv_row(
        &[
            "part_ids",
            "name",
            "quantity",
            "material",
            "cut_length",
            "cut_width",
            "finished_length",
            "finished_width",
            "thickness",
            "grain",
            "edges",
            "operations",
            "weight_kg",
        ]
        .map(String::from),
    );
    for r in &plan.part_list {
        out += &csv_row(&[
            r.part_ids.join(" "),
            r.name.clone(),
            r.quantity.to_string(),
            r.material.clone(),
            n(r.cut_length),
            n(r.cut_width),
            n(r.finished_length),
            n(r.finished_width),
            n(r.thickness),
            format!("{:?}", r.grain).to_lowercase(),
            r.edges.clone(),
            r.operations.to_string(),
            n(r.weight_kg),
        ]);
    }
    out
}

pub fn hardware_csv(plan: &ManufacturingPlan) -> String {
    let mut out = csv_row(&["hardware", "item", "quantity"].map(String::from));
    for h in &plan.bom.hardware {
        out += &csv_row(&[h.hardware.clone(), h.name.clone(), h.quantity.to_string()]);
        for i in &h.items {
            out += &csv_row(&[h.hardware.clone(), format!("  {}", i.name), n(i.quantity)]);
        }
    }
    out
}

/// One purchase order as a CSV a supplier can read: what, how much, at
/// what price, plus a header line with who it is for.
pub fn purchase_order_csv(po: &crate::plan::PurchaseOrder, currency: &str) -> String {
    let mut out = csv_row(&["supplier", "name", "lead_days", "currency"].map(String::from));
    out += &csv_row(&[
        po.supplier.clone(),
        po.name.clone(),
        po.lead_days.to_string(),
        currency.to_string(),
    ]);
    out += &csv_row(
        &[
            "kind",
            "id",
            "item",
            "quantity",
            "unit",
            "unit_price",
            "cost",
        ]
        .map(String::from),
    );
    for l in &po.lines {
        out += &csv_row(&[
            l.kind.clone(),
            l.id.clone(),
            l.name.clone(),
            n(l.quantity),
            l.unit.clone(),
            n(l.unit_price),
            n(l.cost),
        ]);
    }
    out += &csv_row(&[
        "total".into(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        n(po.cost),
    ]);
    out
}

pub fn consumables_csv(plan: &ManufacturingPlan) -> String {
    let mut out = csv_row(&["kind", "id", "name", "quantity", "unit"].map(String::from));
    for s in &plan.bom.sheets {
        out += &csv_row(&[
            "sheet".into(),
            s.material.clone(),
            format!("{} ({}x{})", s.name, n(s.sheet_length), n(s.sheet_width)),
            s.estimated_sheets.to_string(),
            "sheets (estimate)".into(),
        ]);
    }
    for c in &plan.bom.consumables {
        out += &csv_row(&[
            "edge_band".into(),
            c.material.clone(),
            c.name.clone(),
            n(c.length_m),
            "m".into(),
        ]);
    }
    out
}

/// Every machining operation as a flat row, the way a drilling machine's
/// operator or a nesting tool wants it.
pub fn operations_csv(plan: &ManufacturingPlan) -> String {
    let mut out = csv_row(
        &[
            "part",
            "op",
            "type",
            "face",
            "u",
            "v",
            "u2",
            "v2",
            "diameter_or_width",
            "depth",
            "through",
            "source",
        ]
        .map(String::from),
    );
    for p in &plan.parts {
        for op in &p.operations {
            let source = op
                .source
                .as_ref()
                .map(|s| format!("{}/{}#{}:{}", s.joint, s.hardware, s.fastener, s.label))
                .unwrap_or_default();
            let face = serde_json::to_value(op.face)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string();
            let row = match &op.geometry {
                OpGeometry::Drill {
                    u,
                    v,
                    diameter,
                    depth,
                    through,
                    ..
                } => vec![
                    p.id.clone(),
                    op.id.clone(),
                    "DRILL".into(),
                    face,
                    n(*u),
                    n(*v),
                    String::new(),
                    String::new(),
                    n(*diameter),
                    depth.map(n).unwrap_or_default(),
                    through.to_string(),
                    source,
                ],
                OpGeometry::Groove {
                    from,
                    to,
                    width,
                    depth,
                } => vec![
                    p.id.clone(),
                    op.id.clone(),
                    "GROOVE".into(),
                    face,
                    n(from[0]),
                    n(from[1]),
                    n(to[0]),
                    n(to[1]),
                    n(*width),
                    n(*depth),
                    "false".into(),
                    source,
                ],
                OpGeometry::EdgeBand {
                    material,
                    thickness,
                    length,
                } => vec![
                    p.id.clone(),
                    op.id.clone(),
                    "EDGE_BAND".into(),
                    face,
                    "0".into(),
                    "0".into(),
                    n(*length),
                    "0".into(),
                    n(*thickness),
                    String::new(),
                    "false".into(),
                    material.clone(),
                ],
            };
            out += &csv_row(&row);
        }
    }
    out
}

pub fn cutlist_txt(plan: &ManufacturingPlan) -> String {
    let mut s = String::new();
    writeln!(
        s,
        "Despiece — {} ({}) v{}",
        plan.furniture.name, plan.furniture.id, plan.furniture.version
    )
    .unwrap();
    writeln!(
        s,
        "Motor {} · materiales {} · herrajes {} · perfil {}",
        plan.versions.engine,
        plan.versions.materials,
        plan.versions.hardware,
        plan.versions.profile
    )
    .unwrap();
    writeln!(s).unwrap();
    writeln!(
        s,
        "{:<14} {:<22} {:>4} {:>8} {:>8} {:>5}  {:<7} {:<34} {:>4}",
        "ID", "Pieza", "Cant", "Largo", "Ancho", "Esp", "Veta", "Cantos (izq/der/inf/sup)", "Ops"
    )
    .unwrap();
    for r in &plan.part_list {
        writeln!(
            s,
            "{:<14} {:<22} {:>4} {:>8} {:>8} {:>5}  {:<7} {:<34} {:>4}",
            r.part_ids.join(","),
            r.name,
            r.quantity,
            n(r.cut_length),
            n(r.cut_width),
            n(r.thickness),
            format!("{:?}", r.grain).to_lowercase(),
            r.edges,
            r.operations
        )
        .unwrap();
    }
    writeln!(s).unwrap();
    writeln!(s, "Las medidas de corte ya descuentan el canto; las perforaciones se miden sobre la pieza terminada.").unwrap();
    writeln!(s, "Placas estimadas por área y desperdicio, sin nesting:").unwrap();
    for sh in &plan.bom.sheets {
        writeln!(
            s,
            "  {:<28} {:>3} piezas  {:>7.3} m²  ≈ {} placa(s) de {}×{}",
            sh.name,
            sh.parts,
            sh.net_area_m2,
            sh.estimated_sheets,
            n(sh.sheet_length),
            n(sh.sheet_width)
        )
        .unwrap();
    }
    writeln!(s, "Herrajes:").unwrap();
    for h in &plan.bom.hardware {
        writeln!(s, "  {:<28} {:>4}", h.name, h.quantity).unwrap();
        for i in &h.items {
            writeln!(s, "      {:<24} {:>4}", i.name, n(i.quantity)).unwrap();
        }
    }
    writeln!(s, "Consumibles:").unwrap();
    for c in &plan.bom.consumables {
        writeln!(s, "  {:<28} {:>7.2} m", c.name, c.length_m).unwrap();
    }
    writeln!(s, "Peso total: {:.2} kg", plan.bom.total_weight_kg).unwrap();
    s
}

const DXF_README: &str = "\
Convenciones de los DXF de piezas
=================================

- Unidades: milímetros. Origen: esquina inferior izquierda de la pieza vista
  desde su cara FRONT (la cara preferida de mecanizado: el interior de la
  carcasa, el dorso de una puerta). X = largo, Y = ancho.
- OUTLINE: contorno de la pieza terminada (después del canto). La medida de
  corte en bruto está en bom/parts.csv (cut_length / cut_width).
- DRILL_FRONT_D<diámetro>_L<profundidad>: perforación vertical desde FRONT.
  DRILL_BACK_...: desde la cara opuesta, dibujada en la misma (X, Y).
  THRU en vez de L<n> = pasante.
- DRILL_EDGE_<LEFT|RIGHT|BOTTOM|TOP>_D<d>_L<p>: perforación horizontal en el
  canto, dibujada como una línea desde el canto hacia adentro, tan larga como
  profunda; siempre centrada en el espesor.
- GROOVE_<cara>_W<ancho>_L<profundidad>: huella de la ranura.
- EDGE_BAND_<espesor>: cantos que llevan tapacanto.
- LABEL: identificador de la pieza.
";

fn html_esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

const REPORT_CSS: &str = "body{font-family:Helvetica,Arial,sans-serif;font-size:12px;color:#111;margin:24px}\
h1{font-size:20px;margin:0 0 4px}h2{font-size:15px;margin:22px 0 8px;border-bottom:1px solid #999;padding-bottom:3px}\
table{border-collapse:collapse;margin:6px 0}th,td{border:1px solid #bbb;padding:3px 7px;text-align:left;font-size:11px}th{background:#eee}\
td.num,th.num{text-align:right}.muted{color:#666}.page{page-break-before:always}svg{max-width:100%;height:auto}\
.diag{padding:2px 6px;margin:2px 0;border-left:4px solid #999}.FATAL{border-color:#900;background:#fee}.ERROR{border-color:#c40;background:#fee8e0}\
.WARNING{border-color:#c90;background:#fff6dd}.INFO{border-color:#39c;background:#eef6fc}\
.steps li{margin:8px 0}.steps ul{margin:2px 0 4px;color:#333}@media print{body{margin:10mm}}";

/// One self-contained HTML page the browser prints to PDF: header,
/// diagnostics, cut list, BOM, assembly views, one page per part drawing,
/// and the label sheet. All SVG inline.
fn minutes(seconds: f64) -> String {
    let total = seconds.round() as i64;
    if total >= 3600 {
        format!("{} h {:02} min", total / 3600, (total % 3600) / 60)
    } else {
        format!("{} min {:02} s", total / 60, total % 60)
    }
}

pub fn report_html(plan: &ManufacturingPlan) -> String {
    let mut h = String::new();
    writeln!(
        h,
        "<!DOCTYPE html><html lang=\"es\"><head><meta charset=\"utf-8\"><title>{} — paquete de fabricación</title><style>{REPORT_CSS}</style></head><body>",
        html_esc(&plan.furniture.name)
    )
    .unwrap();
    writeln!(
        h,
        "<h1>{}</h1><div class=\"muted\">{} v{} · estado <b>{:?}</b> · motor {} · materiales {} · herrajes {} · perfil {}</div>",
        html_esc(&plan.furniture.name),
        html_esc(&plan.furniture.id),
        html_esc(&plan.furniture.version),
        plan.status,
        plan.versions.engine,
        plan.versions.materials,
        plan.versions.hardware,
        html_esc(&plan.versions.profile)
    )
    .unwrap();
    if plan.manufacturing_blocked {
        h.push_str("<p style=\"color:#900;font-weight:bold\">FABRICACIÓN BLOQUEADA: hay hallazgos fatales.</p>");
    }

    if !plan.diagnostics.items.is_empty() {
        h.push_str("<h2>Hallazgos</h2>");
        for d in &plan.diagnostics.items {
            let sev = serde_json::to_value(d.severity)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string();
            writeln!(
                h,
                "<div class=\"diag {sev}\"><b>{sev} {}</b> {} — {}{}</div>",
                html_esc(&d.code),
                html_esc(d.entity.as_deref().unwrap_or("")),
                html_esc(&d.message),
                d.suggestion
                    .as_ref()
                    .map(|s| format!(" <i>{}</i>", html_esc(s)))
                    .unwrap_or_default()
            )
            .unwrap();
        }
    }

    h.push_str("<h2>Despiece</h2><table><tr><th>ID</th><th>Pieza</th><th class=num>Cant</th><th>Material</th><th class=num>Corte largo</th><th class=num>Corte ancho</th><th class=num>Terminada</th><th class=num>Esp</th><th>Veta</th><th>Cantos izq/der/inf/sup</th><th class=num>Ops</th><th class=num>kg</th></tr>");
    for r in &plan.part_list {
        writeln!(
            h,
            "<tr><td>{}</td><td>{}</td><td class=num>{}</td><td>{}</td><td class=num>{}</td><td class=num>{}</td><td class=num>{}×{}</td><td class=num>{}</td><td>{}</td><td>{}</td><td class=num>{}</td><td class=num>{:.2}</td></tr>",
            html_esc(&r.part_ids.join(", ")),
            html_esc(&r.name),
            r.quantity,
            html_esc(&r.material),
            n(r.cut_length),
            n(r.cut_width),
            n(r.finished_length),
            n(r.finished_width),
            n(r.thickness),
            svg::grain_es(r.grain),
            html_esc(&r.edges),
            r.operations,
            r.weight_kg * r.quantity as f64
        )
        .unwrap();
    }
    h.push_str("</table><p class=muted>Las medidas de corte ya descuentan el canto; las perforaciones se miden sobre la pieza terminada.</p>");

    h.push_str("<h2>Materiales y herrajes</h2><table><tr><th>Placa</th><th class=num>Piezas</th><th class=num>m² netos</th><th class=num>Placas (nesting)</th><th class=num>Aprovech.</th></tr>");
    for sh in &plan.bom.sheets {
        writeln!(
            h,
            "<tr><td>{} ({}×{})</td><td class=num>{}</td><td class=num>{:.3}</td><td class=num>{}</td><td class=num>{:.0}%</td></tr>",
            html_esc(&sh.name),
            n(sh.sheet_length),
            n(sh.sheet_width),
            sh.parts,
            sh.net_area_m2,
            sh.estimated_sheets,
            sh.yield_ratio * 100.0
        )
        .unwrap();
    }
    h.push_str("</table><table><tr><th>Herraje</th><th class=num>Cantidad</th></tr>");
    for hw in &plan.bom.hardware {
        writeln!(
            h,
            "<tr><td><b>{}</b></td><td class=num>{}</td></tr>",
            html_esc(&hw.name),
            hw.quantity
        )
        .unwrap();
        for i in &hw.items {
            writeln!(
                h,
                "<tr><td class=muted>&nbsp;&nbsp;{}</td><td class=num>{}</td></tr>",
                html_esc(&i.name),
                n(i.quantity)
            )
            .unwrap();
        }
    }
    for c in &plan.bom.consumables {
        writeln!(
            h,
            "<tr><td>{}</td><td class=num>{:.2} m</td></tr>",
            html_esc(&c.name),
            c.length_m
        )
        .unwrap();
    }
    writeln!(
        h,
        "</table><p>Peso total: <b>{:.2} kg</b></p>",
        plan.bom.total_weight_kg
    )
    .unwrap();
    if plan.bom.total_cost > 0.0 {
        let b = &plan.bom;
        writeln!(
            h,
            "<h3>Costo estimado</h3><table><tr><th>Concepto</th><th class=num>{}</th></tr>",
            html_esc(&b.currency)
        )
        .unwrap();
        for s in &b.sheets {
            writeln!(
                h,
                "<tr><td>{} × {}</td><td class=num>{:.2}</td></tr>",
                s.estimated_sheets,
                html_esc(&s.name),
                s.cost
            )
            .unwrap();
        }
        for hw in &b.hardware {
            writeln!(
                h,
                "<tr><td>{} × {}</td><td class=num>{:.2}</td></tr>",
                hw.quantity,
                html_esc(&hw.name),
                hw.cost
            )
            .unwrap();
        }
        for c in &b.consumables {
            writeln!(
                h,
                "<tr><td>{:.2} m {}</td><td class=num>{:.2}</td></tr>",
                c.length_m,
                html_esc(&c.name),
                c.cost
            )
            .unwrap();
        }
        writeln!(
            h,
            "<tr><td>Máquina ({})</td><td class=num>{:.2}</td></tr><tr><td><b>Total</b></td><td class=num><b>{:.2}</b></td></tr></table>",
            minutes(plan.machining.total_seconds),
            b.machining_cost,
            b.total_cost
        )
        .unwrap();
        if !b.unpriced.is_empty() {
            writeln!(
                h,
                "<p class=muted>Sin precio en la biblioteca (el total es un piso): {}</p>",
                html_esc(&b.unpriced.join(", "))
            )
            .unwrap();
        }
    }

    if !plan.purchasing.is_empty() {
        h.push_str("<h2>Compras por proveedor</h2>");
        for po in &plan.purchasing {
            writeln!(
                h,
                "<h3>{}{}</h3><table><tr><th>Ítem</th><th class=num>Cantidad</th><th>Unidad</th><th class=num>Precio</th><th class=num>Costo</th></tr>",
                html_esc(&po.name),
                if po.lead_days > 0 { format!(" · entrega ≈ {} días", po.lead_days) } else { String::new() }
            )
            .unwrap();
            for l in &po.lines {
                writeln!(
                    h,
                    "<tr><td>{}</td><td class=num>{}</td><td>{}</td><td class=num>{}</td><td class=num>{}</td></tr>",
                    html_esc(&l.name),
                    n(l.quantity),
                    html_esc(&l.unit),
                    if l.unit_price > 0.0 { format!("{:.2}", l.unit_price) } else { "—".into() },
                    if l.cost > 0.0 { format!("{:.2}", l.cost) } else { "—".into() }
                )
                .unwrap();
            }
            writeln!(
                h,
                "<tr><td><b>Total</b></td><td></td><td></td><td></td><td class=num><b>{}</b></td></tr></table>",
                if po.cost > 0.0 { format!("{:.2} {}", po.cost, html_esc(&plan.bom.currency)) } else { "—".into() }
            )
            .unwrap();
        }
    }

    if !plan.machining.programs.is_empty() {
        let m = &plan.machining;
        writeln!(
            h,
            "<h2>Mecanizado CNC</h2><p>Post <code>{}</code> · {} programas · tiempo de máquina estimado <b>{}</b> (avances del perfil; sin carga ni canteado).</p>",
            html_esc(&m.post_processor),
            m.programs.len(),
            minutes(m.total_seconds)
        )
        .unwrap();
        h.push_str("<table><tr><th>Programa</th><th class=num>Ops</th><th class=num>Cambios</th><th class=num>Corte (m)</th><th class=num>Rápidos (m)</th><th class=num>Tiempo</th></tr>");
        for p in &m.programs {
            writeln!(
                h,
                "<tr><td class=mono>{}_{}</td><td class=num>{}</td><td class=num>{}</td><td class=num>{:.2}</td><td class=num>{:.2}</td><td class=num>{}</td></tr>",
                p.part,
                p.setup,
                p.operations,
                p.tool_changes,
                p.cut_mm / 1000.0,
                p.rapid_mm / 1000.0,
                minutes(p.seconds)
            )
            .unwrap();
        }
        h.push_str("</table>");
    }

    h.push_str("<h2>Vistas</h2>");
    h.push_str(&svg::assembly_views_svg(plan));

    if !plan.nesting.is_empty() {
        h.push_str("<div class=page></div><h2>Plano de corte</h2>");
        h.push_str(&svg::nesting_svg(plan));
    }

    h.push_str("<div class=page></div><h2>Manual de armado</h2>");
    h.push_str(&assembly::manual_html(plan));

    for p in &plan.parts {
        h.push_str("<div class=page></div>");
        h.push_str(&svg::part_drawing_svg(p));
    }

    h.push_str("<div class=page></div><h2>Etiquetas</h2>");
    h.push_str(&svg::labels_svg(plan));
    h.push_str("</body></html>\n");
    h
}

const CNC_README: &str = "\
Programas NC
============

- Un programa por pieza y por puesta: P001_A.nc con la cara FRONT hacia
  arriba (perforaciones y ranuras del frente, perforaciones de canto y el
  contorno), P001_B.nc sólo si el dorso tiene mecanizados, con la pieza
  girada sobre su eje X (largo): un punto (u, v) del dorso queda en
  (u, ancho − v).
- Origen: esquina inferior izquierda de la pieza terminada; Z0 en la cara
  superior; mm; G54.
- El contorno se corta al final de la puesta A, con la fresa por fuera del
  rectángulo terminado, en pasadas, 0,5 mm más profundo que el espesor.
- Postprocesador ISO genérico: G81/G83 para taladrar, G01 para ranuras y
  contorno. Las perforaciones horizontales salen como comentarios (HDRILL
  ...) porque el ISO genérico no tiene agregado horizontal: un post por
  máquina (Biesse, SCM, Homag...) las traduce.
- toolpaths.json es la versión independiente de máquina de los mismos
  programas.
- simulation.json es el resultado de interpretar cada .nc como lo haría el
  control (§28): cortes que produce, longitud de corte y de rápidos, cambios
  de herramienta y tiempo estimado. Cualquier diferencia entre lo que el NC
  corta y lo que el programa pide sale como CAM-30x en el plan.
- BLANK ... ROTATED 90 en la cabecera: la pieza se carga girada porque sólo
  entra en la mesa de costado; las coordenadas ya están giradas.
";

/// All files of the package, sorted by path. Deterministic.
pub fn package(plan: &ManufacturingPlan) -> Vec<PackageFile> {
    let mut files = Vec::new();
    // §33: a fatal finding blocks manufacturing. The package then carries
    // what explains the block and nothing a workshop could cut from.
    if plan.manufacturing_blocked {
        let mut txt = String::from(
            "FABRICACIÓN BLOQUEADA\n\nEste paquete no trae programas, DXF ni despiece: la especificación tiene hallazgos fatales.\nCorregilos y volvé a exportar.\n\n",
        );
        for d in &plan.diagnostics.items {
            if d.severity == crate::diagnostics::Severity::Fatal {
                let _ = writeln!(
                    txt,
                    "{} {}{}",
                    d.code,
                    d.entity
                        .as_deref()
                        .map(|e| format!("[{e}] "))
                        .unwrap_or_default(),
                    d.message
                );
                if let Some(s) = &d.suggestion {
                    let _ = writeln!(txt, "    → {s}");
                }
            }
        }
        files.push(PackageFile {
            path: "BLOQUEADO.txt".into(),
            contents: txt,
        });
        files.push(PackageFile {
            path: "documentation/report.html".into(),
            contents: report_html(plan),
        });
        files.push(PackageFile {
            path: "plan.json".into(),
            contents: plan.to_json_pretty() + "\n",
        });
        return with_manifest(plan, files);
    }
    for p in &plan.parts {
        files.push(PackageFile {
            path: format!("parts/{}.dxf", p.id),
            contents: dxf::part_dxf(p),
        });
        files.push(PackageFile {
            path: format!("documentation/parts/{}.svg", p.id),
            contents: svg::part_drawing_svg(p),
        });
    }
    use crate::cam::PostProcessor;
    let post = crate::cam::GenericIso::default();
    let mut programs = Vec::new();
    let mut simulations = Vec::new();
    let mut cam_diags = crate::diagnostics::Diagnostics::default();
    for p in &plan.parts {
        for program in crate::cam::programs(p, &plan.profile, &mut cam_diags) {
            let nc = post.render(&program);
            simulations.push(crate::simulation::simulate(
                &program,
                &nc,
                &plan.profile,
                &mut cam_diags,
            ));
            files.push(PackageFile {
                path: format!("cnc/{}.{}", program.name(), post.extension()),
                contents: nc,
            });
            programs.push(program);
        }
    }
    files.push(PackageFile {
        path: "cnc/toolpaths.json".into(),
        contents: serde_json::to_string_pretty(&programs).unwrap() + "\n",
    });
    files.push(PackageFile {
        path: "cnc/simulation.json".into(),
        contents: serde_json::to_string_pretty(&simulations).unwrap() + "\n",
    });
    files.push(PackageFile {
        path: "cnc/README.txt".into(),
        contents: CNC_README.into(),
    });
    files.push(PackageFile {
        path: "documentation/assembly.svg".into(),
        contents: svg::assembly_views_svg(plan),
    });
    files.push(PackageFile {
        path: "documentation/nesting.svg".into(),
        contents: svg::nesting_svg(plan),
    });
    files.push(PackageFile {
        path: "documentation/exploded.svg".into(),
        contents: explode::exploded_svg(plan),
    });
    files.push(PackageFile {
        path: "documentation/report.html".into(),
        contents: report_html(plan),
    });
    files.push(PackageFile {
        path: "labels/labels.svg".into(),
        contents: svg::labels_svg(plan),
    });
    files.push(PackageFile {
        path: "parts/README.txt".into(),
        contents: DXF_README.into(),
    });
    files.push(PackageFile {
        path: "bom/parts.csv".into(),
        contents: parts_csv(plan),
    });
    files.push(PackageFile {
        path: "bom/hardware.csv".into(),
        contents: hardware_csv(plan),
    });
    files.push(PackageFile {
        path: "bom/consumables.csv".into(),
        contents: consumables_csv(plan),
    });
    for po in &plan.purchasing {
        let id = if po.supplier.is_empty() {
            "sin_proveedor"
        } else {
            po.supplier.as_str()
        };
        files.push(PackageFile {
            path: format!("purchasing/{id}.csv"),
            contents: purchase_order_csv(po, &plan.bom.currency),
        });
    }
    files.push(PackageFile {
        path: "documentation/assembly.txt".into(),
        contents: assembly::manual_txt(plan),
    });
    files.push(PackageFile {
        path: "documentation/cutlist.txt".into(),
        contents: cutlist_txt(plan),
    });
    files.push(PackageFile {
        path: "documentation/operations.csv".into(),
        contents: operations_csv(plan),
    });
    files.push(PackageFile {
        path: "plan.json".into(),
        contents: plan.to_json_pretty() + "\n",
    });
    with_manifest(plan, files)
}

fn with_manifest(plan: &ManufacturingPlan, mut files: Vec<PackageFile>) -> Vec<PackageFile> {
    let manifest = serde_json::json!({
        "furniture": plan.furniture,
        "versions": plan.versions,
        "status": plan.status,
        "manufacturingBlocked": plan.manufacturing_blocked,
        "parts": plan.parts.len(),
        "joints": plan.joints.len(),
        "diagnostics": {
            "fatal": plan.diagnostics.count(crate::diagnostics::Severity::Fatal),
            "error": plan.diagnostics.count(crate::diagnostics::Severity::Error),
            "warning": plan.diagnostics.count(crate::diagnostics::Severity::Warning),
        },
        "files": files.iter().map(|f| f.path.clone()).collect::<Vec<_>>(),
    });
    files.push(PackageFile {
        path: "manifest.json".into(),
        contents: serde_json::to_string_pretty(&manifest).unwrap() + "\n",
    });
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blocked_plan_packages_nothing_a_workshop_could_cut_from() {
        let spec = include_str!("../../../../fixtures/invalid_cabinet/input.json");
        let plan = crate::compile_json(spec);
        assert!(plan.manufacturing_blocked);
        let files = package(&plan);
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "BLOQUEADO.txt",
                "documentation/report.html",
                "manifest.json",
                "plan.json"
            ]
        );
        let txt = &files[0].contents;
        assert!(txt.contains("LIB-101"), "{txt}");
        assert!(txt.contains("SPEC-401"), "{txt}");
    }

    #[test]
    fn package_has_one_dxf_per_part_and_is_deterministic() {
        let spec = include_str!("../../../../fixtures/basic_cabinet/input.json");
        let plan = crate::compile_json(spec);
        let files = package(&plan);
        let dxfs = files.iter().filter(|f| f.path.ends_with(".dxf")).count();
        assert_eq!(dxfs, plan.parts.len());
        let svgs = files
            .iter()
            .filter(|f| f.path.starts_with("documentation/parts/"))
            .count();
        assert_eq!(svgs, plan.parts.len());
        let report = files
            .iter()
            .find(|f| f.path == "documentation/report.html")
            .unwrap();
        assert!(report.contents.contains("<h2>Despiece</h2>"));
        assert_eq!(
            report.contents.matches("<svg").count(),
            plan.parts.len() + 3
        );
        assert!(files.iter().any(|f| f.path == "labels/labels.svg"));
        assert!(files.iter().any(|f| f.path == "manifest.json"));
        assert_eq!(files, package(&plan));
        let parts = files.iter().find(|f| f.path == "bom/parts.csv").unwrap();
        assert!(parts.contents.lines().count() == plan.part_list.len() + 1);
        let ops = files
            .iter()
            .find(|f| f.path == "documentation/operations.csv")
            .unwrap();
        let total_ops: usize = plan.parts.iter().map(|p| p.operations.len()).sum();
        assert_eq!(ops.contents.lines().count(), total_ops + 1);
    }
}
