//! Drawings as SVG: one per part with dimensions and a hole table, three
//! orthographic views of the whole furniture, and the label sheet with a
//! QR per part. SVG because a browser prints it to PDF as-is and the UI
//! can show it inline; nothing here depends on a PDF library.
//!
//! Every SVG is deterministic: same plan, same bytes.

use std::fmt::Write;

use crate::geometry::Face;
use crate::model::{OpGeometry, Part};
use crate::plan::ManufacturingPlan;
use crate::units::round3;

fn n(v: f64) -> String {
    let r = round3(v);
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(super) const STYLE: &str = "\
<style>\
text{font-family:Helvetica,Arial,sans-serif;fill:#111}\
.outline{fill:#fff;stroke:#111;stroke-width:1.2}\
.hole{fill:none;stroke:#c00;stroke-width:0.8}\
.hole-back{fill:none;stroke:#06c;stroke-width:0.8;stroke-dasharray:3 2}\
.edge-hole{stroke:#c00;stroke-width:1.2}\
.groove{fill:#fde;stroke:#a06;stroke-width:0.6}\
.band{stroke:#0a0;stroke-width:3}\
.dim{stroke:#555;stroke-width:0.6}\
.dimtext{font-size:9px;fill:#333;text-anchor:middle}\
.title{font-size:13px;font-weight:bold}\
.small{font-size:9px}\
.tiny{font-size:7px}\
.row{font-size:10px;font-family:ui-monospace,Menlo,Consolas,monospace}\
.part{fill:#e8eef6;stroke:#345;stroke-width:0.8}\
.part-front{fill:#f6e8c8;stroke:#765;stroke-width:0.8}\
.label-box{fill:#fff;stroke:#888;stroke-width:0.6}\
.iso-top{fill:#f3f6fa;stroke:#345;stroke-width:0.6}\
.iso-front{fill:#dfe7f1;stroke:#345;stroke-width:0.6}\
.iso-right{fill:#c9d4e2;stroke:#345;stroke-width:0.6}\
.iso-top.iso-tinted{fill:#fbf3e3}\
.iso-front.iso-tinted{fill:#f4e4c4}\
.iso-right.iso-tinted{fill:#e6d2ac}\
.iso-top.iso-hw{fill:#d0d4da}\
.iso-front.iso-hw{fill:#b4bac2}\
.iso-right.iso-hw{fill:#979ea8}\
.part-hw{fill:#b4bac2;stroke:#345;stroke-width:0.6}\
.see-through{fill-opacity:0.5}\
.cut1{stroke:#c00;stroke-width:1.2}\
.cut2{stroke:#06c;stroke-width:0.9}\
.cut3{stroke:#0a0;stroke-width:0.7;stroke-dasharray:3 2}\
.cutno{fill:#c00}\
</style>";

/// Doors and drawer fronts: drawn last and in a different tone.
pub(super) fn is_front(p: &Part) -> bool {
    p.role.contains("door") || (p.role.ends_with("_front") && !p.role.contains("box_front"))
}

pub fn grain_es(g: crate::model::Grain) -> &'static str {
    match g {
        crate::model::Grain::Length => "a lo largo",
        crate::model::Grain::Width => "a lo ancho",
        crate::model::Grain::None => "sin veta",
    }
}

/// Scale so that `len` mm fits in `px` pixels.
fn fit(len: f64, px: f64) -> f64 {
    px / len
}

pub fn part_drawing_svg(part: &Part) -> String {
    let (len, wid) = (part.dims.length, part.dims.width);
    // Drawing area 1000×360 px for the panel, margins for dimensions, the
    // operations table in columns underneath: printed on a landscape A4
    // the panel spans the page and the table stays legible.
    let scale = fit(len, 1000.0).min(fit(wid, 360.0));
    let (pw, ph) = (len * scale, wid * scale);
    let (ox, oy) = (60.0, 60.0);
    const WIDTH: f64 = 1120.0;
    const COLUMNS: usize = 3;
    const ROW: f64 = 12.0;
    let mut holes_rows: Vec<String> = Vec::new();
    let mut s = String::new();
    // Y grows downwards in SVG; the panel's v axis grows upwards.
    let y = |v: f64| oy + ph - v * scale;
    let x = |u: f64| ox + u * scale;

    writeln!(s, "<g>").unwrap();
    writeln!(
        s,
        "<rect class=\"outline\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
        n(ox),
        n(oy),
        n(pw),
        n(ph)
    )
    .unwrap();
    for op in &part.operations {
        match &op.geometry {
            OpGeometry::Drill {
                u,
                v,
                diameter,
                depth,
                through,
                ..
            } => {
                let r = diameter / 2.0 * scale;
                let depth_txt = if *through {
                    "pasante".to_string()
                } else {
                    format!("{} prof.", n(depth.unwrap_or(0.0)))
                };
                match op.face {
                    Face::Front | Face::Back => {
                        let class = if op.face == Face::Front {
                            "hole"
                        } else {
                            "hole-back"
                        };
                        writeln!(
                            s,
                            "<circle class=\"{class}\" cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
                            n(x(*u)),
                            n(y(*v)),
                            n(r.max(1.5))
                        )
                        .unwrap();
                        holes_rows.push(format!(
                            "{} | {} | Ø{} {} | u {} v {}",
                            op.id,
                            if op.face == Face::Front {
                                "frente"
                            } else {
                                "dorso"
                            },
                            n(*diameter),
                            depth_txt,
                            n(*u),
                            n(*v)
                        ));
                    }
                    _ => {
                        let d = depth.unwrap_or(0.0) * scale;
                        let (x1, y1, x2, y2) = match op.face {
                            Face::Left => (x(0.0), y(*u), x(0.0) + d, y(*u)),
                            Face::Right => (x(len), y(*u), x(len) - d, y(*u)),
                            Face::Bottom => (x(*u), y(0.0), x(*u), y(0.0) - d),
                            _ => (x(*u), y(wid), x(*u), y(wid) + d),
                        };
                        writeln!(
                            s,
                            "<line class=\"edge-hole\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>",
                            n(x1),
                            n(y1),
                            n(x2),
                            n(y2)
                        )
                        .unwrap();
                        let face_name = match op.face {
                            Face::Left => "canto izq.",
                            Face::Right => "canto der.",
                            Face::Bottom => "canto inf.",
                            _ => "canto sup.",
                        };
                        holes_rows.push(format!(
                            "{} | {} | Ø{} {} | a {}",
                            op.id,
                            face_name,
                            n(*diameter),
                            depth_txt,
                            n(*u)
                        ));
                    }
                }
            }
            OpGeometry::Groove {
                from,
                to,
                width,
                depth,
            } => {
                let (u0, v0) = (from[0].min(to[0]), from[1].min(to[1]));
                let (u1, v1) = (from[0].max(to[0]), from[1].max(to[1]));
                let half = width / 2.0;
                let (gx, gy, gw, gh) = if (v1 - v0).abs() < (u1 - u0).abs() {
                    (u0, v0 - half, u1 - u0, *width)
                } else {
                    (u0 - half, v0, *width, v1 - v0)
                };
                writeln!(
                    s,
                    "<rect class=\"groove\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
                    n(x(gx)),
                    n(y(gy + gh)),
                    n(gw * scale),
                    n(gh * scale)
                )
                .unwrap();
                holes_rows.push(format!(
                    "{} | ranura {} | {}×{} | ({},{})–({},{})",
                    op.id,
                    if op.face == Face::Front {
                        "frente"
                    } else {
                        "dorso"
                    },
                    n(*width),
                    n(*depth),
                    n(from[0]),
                    n(from[1]),
                    n(to[0]),
                    n(to[1])
                ));
            }
            OpGeometry::EdgeBand { .. } => {
                let (x1, y1, x2, y2) = match op.face {
                    Face::Left => (x(0.0), y(0.0), x(0.0), y(wid)),
                    Face::Right => (x(len), y(0.0), x(len), y(wid)),
                    Face::Bottom => (x(0.0), y(0.0), x(len), y(0.0)),
                    _ => (x(0.0), y(wid), x(len), y(wid)),
                };
                writeln!(
                    s,
                    "<line class=\"band\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>",
                    n(x1),
                    n(y1),
                    n(x2),
                    n(y2)
                )
                .unwrap();
            }
        }
    }
    // Overall dimensions.
    let dy = oy + ph + 22.0;
    writeln!(
        s,
        "<line class=\"dim\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>",
        n(ox),
        n(dy),
        n(ox + pw),
        n(dy)
    )
    .unwrap();
    writeln!(
        s,
        "<text class=\"dimtext\" x=\"{}\" y=\"{}\">{}</text>",
        n(ox + pw / 2.0),
        n(dy - 3.0),
        n(len)
    )
    .unwrap();
    let dx = ox - 22.0;
    writeln!(
        s,
        "<line class=\"dim\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>",
        n(dx),
        n(oy),
        n(dx),
        n(oy + ph)
    )
    .unwrap();
    writeln!(
        s,
        "<text class=\"dimtext\" transform=\"translate({} {}) rotate(-90)\">{}</text>",
        n(dx - 4.0),
        n(oy + ph / 2.0),
        n(wid)
    )
    .unwrap();

    // Title block.
    let edges = [Face::Left, Face::Right, Face::Bottom, Face::Top]
        .iter()
        .map(|f| part.edges.get(f).map(String::as_str).unwrap_or("-"))
        .collect::<Vec<_>>()
        .join(" / ");
    writeln!(
        s,
        "<text class=\"title\" x=\"{}\" y=\"28\">{} — {}</text>",
        n(ox),
        esc(&part.id),
        esc(&part.name)
    )
    .unwrap();
    writeln!(
        s,
        "<text class=\"small\" x=\"{}\" y=\"44\">{} · {}×{}×{} mm terminada (corte {}×{}) · veta {} · cantos izq/der/inf/sup: {}</text>",
        n(ox),
        esc(&part.material),
        n(len),
        n(wid),
        n(part.dims.thickness),
        n(part.cut.length),
        n(part.cut.width),
        grain_es(part.grain),
        esc(&edges)
    )
    .unwrap();
    // Operations table, in columns under the drawing.
    let ty0 = oy + ph + 50.0;
    writeln!(
        s,
        "<text class=\"small\" x=\"{}\" y=\"{}\" font-weight=\"bold\">Operaciones (u, v desde la esquina inferior izquierda vista desde el frente)</text>",
        n(ox),
        n(ty0)
    )
    .unwrap();
    let per_column = holes_rows.len().div_ceil(COLUMNS).max(1);
    let column_w = (WIDTH - ox - 20.0) / COLUMNS as f64;
    for (i, row) in holes_rows.iter().enumerate() {
        let (col, line) = (i / per_column, i % per_column);
        writeln!(
            s,
            "<text class=\"row\" x=\"{}\" y=\"{}\">{}</text>",
            n(ox + col as f64 * column_w),
            n(ty0 + 6.0 + (line as f64 + 1.0) * ROW),
            esc(row)
        )
        .unwrap();
    }
    writeln!(s, "</g>").unwrap();
    let height = ty0 + 6.0 + (per_column as f64 + 1.0) * ROW + 10.0;
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">{STYLE}{s}</svg>\n",
        w = n(WIDTH),
        h = n(height)
    )
}

/// Front (X–Z), side (Y–Z) and top (X–Y) projections of every part's box,
/// fronts drawn last so doors and drawer fronts sit on top.
pub fn assembly_views_svg(plan: &ManufacturingPlan) -> String {
    // Legs reach below z = 0: extents are min..max, not 0..max.
    let (mut maxx, mut maxy, mut maxz) = (1.0_f64, 1.0_f64, 1.0_f64);
    let (mut minx, mut miny, mut minz) = (0.0_f64, 0.0_f64, 0.0_f64);
    for p in &plan.parts {
        maxx = maxx.max(p.aabb.max.0);
        maxy = maxy.max(p.aabb.max.1);
        maxz = maxz.max(p.aabb.max.2);
        minx = minx.min(p.aabb.min.0);
        miny = miny.min(p.aabb.min.1);
        minz = minz.min(p.aabb.min.2);
    }
    let lo = [minx, miny, minz];
    let span = [maxx - minx, maxy - miny, maxz - minz];
    let (maxx, maxy, maxz) = (span[0], span[1], span[2]);
    let scale = fit(maxx, 420.0).min(fit(maxz, 420.0)).min(fit(maxy, 420.0));
    let mut s = String::new();
    let rails = super::explode::rail_bars(plan, None);

    // view: (title, ox, oy, horizontal axis index, vertical axis index)
    let top = 56.0;
    let views = [
        ("Vista frontal (X–Z)", 30.0, top, 0usize, 2usize),
        ("Vista lateral (Y–Z)", 30.0 + maxx * scale + 60.0, top, 1, 2),
        (
            "Vista superior (X–Y)",
            30.0,
            top + maxz * scale + 60.0,
            0,
            1,
        ),
    ];
    let mut total_h = 0.0_f64;
    let mut total_w = 0.0_f64;
    // Fourth view: exploded isometric, right of the top view.
    let exploded = super::explode::exploded_body_at(
        plan,
        30.0 + maxx * scale + 60.0,
        top + maxz * scale + 60.0,
        (maxx.max(maxy) * scale * 1.4).max(240.0),
    );
    writeln!(
        s,
        "<text class=\"title\" x=\"{}\" y=\"{}\">Vista explotada</text>{}",
        n(30.0 + maxx * scale + 60.0),
        n(top + maxz * scale + 50.0),
        exploded.svg
    )
    .unwrap();
    total_w = total_w.max(exploded.width);
    total_h = total_h.max(exploded.height);
    for (title, ox, oy, h, v) in views {
        let vh = span[v] * scale;
        let vw = span[h] * scale;
        total_h = total_h.max(oy + vh + 30.0);
        total_w = total_w.max(ox + vw + 30.0);
        writeln!(
            s,
            "<text class=\"title\" x=\"{}\" y=\"{}\">{title}</text>",
            n(ox),
            n(oy - 10.0)
        )
        .unwrap();
        // Painter's order along the axis the view looks down: what is
        // nearer the viewer (front: +Y, side: +X, top: +Z) paints last.
        // Fronts, and the panel that closes the furniture on the viewer's
        // side (the top in the top view, a side in the side view), are
        // see-through: an opaque one would leave the view a blank rectangle.
        let depth = 3 - h - v;
        let near = lo[depth] + span[depth];
        let mut boxes: Vec<(&str, &str, &str, crate::geometry::Aabb)> = plan
            .parts
            .iter()
            .map(|p| {
                let closes = p.placement.z().vec().component(depth) != 0.0
                    && p.aabb.max.component(depth) >= near - 1.0;
                let class = match (is_front(p), closes) {
                    (true, _) => "part-front see-through",
                    (false, true) => "part see-through",
                    (false, false) => "part",
                };
                (p.id.as_str(), p.name.as_str(), class, p.aabb)
            })
            .collect();
        boxes.extend(rails.iter().map(|b| {
            (
                b.id.as_str(),
                b.name.as_str(),
                "part-hw",
                crate::geometry::Aabb {
                    min: b.lo,
                    max: b.hi,
                },
            )
        }));
        boxes.sort_by(|a, b| {
            let ka = a.3.min.component(depth) + a.3.max.component(depth);
            let kb = b.3.min.component(depth) + b.3.max.component(depth);
            ka.partial_cmp(&kb).unwrap().then(a.0.cmp(b.0))
        });
        for (id, name, class, aabb) in boxes {
            let (a0, a1) = (aabb.min.component(h) - lo[h], aabb.max.component(h) - lo[h]);
            let (b0, b1) = (aabb.min.component(v) - lo[v], aabb.max.component(v) - lo[v]);
            writeln!(
                s,
                "<rect class=\"{class}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"><title>{} {}</title></rect>",
                n(ox + a0 * scale),
                n(oy + vh - b1 * scale),
                n(((a1 - a0) * scale).max(0.5)),
                n(((b1 - b0) * scale).max(0.5)),
                esc(id),
                esc(name)
            )
            .unwrap();
            if (a1 - a0) * scale > 40.0 && (b1 - b0) * scale > 12.0 && !id.contains(':') {
                writeln!(
                    s,
                    "<text class=\"tiny\" x=\"{}\" y=\"{}\">{}</text>",
                    n(ox + a0 * scale + 3.0),
                    n(oy + vh - b0 * scale - 3.0),
                    esc(id)
                )
                .unwrap();
            }
        }
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">{STYLE}<text class=\"title\" x=\"30\" y=\"18\">{}</text>{s}</svg>\n",
        esc(&plan.furniture.name),
        w = n(total_w),
        h = n(total_h)
    )
}

/// Every sheet of the nesting, parts labelled, rotated ones hatched.
pub fn nesting_svg(plan: &ManufacturingPlan) -> String {
    let mut s = String::new();
    let mut y_cursor = 20.0;
    let mut max_w = 0.0_f64;
    for layout in &plan.nesting {
        let (w, next) = nesting_sheet(&mut s, layout, y_cursor);
        max_w = max_w.max(w);
        y_cursor = next;
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">{STYLE}{s}</svg>\n",
        w = n(max_w.max(200.0)),
        h = n(y_cursor.max(40.0))
    )
}

/// The same drawing, one SVG per sheet: the report prints each on its
/// own page.
pub fn nesting_sheet_svgs(plan: &ManufacturingPlan) -> Vec<String> {
    plan.nesting
        .iter()
        .map(|layout| {
            let mut s = String::new();
            let (w, h) = nesting_sheet(&mut s, layout, 20.0);
            format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">{STYLE}{s}</svg>
",
                w = n(w.max(200.0)),
                h = n(h)
            )
        })
        .collect()
}

/// Draw one sheet starting at `y_cursor`; returns (width used, next y).
fn nesting_sheet(
    s: &mut String,
    layout: &crate::nesting::SheetLayout,
    y_cursor: f64,
) -> (f64, f64) {
    let mut max_w = 0.0_f64;
    let scale = fit(layout.sheet_length, 700.0).min(fit(layout.sheet_width, 470.0));
    let (ox, oy) = (20.0, y_cursor + 18.0);
    let (sw, sh) = (layout.sheet_length * scale, layout.sheet_width * scale);
    max_w = max_w.max(ox + sw + 20.0);
    writeln!(
            s,
            "<text class=\"title\" x=\"{}\" y=\"{}\">{} — placa {} de {}×{} · aprovechamiento {}% · {} piezas</text>",
            n(ox),
            n(y_cursor + 8.0),
            esc(&layout.material),
            layout.index,
            n(layout.sheet_length),
            n(layout.sheet_width),
            n(((1.0 - layout.waste_ratio) * 1000.0).round() / 10.0),
            layout.parts.len()
        )
        .unwrap();
    writeln!(
        s,
        "<rect class=\"outline\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
        n(ox),
        n(oy),
        n(sw),
        n(sh)
    )
    .unwrap();
    for p in &layout.parts {
        // Sheet y grows upwards; SVG y grows downwards.
        let px = ox + p.x * scale;
        let py = oy + sh - (p.y + p.width) * scale;
        let class = if p.rotated { "part-front" } else { "part" };
        writeln!(
                s,
                "<rect class=\"{class}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"><title>{} {}×{}{}</title></rect>",
                n(px),
                n(py),
                n(p.length * scale),
                n(p.width * scale),
                esc(&p.part),
                n(p.length),
                n(p.width),
                if p.rotated { " (girada)" } else { "" }
            )
            .unwrap();
        if p.length * scale > 28.0 && p.width * scale > 10.0 {
            writeln!(
                s,
                "<text class=\"tiny\" x=\"{}\" y=\"{}\">{}</text>",
                n(px + 2.0),
                n(py + 8.0),
                esc(&p.part)
            )
            .unwrap();
        }
    }
    // Saw sequence (guillotine mode): numbered cut lines, one colour
    // per stage.
    for c in &layout.cuts {
        let (x0, y0) = (ox + c.x0 * scale, oy + sh - c.y0 * scale);
        let (x1, y1) = (ox + c.x1 * scale, oy + sh - c.y1 * scale);
        writeln!(
                s,
                "<line class=\"cut{}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"><title>corte {} (etapa {})</title></line>",
                c.stage,
                n(x0),
                n(y0),
                n(x1),
                n(y1),
                c.order,
                c.stage
            )
            .unwrap();
        if c.stage == 1 {
            writeln!(
                s,
                "<text class=\"tiny cutno\" x=\"{}\" y=\"{}\">{}</text>",
                n(x1 + 2.0),
                n(y1 + 2.5),
                c.order
            )
            .unwrap();
        }
    }
    if !layout.cuts.is_empty() {
        writeln!(
                s,
                "<text class=\"small\" x=\"{}\" y=\"{}\">Secuencia de corte (sierra): {} cortes · rojo = etapa 1 (tiras a lo largo), azul = etapa 2 (transversales), verde = etapa 3 (recortes)</text>",
                n(ox),
                n(oy + sh + 12.0),
                layout.cuts.len()
            )
            .unwrap();
    }
    let next = oy + sh + 24.0 + if layout.cuts.is_empty() { 0.0 } else { 14.0 };
    (max_w, next)
}

/// One label per part with a QR code carrying `rewood://<order>/<part>`.
pub fn labels_svg(plan: &ManufacturingPlan) -> String {
    let order = format!("{}-v{}", plan.furniture.id, plan.furniture.version);
    let (lw, lh, cols) = (260.0, 90.0, 3usize);
    let mut s = String::new();
    for (i, p) in plan.parts.iter().enumerate() {
        let ox = 20.0 + (i % cols) as f64 * (lw + 10.0);
        let oy = 20.0 + (i / cols) as f64 * (lh + 10.0);
        writeln!(
            s,
            "<rect class=\"label-box\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
            n(ox),
            n(oy),
            n(lw),
            n(lh)
        )
        .unwrap();
        let qr = qr_svg_path(
            &format!("rewood://{order}/{}", p.id),
            ox + 6.0,
            oy + 6.0,
            78.0,
        );
        s.push_str(&qr);
        let tx = ox + 92.0;
        writeln!(
            s,
            "<text class=\"title\" x=\"{}\" y=\"{}\">{}</text>",
            n(tx),
            n(oy + 20.0),
            esc(&p.id)
        )
        .unwrap();
        let lines = [
            p.name.clone(),
            format!(
                "{} · {}×{}×{}",
                p.material,
                n(p.dims.length),
                n(p.dims.width),
                n(p.dims.thickness)
            ),
            format!(
                "veta {} · {} operaciones",
                grain_es(p.grain),
                p.operations.len()
            ),
            format!("{} · {}", order, p.component),
        ];
        for (k, line) in lines.iter().enumerate() {
            writeln!(
                s,
                "<text class=\"small\" x=\"{}\" y=\"{}\">{}</text>",
                n(tx),
                n(oy + 36.0 + k as f64 * 13.0),
                esc(line)
            )
            .unwrap();
        }
    }
    let rows = plan.parts.len().div_ceil(cols);
    let h = 40.0 + rows as f64 * (lh + 10.0);
    let w = 40.0 + cols as f64 * (lw + 10.0);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">{STYLE}{s}</svg>\n",
        w = n(w),
        h = n(h)
    )
}

/// A QR code as a single SVG path of dark modules, fitted in `size` px.
fn qr_svg_path(content: &str, ox: f64, oy: f64, size: f64) -> String {
    let code = qrcode::QrCode::new(content.as_bytes()).expect("short ASCII content always encodes");
    let width = code.width();
    let module = size / width as f64;
    let mut d = String::new();
    for (i, colour) in code.to_colors().iter().enumerate() {
        if *colour == qrcode::Color::Dark {
            let (x, y) = ((i % width) as f64, (i / width) as f64);
            write!(
                d,
                "M{} {}h{}v{}h-{}z",
                n(ox + x * module),
                n(oy + y * module),
                n(module),
                n(module),
                n(module)
            )
            .unwrap();
        }
    }
    format!("<path fill=\"#000\" d=\"{d}\"/>\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawings_are_valid_svg_and_deterministic() {
        let spec = include_str!("../../../../fixtures/basic_cabinet/input.json");
        let plan = crate::compile_json(spec);
        let side = plan.parts.iter().find(|p| p.role == "side_left").unwrap();
        let svg = part_drawing_svg(side);
        assert!(svg.starts_with("<svg xmlns"));
        assert!(svg.trim_end().ends_with("</svg>"));
        assert_eq!(svg.matches("<circle").count(), 16);
        assert!(svg.contains("class=\"groove\""));
        assert_eq!(svg, part_drawing_svg(side));

        let views = assembly_views_svg(&plan);
        assert_eq!(
            views.matches("<rect class=\"part").count(),
            plan.parts.len() * 3
        );

        let labels = labels_svg(&plan);
        assert_eq!(
            labels.matches("<path fill=\"#000\"").count(),
            plan.parts.len()
        );
        assert!(labels.contains("P001"));
        assert_eq!(labels, labels_svg(&plan));
    }
}
