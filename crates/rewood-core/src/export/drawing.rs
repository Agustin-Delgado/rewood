//! Part drawings to the usual technical drawing norms (IRAM 4500 series,
//! which follow ISO): one or more A4 landscape sheets per part with a
//! frame and a title block, a standard scale, ISO 128 line types that
//! read in black and white, coordinate dimensioning from a 0 origin with
//! arrows (ISO 129), a view of each machined face, typical sections of
//! edge holes and grooves, and the hole tables. Paper units are mm.

use std::fmt::Write;

use crate::geometry::Face;
use crate::model::{OpGeometry, Part};

/// One printed sheet: its SVG and its paper size ("A4", "A3").
pub struct Sheet {
    pub svg: String,
    pub format: &'static str,
}

/// A table for a sheet: title, column widths (mm), header and rows.
pub type Table = (String, Vec<f64>, Vec<String>, Vec<Vec<String>>);

/// What the title block says and how the edges are named.
pub struct SheetInfo {
    pub number: usize,
    /// "P009, P010, P011".
    pub ids: String,
    pub name: String,
    pub quantity: usize,
    /// "Melamina 18 mm · Blanco Nature (Faplac 135NAT)".
    pub material: String,
    /// "Placard 1800 × 2100 … · wardrobe_1800-v1.0".
    pub furniture: String,
    /// "medidas ±0,2 · agujeros ±0,1 · Ø ±0,1".
    pub tolerances: String,
    /// L1, L2, A1, A2 and the face each one is.
    pub sides: [(&'static str, Face); 4],
    /// Edge band name per face, when banded ("PVC 22 × 0,45 mm").
    pub bands: Vec<(Face, String)>,
    /// The decor has a direction: `Some(true)` along the part's X.
    pub grain_along_x: Option<bool>,
    /// Parts that are this one's mirror image: same size, holes mirrored.
    pub mirror_of: Vec<String>,
}

/// A sheet size (ISO 5457), landscape: a 20 mm filing margin on the left
/// and 10 on the other sides, the title block in the lower right corner.
#[derive(Clone, Copy)]
struct Format {
    name: &'static str,
    w: f64,
    h: f64,
    /// Frame: left, top, right, bottom.
    frame: (f64, f64, f64, f64),
    /// Upper left corner of the title block.
    tb: (f64, f64),
    /// The drawing field above the title block.
    field: (f64, f64, f64, f64),
}

const A4: Format = Format {
    name: "A4",
    w: 297.0,
    h: 210.0,
    frame: (20.0, 10.0, 287.0, 200.0),
    tb: (117.0, 170.0),
    field: (24.0, 14.0, 283.0, 166.0),
};

const A3: Format = Format {
    name: "A3",
    w: 420.0,
    h: 297.0,
    frame: (20.0, 10.0, 410.0, 287.0),
    tb: (240.0, 257.0),
    field: (24.0, 14.0, 406.0, 253.0),
};

/// ISO 5455 reduction scales, largest first.
const SCALES: [f64; 7] = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0];

/// Millimetres with a decimal comma and tenths at most.
fn mm(v: f64) -> String {
    let r = (v * 10.0).round() / 10.0 + 0.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r:.1}").replace('.', ",")
    }
}

fn f(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0 + 0.0;
    format!("{r}")
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Cut a text to about what fits in `width` mm at `size`.
fn fit(s: &str, width: f64, size: f64) -> String {
    let max = (width / (size * 0.52)).floor() as usize;
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", cut.trim_end())
    }
}

/// A drawing being written, in paper millimetres.
#[derive(Default)]
struct Pen {
    s: String,
}

impl Pen {
    fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, class: &str) {
        let _ = write!(
            self.s,
            "<line class=\"{class}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"/>",
            f(x1),
            f(y1),
            f(x2),
            f(y2)
        );
    }
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, class: &str) {
        let _ = write!(
            self.s,
            "<rect class=\"{class}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",
            f(x),
            f(y),
            f(w),
            f(h)
        );
    }
    fn circle(&mut self, cx: f64, cy: f64, r: f64, class: &str) {
        let _ = write!(
            self.s,
            "<circle class=\"{class}\" cx=\"{}\" cy=\"{}\" r=\"{}\"/>",
            f(cx),
            f(cy),
            f(r)
        );
    }
    fn text(&mut self, x: f64, y: f64, size: f64, anchor: &str, t: &str) {
        let _ = write!(
            self.s,
            "<text x=\"{}\" y=\"{}\" font-size=\"{}\" text-anchor=\"{anchor}\">{}</text>",
            f(x),
            f(y),
            f(size),
            esc(t)
        );
    }
    fn bold(&mut self, x: f64, y: f64, size: f64, anchor: &str, t: &str) {
        let _ = write!(
            self.s,
            "<text x=\"{}\" y=\"{}\" font-size=\"{}\" font-weight=\"bold\" text-anchor=\"{anchor}\">{}</text>",
            f(x),
            f(y),
            f(size),
            esc(t)
        );
    }
    /// Text read from the right: rotated a quarter turn anticlockwise.
    fn vtext(&mut self, x: f64, y: f64, size: f64, anchor: &str, t: &str) {
        let _ = write!(
            self.s,
            "<text transform=\"translate({} {}) rotate(-90)\" font-size=\"{}\" text-anchor=\"{anchor}\">{}</text>",
            f(x),
            f(y),
            f(size),
            esc(t)
        );
    }
    /// A filled arrowhead at (x, y) pointing along (dx, dy).
    fn arrow(&mut self, x: f64, y: f64, dx: f64, dy: f64) {
        let (l, w) = (2.5, 0.9);
        let (bx, by) = (x - dx * l, y - dy * l);
        let (px, py) = (-dy * w, dx * w);
        let _ = write!(
            self.s,
            "<path class=\"ar\" d=\"M{} {} L{} {} L{} {} Z\"/>",
            f(x),
            f(y),
            f(bx + px),
            f(by + py),
            f(bx - px),
            f(by - py)
        );
    }
    /// A dimension between two points on a horizontal line, arrows
    /// inside, the value above the line.
    fn hdim(&mut self, x1: f64, x2: f64, y: f64, label: &str) {
        self.line(x1, y, x2, y, "dm");
        self.arrow(x1, y, -1.0, 0.0);
        self.arrow(x2, y, 1.0, 0.0);
        self.text((x1 + x2) / 2.0, y - 1.0, 3.0, "middle", label);
    }
    fn vdim(&mut self, x: f64, y1: f64, y2: f64, label: &str) {
        self.line(x, y1, x, y2, "dm");
        self.arrow(x, y1, 0.0, -1.0);
        self.arrow(x, y2, 0.0, 1.0);
        self.vtext(x - 1.0, (y1 + y2) / 2.0, 3.0, "middle", label);
    }
}

/// Label positions for sorted `targets`, at least `gap` apart and each as
/// close to its target as the others let it be.
fn spread(targets: &[f64], gap: f64) -> Vec<f64> {
    let mut p: Vec<f64> = targets.to_vec();
    for i in 1..p.len() {
        p[i] = p[i].max(p[i - 1] + gap);
    }
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

fn distinct(mut v: Vec<f64>) -> Vec<f64> {
    v.iter_mut().for_each(|a| *a = (*a * 10.0).round() / 10.0);
    v.sort_by(f64::total_cmp);
    v.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    v
}

/// One face of the part as drawn: what is on it, in that face's own
/// frame (x along the part's length, y up, origin lower left).
struct FaceView {
    title: String,
    holes: Vec<(f64, f64, f64)>,
    /// Edge holes as (edge face in this view, position along it, Ø, depth).
    edge_holes: Vec<(Face, f64, f64, f64)>,
    grooves: Vec<([f64; 2], [f64; 2], f64)>,
    cutouts: Vec<(f64, f64, f64, f64, f64)>,
    /// Which part edge lies at the bottom, top, left and right.
    edges: [(Face, Face); 4],
}

/// Space a view takes at `scale` besides the part itself: its title
/// above, ordinate labels and the overall dimension below and left.
const VIEW_TOP: f64 = 7.0;
const VIEW_BELOW: f64 = 24.0;
const VIEW_LEFT: f64 = 24.0;
const VIEW_RIGHT: f64 = 6.0;

fn views(part: &Part) -> Vec<FaceView> {
    let w = part.dims.width;
    let mut a = FaceView {
        title: "Cara A".into(),
        holes: Vec::new(),
        edge_holes: Vec::new(),
        grooves: Vec::new(),
        cutouts: Vec::new(),
        edges: [
            (Face::Bottom, Face::Bottom),
            (Face::Top, Face::Top),
            (Face::Left, Face::Left),
            (Face::Right, Face::Right),
        ],
    };
    // Face B turned over its X axis: the bottom edge goes to the top.
    let mut b = FaceView {
        title: "Cara B (pieza dada vuelta)".into(),
        holes: Vec::new(),
        edge_holes: Vec::new(),
        grooves: Vec::new(),
        cutouts: Vec::new(),
        edges: [
            (Face::Bottom, Face::Top),
            (Face::Top, Face::Bottom),
            (Face::Left, Face::Left),
            (Face::Right, Face::Right),
        ],
    };
    for op in &part.operations {
        let flip = |v: f64| w - v;
        match (&op.geometry, op.face) {
            (OpGeometry::Drill { u, v, diameter, .. }, Face::Front) => {
                a.holes.push((*u, *v, *diameter))
            }
            (OpGeometry::Drill { u, v, diameter, .. }, Face::Back) => {
                b.holes.push((*u, flip(*v), *diameter))
            }
            (
                OpGeometry::Drill {
                    u, diameter, depth, ..
                },
                edge,
            ) => a
                .edge_holes
                .push((edge, *u, *diameter, depth.unwrap_or(0.0))),
            (
                OpGeometry::Groove {
                    from, to, width, ..
                },
                Face::Front,
            ) => a.grooves.push((*from, *to, *width)),
            (
                OpGeometry::Groove {
                    from, to, width, ..
                },
                Face::Back,
            ) => b
                .grooves
                .push(([from[0], flip(from[1])], [to[0], flip(to[1])], *width)),
            (
                OpGeometry::Cutout {
                    u,
                    v,
                    width,
                    height,
                    radius,
                },
                Face::Back,
            ) => b.cutouts.push((*u, flip(*v), *width, *height, *radius)),
            (
                OpGeometry::Cutout {
                    u,
                    v,
                    width,
                    height,
                    radius,
                },
                _,
            ) => a.cutouts.push((*u, *v, *width, *height, *radius)),
            _ => {}
        }
    }
    let mut out = vec![a];
    if !b.holes.is_empty() || !b.grooves.is_empty() || !b.cutouts.is_empty() {
        out.push(b);
    }
    out
}

/// The largest standard scale at which the views fit the field, stacked.
fn scale_for(part: &Part, n: usize, room_w: f64, room_h: f64) -> f64 {
    let (l, w) = (part.dims.length, part.dims.width);
    for k in SCALES {
        let vw = VIEW_LEFT + l / k + VIEW_RIGHT;
        let vh = n as f64 * (VIEW_TOP + w / k + VIEW_BELOW) + (n as f64 - 1.0) * 3.0;
        if vw <= room_w && vh <= room_h {
            return k;
        }
    }
    *SCALES.last().unwrap()
}

fn side_name(info: &SheetInfo, face: Face) -> &'static str {
    info.sides
        .iter()
        .find(|(_, f)| *f == face)
        .map_or("?", |(n, _)| *n)
}

/// Draw one face view with its part corner at (x0, y0) (upper left of the
/// part outline on paper). Returns nothing; everything goes to `pen`.
fn draw_view(pen: &mut Pen, part: &Part, info: &SheetInfo, v: &FaceView, k: f64, x0: f64, y0: f64) {
    let (l, w) = (part.dims.length, part.dims.width);
    let (pw, ph) = (l / k, w / k);
    let px = |u: f64| x0 + u / k;
    let py = |vv: f64| y0 + ph - vv / k;

    let grain = match info.grain_along_x {
        Some(true) => " · veta ⟷",
        Some(false) => " · veta ↕",
        None => "",
    };
    pen.bold(
        x0,
        y0 - 2.5,
        3.2,
        "start",
        &format!("{} · escala 1:{}{grain}", v.title, k),
    );
    pen.rect(x0, y0, pw, ph, "o");

    // Edges: a thick line where there is band, the edge's name inside.
    for (at, face) in v.edges {
        let banded = info.bands.iter().find(|(bf, _)| *bf == face);
        let (x1, y1, x2, y2) = match at {
            Face::Bottom => (x0, y0 + ph, x0 + pw, y0 + ph),
            Face::Top => (x0, y0, x0 + pw, y0),
            Face::Left => (x0, y0, x0, y0 + ph),
            _ => (x0 + pw, y0, x0 + pw, y0 + ph),
        };
        if banded.is_some() {
            pen.line(x1, y1, x2, y2, "band");
        }
        let label = format!(
            "{}{}",
            side_name(info, face),
            if banded.is_some() { " ▬ canto" } else { "" }
        );
        match at {
            Face::Bottom if ph > 6.0 => {
                pen.text(x0 + pw / 2.0, y0 + ph - 1.2, 2.2, "middle", &label)
            }
            Face::Top => {
                let title_w = (v.title.chars().count() + 16) as f64 * 3.2 * 0.55;
                if pw - title_w > label.chars().count() as f64 * 1.3 {
                    pen.text(x0 + pw, y0 - 1.2, 2.2, "end", &label)
                } else if ph > 6.0 {
                    pen.text(x0 + pw - 1.5, y0 + 3.0, 2.2, "end", &label)
                }
            }
            Face::Left if pw > 12.0 && ph > 12.0 => {
                pen.vtext(x0 + 3.0, y0 + ph / 2.0, 2.2, "middle", &label)
            }
            Face::Right if pw > 12.0 && ph > 12.0 => {
                pen.vtext(x0 + pw - 1.2, y0 + ph / 2.0, 2.2, "middle", &label)
            }
            _ => {}
        }
    }

    let mut xs = vec![0.0];
    let mut ys = vec![0.0];
    for &(u, vv, d) in &v.holes {
        let r = (d / 2.0 / k).max(0.3);
        pen.circle(px(u), py(vv), r, "o");
        if d / k >= 1.5 {
            let e = r + 1.2;
            pen.line(px(u) - e, py(vv), px(u) + e, py(vv), "ax");
            pen.line(px(u), py(vv) - e, px(u), py(vv) + e, "ax");
        }
        xs.push(u);
        ys.push(vv);
    }
    for &(edge, at, d, depth) in &v.edge_holes {
        // A bore inside the panel: two hidden lines as deep as the hole.
        let (hd, hl) = ((d / 2.0 / k).max(0.2), depth / k);
        match edge {
            Face::Left | Face::Right => {
                let (xa, dir) = if edge == Face::Left {
                    (x0, 1.0)
                } else {
                    (x0 + pw, -1.0)
                };
                let y = py(at);
                pen.line(xa, y - hd, xa + dir * hl, y - hd, "hid");
                pen.line(xa, y + hd, xa + dir * hl, y + hd, "hid");
                pen.line(xa + dir * hl, y - hd, xa + dir * hl, y + hd, "hid");
                ys.push(at);
            }
            _ => {
                let (ya, dir) = if edge == Face::Bottom {
                    (y0 + ph, -1.0)
                } else {
                    (y0, 1.0)
                };
                let x = px(at);
                pen.line(x - hd, ya, x - hd, ya + dir * hl, "hid");
                pen.line(x + hd, ya, x + hd, ya + dir * hl, "hid");
                pen.line(x - hd, ya + dir * hl, x + hd, ya + dir * hl, "hid");
                xs.push(at);
            }
        }
    }
    for &(from, to, width) in &v.grooves {
        let hw = width / 2.0;
        if (to[1] - from[1]).abs() < (to[0] - from[0]).abs() {
            for off in [-hw, hw] {
                pen.line(
                    px(from[0]),
                    py(from[1] + off),
                    px(to[0]),
                    py(to[1] + off),
                    "o2",
                );
            }
            ys.push(from[1]);
        } else {
            for off in [-hw, hw] {
                pen.line(
                    px(from[0] + off),
                    py(from[1]),
                    px(to[0] + off),
                    py(to[1]),
                    "o2",
                );
            }
            xs.push(from[0]);
        }
        for u in [from[0], to[0]] {
            if u > 0.05 && u < l - 0.05 {
                xs.push(u);
            }
        }
    }
    for &(u, vv, cw, chh, r) in &v.cutouts {
        let _ = write!(
            pen.s,
            "<rect class=\"o\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"/>",
            f(px(u - cw / 2.0)),
            f(py(vv + chh / 2.0)),
            f(cw / k),
            f(chh / k),
            f(r / k)
        );
        xs.push(u);
        ys.push(vv);
    }

    // Coordinate dimensions (ISO 129): a short extension line from the
    // outline for every coordinate, the value at its end, moved aside on a
    // jog where values crowd. 0 is the origin.
    let xs = distinct(xs);
    let at: Vec<f64> = xs.iter().map(|u| px(*u)).collect();
    let yb = y0 + ph;
    for ((u, x), lx) in xs.iter().zip(&at).zip(spread(&at, 2.6)) {
        let _ = write!(
            pen.s,
            "<polyline class=\"dm\" points=\"{},{} {},{} {},{} {},{}\"/>",
            f(*x),
            f(yb + 1.0),
            f(*x),
            f(yb + 3.0),
            f(lx),
            f(yb + 5.0),
            f(lx),
            f(yb + 6.0)
        );
        pen.vtext(lx + 0.9, yb + 6.8, 2.5, "end", &mm(*u));
    }
    let ys = distinct(ys);
    let at: Vec<f64> = ys.iter().map(|vv| -py(*vv)).collect();
    for ((vv, y), ly) in ys.iter().zip(&at).zip(spread(&at, 2.9)) {
        let (y, ly) = (-y, -ly);
        let _ = write!(
            pen.s,
            "<polyline class=\"dm\" points=\"{},{} {},{} {},{} {},{}\"/>",
            f(x0 - 1.0),
            f(y),
            f(x0 - 3.0),
            f(y),
            f(x0 - 5.0),
            f(ly),
            f(x0 - 6.0),
            f(ly)
        );
        pen.text(x0 - 6.8, ly + 0.9, 2.5, "end", &mm(*vv));
    }
    // The origin, and the overall size with arrows and extension lines.
    pen.circle(x0, yb, 0.8, "org");
    let dy = yb + 20.5;
    pen.line(x0, yb + 1.0, x0, dy + 1.5, "dm");
    pen.line(x0 + pw, yb + 1.0, x0 + pw, dy + 1.5, "dm");
    pen.hdim(x0, x0 + pw, dy, &mm(l));
    let dx = x0 - 20.0;
    pen.line(x0 - 1.0, y0, dx - 1.5, y0, "dm");
    pen.line(x0 - 1.0, yb, dx - 1.5, yb, "dm");
    pen.vdim(dx, y0, yb, &mm(w));
}

type Draw = Box<dyn Fn(&mut Pen, f64, f64)>;

/// A block placed in the free space of a sheet: a table or a detail.
struct Block {
    w: f64,
    h: f64,
    svg: Draw,
}

/// A table of text with a title, columns of the given widths.
fn table(
    title: String,
    widths: Vec<f64>,
    header: Vec<String>,
    rows: Vec<Vec<String>>,
) -> Vec<Block> {
    const ROW: f64 = 3.6;
    // Long tables go in pieces of at most 36 rows, the header repeated.
    let chunks: Vec<Vec<Vec<String>>> = rows.chunks(36).map(|c| c.to_vec()).collect();
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, rows)| {
            let title = if i == 0 {
                title.clone()
            } else {
                format!("{title} (sigue)")
            };
            let widths = widths.clone();
            let header = header.clone();
            let w: f64 = widths.iter().sum();
            let bw = w.max(title.chars().count() as f64 * 1.55);
            let h = 5.0 + ROW * (rows.len() + 1) as f64;
            Block {
                w: bw,
                h,
                svg: Box::new(move |pen: &mut Pen, x: f64, y: f64| {
                    pen.bold(x, y + 3.0, 2.8, "start", &title);
                    let top = y + 4.5;
                    let all: Vec<&Vec<String>> =
                        std::iter::once(&header).chain(rows.iter()).collect();
                    pen.rect(x, top, w, ROW * all.len() as f64, "tb");
                    for (r, cells) in all.iter().enumerate() {
                        let yy = top + ROW * r as f64;
                        if r > 0 {
                            pen.line(x, yy, x + w, yy, "tl");
                        }
                        let mut cx = x;
                        for (c, cell) in cells.iter().enumerate() {
                            if c > 0 {
                                pen.line(cx, top, cx, top + ROW * all.len() as f64, "tl");
                            }
                            let t = fit(cell, widths[c] - 1.2, 2.4);
                            if r == 0 {
                                pen.bold(cx + 0.8, yy + 2.7, 2.3, "start", &t);
                            } else {
                                pen.text(cx + 0.8, yy + 2.7, 2.4, "start", &t);
                            }
                            cx += widths[c];
                        }
                    }
                }),
            }
        })
        .collect()
}

/// How to put the part on the machine so it cannot come out mirrored,
/// and what to count when it is done.
fn placing(part: &Part, info: &SheetInfo, face_b: bool) -> Block {
    let banded = |f: Face| info.bands.iter().any(|(b, _)| *b == f);
    let name = |f: Face| side_name(info, f);
    let names: Vec<&str> = info
        .sides
        .iter()
        .filter(|(_, f)| banded(*f))
        .map(|(n, _)| *n)
        .collect();
    let symmetric = info
        .sides
        .chunks(2)
        .all(|pair| banded(pair[0].1) == banded(pair[1].1));
    let mut lines: Vec<String> = Vec::new();
    if names.is_empty() {
        lines.push(
            "Sin tapacanto: cualquier cara puede ser la A y cualquier canto largo el L1.".into(),
        );
    } else if symmetric {
        lines.push(format!(
            "Tapacanto en {}: cantos iguales enfrentados, cualquier cara puede ser la A.",
            names.join(", ")
        ));
    } else {
        lines.push(format!(
            "Cara A arriba y el tapacanto ({}) donde lo marca el dibujo: {} abajo, {} a la izquierda.",
            names.join(", "),
            name(Face::Bottom),
            name(Face::Left)
        ));
    }
    if face_b {
        lines.push(format!(
            "Primero la cara A. Después dar vuelta la pieza de arriba hacia abajo ({} pasa arriba) y perforar la cara B según su vista.",
            name(Face::Bottom)
        ));
    }
    let (mut a, mut b, mut e, mut g, mut c) = (0, 0, 0, 0, 0);
    for op in &part.operations {
        match (&op.geometry, op.face) {
            (OpGeometry::Drill { .. }, Face::Front) => a += 1,
            (OpGeometry::Drill { .. }, Face::Back) => b += 1,
            (OpGeometry::Drill { .. }, _) => e += 1,
            (OpGeometry::Groove { .. }, _) => g += 1,
            (OpGeometry::Cutout { .. }, _) => c += 1,
            _ => {}
        }
    }
    let mut count = format!("Control: {} perforaciones (cara A {a}", a + b + e);
    if b > 0 {
        count += &format!(" · cara B {b}");
    }
    if e > 0 {
        count += &format!(" · canto {e}");
    }
    count += ")";
    if g > 0 {
        count += &format!(", {g} ranura{}", if g > 1 { "s" } else { "" });
    }
    if c > 0 {
        count += &format!(", {c} calado{}", if c > 1 { "s" } else { "" });
    }
    count += " por pieza.";
    lines.push(count);
    if !info.mirror_of.is_empty() {
        lines.push(format!(
            "ATENCIÓN: es la simétrica (espejo) de {}. No son intercambiables: cada una con su plano.",
            info.mirror_of.join(", ")
        ));
    }
    const W: f64 = 150.0;
    const SIZE: f64 = 2.5;
    // Wrap each line to the block's width.
    let per_line = (W / (SIZE * 0.5)).floor() as usize;
    let mut wrapped: Vec<(bool, String)> = Vec::new();
    for l in &lines {
        let strong = l.starts_with("ATENCIÓN");
        let mut cur = String::new();
        for word in l.split(' ') {
            if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > per_line {
                wrapped.push((strong, std::mem::take(&mut cur)));
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur += word;
        }
        wrapped.push((strong, cur));
    }
    let h = 6.0 + wrapped.len() as f64 * 3.6;
    Block {
        w: W,
        h,
        svg: Box::new(move |pen: &mut Pen, x: f64, y: f64| {
            pen.bold(x, y + 3.0, 2.8, "start", "Cómo ubicar la pieza");
            for (i, (strong, l)) in wrapped.iter().enumerate() {
                let yy = y + 7.0 + i as f64 * 3.6;
                if *strong {
                    pen.bold(x, yy, SIZE, "start", l);
                } else {
                    pen.text(x, yy, SIZE, "start", l);
                }
            }
        }),
    }
}

/// Section through an edge hole, full size: the panel's thickness, the
/// bore centred in it, its diameter and depth.
fn edge_hole_detail(tag: String, letter: char, t: f64, d: f64, depth: f64) -> Block {
    let k = if depth + 14.0 <= 52.0 { 1.0 } else { 2.0 };
    let (lw, lh) = ((depth + 12.0) / k, t / k);
    Block {
        w: (lw + 24.0).max(62.0),
        h: lh + 15.0,
        svg: Box::new(move |pen: &mut Pen, x: f64, y: f64| {
            pen.bold(
                x,
                y + 3.0,
                2.8,
                "start",
                &format!("Detalle {letter} — perforación de canto (esc. 1:{k})"),
            );
            let (x0, y0) = (x + 12.0, y + 6.5);
            let clip = format!("{tag}{letter}");
            hatch(pen, &clip, x0, y0, lw, lh);
            pen.rect(x0, y0, lw, lh, "o");
            // The edge is on the left; the bore goes in to the right.
            let (hd, hl) = (d / 2.0 / k, depth / k);
            let cy = y0 + lh / 2.0;
            pen.rect(x0, cy - hd, hl, 2.0 * hd, "void");
            pen.line(x0 - 2.0, cy, x0 + hl + 2.0, cy, "ax");
            // Depth below, diameter on the right, half the thickness left.
            let yb = y0 + lh + 5.0;
            pen.line(x0, y0 + lh + 0.8, x0, yb + 1.5, "dm");
            pen.line(x0 + hl, cy + hd + 0.8, x0 + hl, yb + 1.5, "dm");
            pen.hdim(x0, x0 + hl, yb, &mm(depth));
            let xr = x0 + hl + 5.0;
            pen.line(x0 + hl + 0.8, cy - hd, xr + 1.5, cy - hd, "dm");
            pen.line(x0 + hl + 0.8, cy + hd, xr + 1.5, cy + hd, "dm");
            pen.line(xr, cy - hd - 3.0, xr, cy + hd + 3.0, "dm");
            pen.arrow(xr, cy - hd, 0.0, 1.0);
            pen.arrow(xr, cy + hd, 0.0, -1.0);
            pen.text(xr + 1.2, cy + 1.0, 2.5, "start", &format!("Ø{}", mm(d)));
            let xl = x0 - 5.0;
            pen.line(x0 - 0.8, y0 + lh, xl - 1.5, y0 + lh, "dm");
            pen.line(x0 - 0.8, cy, xl - 1.5, cy, "dm");
            pen.vdim(xl, cy, y0 + lh, &mm(t / 2.0));
        }),
    }
}

/// Section across a groove that runs along an edge: the panel thickness,
/// the groove's width, depth and distance from the edge, full size.
/// `groove` is its distance from the edge (centre), width and depth.
fn groove_detail(
    tag: String,
    letter: char,
    t: f64,
    groove: [f64; 3],
    face_a: bool,
    edge: &str,
) -> Block {
    let [offset, width, depth] = groove;
    let k = if offset + width + 12.0 <= 60.0 {
        1.0
    } else {
        2.0
    };
    let (lw, lh) = ((offset + width / 2.0 + 12.0) / k, t / k);
    let edge = edge.to_string();
    let title = format!(
        "Detalle {letter} — ranura cara {} (esc. 1:{k})",
        if face_a { "A" } else { "B" }
    );
    let sub = format!("medida desde el canto {edge}");
    Block {
        w: (lw + 24.0).max(title.chars().count() as f64 * 1.55),
        h: lh + 18.0,
        svg: Box::new(move |pen: &mut Pen, x: f64, y: f64| {
            pen.bold(x, y + 3.0, 2.8, "start", &title);
            pen.text(x, y + 6.3, 2.3, "start", &sub);
            let (x0, y0) = (x + 12.0, y + 9.5);
            let clip = format!("{tag}{letter}");
            hatch(pen, &clip, x0, y0, lw, lh);
            pen.rect(x0, y0, lw, lh, "o");
            // The edge is on the left, face A on top.
            let (g0, gw, gd) = ((offset - width / 2.0) / k, width / k, depth / k);
            let gy = if face_a { y0 } else { y0 + lh - gd };
            pen.rect(x0 + g0, gy, gw, gd, "void");
            // Distance from the edge and width below, depth on the right.
            let yb = y0 + lh + 5.0;
            pen.line(x0, y0 + lh + 0.8, x0, yb + 1.5, "dm");
            pen.line(x0 + g0, gy + gd + 0.8, x0 + g0, yb + 1.5, "dm");
            pen.line(x0 + g0 + gw, gy + gd + 0.8, x0 + g0 + gw, yb + 1.5, "dm");
            pen.hdim(x0, x0 + g0, yb, &mm(offset - width / 2.0));
            pen.text(
                x0 + g0 + gw + 1.0,
                yb + 1.0,
                2.5,
                "start",
                &format!("← {} →", mm(width)),
            );
            let xr = x0 + g0 + gw + 5.0;
            pen.line(x0 + g0 + gw + 0.8, gy, xr + 1.5, gy, "dm");
            pen.line(x0 + g0 + gw + 0.8, gy + gd, xr + 1.5, gy + gd, "dm");
            pen.vdim(xr, gy, gy + gd, &mm(depth));
        }),
    }
}

/// 45° section hatching (ISO 128-50) clipped to a rectangle.
fn hatch(pen: &mut Pen, id: &str, x: f64, y: f64, w: f64, h: f64) {
    let _ = write!(
        pen.s,
        "<clipPath id=\"{id}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath><g clip-path=\"url(#{id})\">",
        f(x),
        f(y),
        f(w),
        f(h)
    );
    let mut s = -h;
    while s < w {
        pen.line(x + s, y + h, x + s + h, y, "ht");
        s += 1.8;
    }
    pen.s += "</g>";
}

fn title_block(pen: &mut Pen, fmt: &Format, info: &SheetInfo, k: f64, sheet: usize, sheets: usize) {
    let (x, y) = fmt.tb;
    let w = fmt.frame.2 - x;
    pen.rect(x, y, w, fmt.frame.3 - y, "fr");
    for r in 1..3 {
        pen.line(
            x,
            y + 10.0 * r as f64,
            fmt.frame.2,
            y + 10.0 * r as f64,
            "tl",
        );
    }
    let cell = |pen: &mut Pen, cx: f64, cy: f64, cw: f64, label: &str, value: &str, size: f64| {
        pen.text(cx + 1.0, cy + 2.6, 1.9, "start", label);
        pen.bold(
            cx + 1.0,
            cy + 8.0,
            size,
            "start",
            &fit(value, cw - 2.0, size),
        );
    };
    let v = |pen: &mut Pen, cx: f64, row: usize| {
        pen.line(
            cx,
            y + 10.0 * row as f64,
            cx,
            y + 10.0 * (row + 1) as f64,
            "tl",
        );
    };
    // Row 1: part, quantity, drawing, sheet.
    cell(
        pen,
        x,
        y,
        110.0,
        "Pieza",
        &format!("{} — {}", info.ids, info.name),
        3.4,
    );
    v(pen, x + 110.0, 0);
    cell(
        pen,
        x + 110.0,
        y,
        20.0,
        "Cantidad",
        &info.quantity.to_string(),
        3.4,
    );
    v(pen, x + 130.0, 0);
    cell(
        pen,
        x + 130.0,
        y,
        20.0,
        "Plano Nº",
        &info.number.to_string(),
        3.4,
    );
    v(pen, x + 150.0, 0);
    cell(
        pen,
        x + 150.0,
        y,
        20.0,
        "Hoja",
        &format!("{sheet} / {sheets}"),
        3.4,
    );
    // Row 2: material, scale, units.
    cell(
        pen,
        x,
        y + 10.0,
        130.0,
        "Material y color",
        &info.material,
        3.0,
    );
    v(pen, x + 130.0, 1);
    cell(
        pen,
        x + 130.0,
        y + 10.0,
        20.0,
        "Escala",
        &format!("1:{k}"),
        3.4,
    );
    v(pen, x + 150.0, 1);
    cell(pen, x + 150.0, y + 10.0, 20.0, "Unidades", "mm", 3.4);
    // Row 3: furniture, tolerances.
    cell(pen, x, y + 20.0, 80.0, "Mueble", &info.furniture, 2.6);
    v(pen, x + 80.0, 2);
    cell(
        pen,
        x + 80.0,
        y + 20.0,
        90.0,
        "Tolerancias generales",
        &info.tolerances,
        2.6,
    );
}

/// The notes left of the title block: conventions every sheet repeats.
fn notes(pen: &mut Pen, fmt: &Format) {
    let (x, y) = (fmt.frame.0 + 3.0, fmt.tb.1 + 3.6);
    let lines = [
        "NO MEDIR SOBRE EL PLANO: usar sólo las cotas.",
        "Medidas finales en mm, con el canto incluido.",
        "Cotas desde el origen 0: esquina inferior izquierda de cada vista.",
        "Cara A arriba al perforar; la etiqueta de la pieza va en la cara A.",
        "Líneas (ISO 128): llena visible · trazos oculta · trazo y punto eje.",
        "▬ canto con tapacanto. Perforaciones de canto centradas en el espesor.",
    ];
    for (i, l) in lines.iter().enumerate() {
        let t = fit(l, fmt.tb.0 - x - 2.0, 2.0);
        if i == 0 {
            pen.bold(x, y + i as f64 * 4.3, 2.2, "start", &t);
        } else {
            pen.text(x, y + i as f64 * 4.3, 2.0, "start", &t);
        }
    }
}

fn sheet_svg(
    pen: Pen,
    fmt: &Format,
    info: &SheetInfo,
    k: f64,
    sheet: usize,
    sheets: usize,
) -> String {
    let mut frame = Pen::default();
    let (x0, y0, x1, y1) = fmt.frame;
    frame.rect(x0, y0, x1 - x0, y1 - y0, "fr");
    title_block(&mut frame, fmt, info, k, sheet, sheets);
    notes(&mut frame, fmt);
    format!(
        "<svg class=\"iso\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" font-family=\"Helvetica,Arial,sans-serif\">{}{}</svg>",
        fmt.w, fmt.h, frame.s, pen.s
    )
}

/// The sheets of one part, each a full A4 landscape page.
pub fn part_sheets(part: &Part, info: &SheetInfo, tables: Vec<Table>) -> Vec<Sheet> {
    let vs = views(part);
    // A4 unless the part would go smaller than 1:10 there and A3 draws it
    // larger: a long panel on A4 at 1:20 leaves its holes as dots.
    let fits = |f: &Format| {
        let (x0, y0, x1, y1) = f.field;
        scale_for(part, vs.len(), x1 - x0, y1 - y0)
    };
    let (fmt, k) = {
        let k4 = fits(&A4);
        let k3 = fits(&A3);
        if k4 > 10.0 && k3 < k4 {
            (A3, k3)
        } else {
            (A4, k4)
        }
    };
    let field = fmt.field;
    let (l, w) = (part.dims.length, part.dims.width);

    let mut pen = Pen::default();
    let title_w = vs
        .iter()
        .map(|v| (v.title.chars().count() + 24) as f64 * 3.2 * 0.56)
        .fold(0.0, f64::max);
    let view_w = (VIEW_LEFT + l / k + VIEW_RIGHT).max(VIEW_LEFT + title_w);
    let mut y = field.1;
    for v in &vs {
        draw_view(
            &mut pen,
            part,
            info,
            v,
            k,
            field.0 + VIEW_LEFT,
            y + VIEW_TOP,
        );
        y += VIEW_TOP + w / k + VIEW_BELOW + 3.0;
    }
    let views_bottom = y;

    // Details: one section per kind of edge hole and per groove along an
    // edge, lettered.
    let mut blocks: Vec<Block> = Vec::new();
    let mut letter = b'A';
    let mut kinds: Vec<(f64, f64)> = Vec::new();
    for op in &part.operations {
        if let (
            OpGeometry::Drill {
                diameter, depth, ..
            },
            Face::Left | Face::Right | Face::Bottom | Face::Top,
        ) = (&op.geometry, op.face)
        {
            let key = (*diameter, depth.unwrap_or(0.0));
            if !kinds.contains(&key) {
                kinds.push(key);
                blocks.push(edge_hole_detail(
                    format!("h{}", info.number),
                    letter as char,
                    part.dims.thickness,
                    key.0,
                    key.1,
                ));
                letter += 1;
            }
        }
    }
    for op in &part.operations {
        if let OpGeometry::Groove {
            from,
            to,
            width,
            depth,
        } = &op.geometry
        {
            if !matches!(op.face, Face::Front | Face::Back) {
                continue;
            }
            // Along the length: distance to the nearer long edge.
            let (offset, edge) = if (to[1] - from[1]).abs() < (to[0] - from[0]).abs() {
                let v = from[1];
                if v <= w - v {
                    (v, Face::Bottom)
                } else {
                    (w - v, Face::Top)
                }
            } else {
                let u = from[0];
                if u <= l - u {
                    (u, Face::Left)
                } else {
                    (l - u, Face::Right)
                }
            };
            blocks.push(groove_detail(
                format!("g{}", info.number),
                letter as char,
                part.dims.thickness,
                [offset, *width, *depth],
                op.face == Face::Front,
                side_name(info, edge),
            ));
            letter += 1;
        }
    }
    let details = std::mem::take(&mut blocks);
    blocks.push(placing(part, info, vs.len() > 1));
    for (title, widths, header, rows) in tables {
        blocks.extend(table(title, widths, header, rows));
    }
    blocks.extend(details);

    // Lay the blocks out in rows, left to right, in the first free space
    // that takes them: right of the views, under them, then new sheets.
    /// A free rectangle of the sheet filled bottom-left first: a block
    /// goes at the highest, then leftmost, corner next to or under what
    /// is already placed where it overlaps nothing.
    struct Region {
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        placed: Vec<(f64, f64, f64, f64)>,
    }
    impl Region {
        fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Region {
            Region {
                x0,
                y0,
                x1,
                y1,
                placed: Vec::new(),
            }
        }
        fn take(&mut self, w: f64, h: f64) -> Option<(f64, f64)> {
            const GAP_X: f64 = 7.0;
            const GAP_Y: f64 = 4.0;
            let mut spots = vec![(self.x0, self.y0)];
            for &(x, y, pw, ph) in &self.placed {
                spots.push((x + pw + GAP_X, y));
                spots.push((x, y + ph + GAP_Y));
            }
            let free = |&(x, y): &(f64, f64)| {
                x + w <= self.x1 + 1e-9
                    && y + h <= self.y1 + 1e-9
                    && self.placed.iter().all(|&(px, py, pw, ph)| {
                        x >= px + pw + GAP_X - 1e-9
                            || x + w + GAP_X <= px + 1e-9
                            || y >= py + ph + GAP_Y - 1e-9
                            || y + h + GAP_Y <= py + 1e-9
                    })
            };
            let at = spots
                .into_iter()
                .filter(free)
                .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0)))?;
            self.placed.push((at.0, at.1, w, h));
            Some(at)
        }
    }
    let mut sheets: Vec<Pen> = Vec::new();
    let mut regions: Vec<Region> = Vec::new();
    let right = field.0 + view_w + 8.0;
    let has_right = field.2 - right >= 45.0;
    if has_right {
        regions.push(Region::new(right, field.1, field.2, field.3));
    }
    if field.3 - views_bottom >= 20.0 {
        // Under the views only: the column on the right is its own region.
        let x1 = if has_right { right - 4.0 } else { field.2 };
        regions.push(Region::new(field.0, views_bottom + 2.0, x1, field.3));
    }
    for b in blocks {
        let spot = regions.iter_mut().find_map(|r| r.take(b.w, b.h));
        let (x, y) = match spot {
            Some(at) => at,
            None => {
                // A new sheet: the whole field is free. What does not fit
                // an empty sheet is drawn anyway.
                sheets.push(std::mem::take(&mut pen));
                regions = vec![Region::new(field.0, field.1, field.2, field.3)];
                regions[0].take(b.w, b.h).unwrap_or_else(|| {
                    regions[0].placed.push((field.0, field.1, b.w, b.h));
                    (field.0, field.1)
                })
            }
        };
        (b.svg)(&mut pen, x, y);
    }
    sheets.push(pen);
    let n = sheets.len();
    sheets
        .into_iter()
        .enumerate()
        .map(|(i, p)| Sheet {
            svg: sheet_svg(p, &fmt, info, k, i + 1, n),
            format: fmt.name,
        })
        .collect()
}

/// Line styles (ISO 128: continuous wide 0.5, narrow 0.25/0.18, dashed
/// for hidden, long dash dot for axes), black only so they print right.
pub const CSS: &str =
    "svg.iso line,svg.iso rect,svg.iso circle,svg.iso polyline,svg.iso path{fill:none;stroke:#000}\
svg.iso text{fill:#000}\
svg.iso .fr{stroke-width:.5}svg.iso .o{stroke-width:.5}svg.iso .o2{stroke-width:.35}\
svg.iso .band{stroke-width:1.4}svg.iso .hid{stroke-width:.25;stroke-dasharray:1.2 .6}\
svg.iso .ax{stroke-width:.18;stroke-dasharray:4 .8 .6 .8}svg.iso .dm{stroke-width:.18}\
svg.iso .ar{fill:#000;stroke:none}svg.iso .org{stroke-width:.25;fill:#fff}\
svg.iso .tb{stroke-width:.35}svg.iso .tl{stroke-width:.18}svg.iso .ht{stroke-width:.13}\
svg.iso .void{fill:#fff;stroke-width:.35}";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_are_standard_and_the_largest_that_fits() {
        assert!(SCALES.windows(2).all(|w| w[0] < w[1]));
        let fits = |l: f64, w: f64, n: usize| {
            let k = SCALES
                .iter()
                .copied()
                .find(|k| {
                    VIEW_LEFT + l / k + VIEW_RIGHT <= 259.0
                        && n as f64 * (VIEW_TOP + w / k + VIEW_BELOW) + (n as f64 - 1.0) * 3.0
                            <= 152.0
                })
                .unwrap();
            k
        };
        assert_eq!(fits(2100.0, 500.0, 1), 10.0);
        assert_eq!(fits(576.0, 466.8, 1), 5.0);
        assert_eq!(fits(450.0, 224.3, 2), 10.0);
    }

    #[test]
    fn spread_keeps_labels_apart_and_close_to_their_targets() {
        let p = spread(&[10.0, 10.5, 11.0, 50.0], 2.0);
        assert!(p.windows(2).all(|w| w[1] - w[0] >= 2.0 - 1e-9));
        assert!((p[3] - 50.0).abs() < 1e-9);
        assert!((p[1] - 10.5).abs() < 1.5);
    }
}
