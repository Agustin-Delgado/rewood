//! One DXF (R12 ASCII) per part: the finished outline seen from the
//! part's `front` face, every hole as a circle on a layer that names its
//! face, diameter and depth, edge holes as short lines from the edge
//! inward, grooves as their footprint. Enough for a CAM operator to import
//! without interpreting anything.
//!
//! 2D frame: X = part length (u), Y = part width (v), origin at the panel's
//! bottom-left corner as seen from `front`. A hole on `back` is drawn at
//! the same (u, v) it has on `front` — that is the engine's convention —
//! and its layer says which side it is drilled from.

use std::fmt::Write;

use crate::geometry::Face;
use crate::model::{OpGeometry, Part};
use crate::units::round3;

fn num(v: f64) -> String {
    let r = round3(v);
    if r.fract() == 0.0 {
        format!("{:.1}", r)
    } else {
        format!("{}", r)
    }
}

fn depth_tag(depth: Option<f64>) -> String {
    match depth {
        Some(d) => format!("L{}", num(d).trim_end_matches(".0")),
        None => "THRU".to_string(),
    }
}

fn face_tag(face: Face) -> &'static str {
    match face {
        Face::Front => "FRONT",
        Face::Back => "BACK",
        Face::Left => "EDGE_LEFT",
        Face::Right => "EDGE_RIGHT",
        Face::Bottom => "EDGE_BOTTOM",
        Face::Top => "EDGE_TOP",
    }
}

struct Dxf {
    layers: Vec<String>,
    entities: String,
}

impl Dxf {
    fn new() -> Dxf {
        Dxf {
            layers: Vec::new(),
            entities: String::new(),
        }
    }

    fn layer(&mut self, name: &str) {
        if !self.layers.iter().any(|l| l == name) {
            self.layers.push(name.to_string());
        }
    }

    fn line(&mut self, layer: &str, x1: f64, y1: f64, x2: f64, y2: f64) {
        self.layer(layer);
        write!(
            self.entities,
            "0\nLINE\n8\n{layer}\n10\n{}\n20\n{}\n30\n0.0\n11\n{}\n21\n{}\n31\n0.0\n",
            num(x1),
            num(y1),
            num(x2),
            num(y2)
        )
        .unwrap();
    }

    fn rect(&mut self, layer: &str, x: f64, y: f64, w: f64, h: f64) {
        self.line(layer, x, y, x + w, y);
        self.line(layer, x + w, y, x + w, y + h);
        self.line(layer, x + w, y + h, x, y + h);
        self.line(layer, x, y + h, x, y);
    }

    fn circle(&mut self, layer: &str, x: f64, y: f64, r: f64) {
        self.layer(layer);
        write!(
            self.entities,
            "0\nCIRCLE\n8\n{layer}\n10\n{}\n20\n{}\n30\n0.0\n40\n{}\n",
            num(x),
            num(y),
            num(r)
        )
        .unwrap();
    }

    fn text(&mut self, layer: &str, x: f64, y: f64, height: f64, text: &str) {
        self.layer(layer);
        write!(
            self.entities,
            "0\nTEXT\n8\n{layer}\n10\n{}\n20\n{}\n30\n0.0\n40\n{}\n1\n{text}\n",
            num(x),
            num(y),
            num(height)
        )
        .unwrap();
    }

    fn finish(self) -> String {
        let mut out = String::new();
        out.push_str("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n0\nENDSEC\n");
        out.push_str("0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n70\n");
        out.push_str(&format!("{}\n", self.layers.len()));
        for (i, layer) in self.layers.iter().enumerate() {
            // Colour index cycles through the ACI palette so layers are
            // told apart at a glance; 7 (white/black) for the outline.
            let colour = if i == 0 { 7 } else { 1 + (i % 6) };
            out.push_str(&format!(
                "0\nLAYER\n2\n{layer}\n70\n0\n62\n{colour}\n6\nCONTINUOUS\n"
            ));
        }
        out.push_str("0\nENDTAB\n0\nENDSEC\n");
        out.push_str("0\nSECTION\n2\nENTITIES\n");
        out.push_str(&self.entities);
        out.push_str("0\nENDSEC\n0\nEOF\n");
        out
    }
}

pub fn part_dxf(part: &Part) -> String {
    let mut dxf = Dxf::new();
    let (len, wid) = (part.dims.length, part.dims.width);
    dxf.rect("OUTLINE", 0.0, 0.0, len, wid);

    for op in &part.operations {
        match &op.geometry {
            OpGeometry::Drill {
                u,
                v,
                diameter,
                depth,
                ..
            } => {
                let layer = format!(
                    "DRILL_{}_D{}_{}",
                    face_tag(op.face),
                    num(*diameter).trim_end_matches(".0"),
                    depth_tag(*depth)
                );
                match op.face {
                    Face::Front | Face::Back => dxf.circle(&layer, *u, *v, diameter / 2.0),
                    Face::Left | Face::Right | Face::Bottom | Face::Top => {
                        // Horizontal drilling: a line from the edge inward,
                        // as long as the hole is deep. `u` runs along the
                        // edge; `v` (across the thickness) is not drawable
                        // in 2D and is always the thickness centre here.
                        let d = depth.unwrap_or(0.0);
                        let (x1, y1, x2, y2) = match op.face {
                            Face::Left => (0.0, *u, d, *u),
                            Face::Right => (len, *u, len - d, *u),
                            Face::Bottom => (*u, 0.0, *u, d),
                            _ => (*u, wid, *u, wid - d),
                        };
                        dxf.line(&layer, x1, y1, x2, y2);
                    }
                }
            }
            OpGeometry::Groove {
                from,
                to,
                width,
                depth,
            } => {
                let layer = format!(
                    "GROOVE_{}_W{}_L{}",
                    face_tag(op.face),
                    num(*width).trim_end_matches(".0"),
                    num(*depth).trim_end_matches(".0")
                );
                let (x0, y0) = (from[0].min(to[0]), from[1].min(to[1]));
                let (x1, y1) = (from[0].max(to[0]), from[1].max(to[1]));
                let half = width / 2.0;
                if (y1 - y0).abs() < (x1 - x0).abs() {
                    dxf.rect(&layer, x0, y0 - half, x1 - x0, *width);
                } else {
                    dxf.rect(&layer, x0 - half, y0, *width, y1 - y0);
                }
            }
            OpGeometry::EdgeBand { thickness, .. } => {
                let layer = format!("EDGE_BAND_{}", num(*thickness).trim_end_matches(".0"));
                let (x1, y1, x2, y2) = match op.face {
                    Face::Left => (0.0, 0.0, 0.0, wid),
                    Face::Right => (len, 0.0, len, wid),
                    Face::Bottom => (0.0, 0.0, len, 0.0),
                    _ => (0.0, wid, len, wid),
                };
                dxf.line(&layer, x1, y1, x2, y2);
            }
        }
    }

    let label = format!(
        "{} {} {}x{}x{}",
        part.id,
        part.name,
        num(len).trim_end_matches(".0"),
        num(wid).trim_end_matches(".0"),
        num(part.dims.thickness).trim_end_matches(".0")
    );
    let height = (wid / 12.0).clamp(5.0, 30.0);
    dxf.text(
        "LABEL",
        len / 2.0 - label.len() as f64 * height * 0.3,
        wid / 2.0,
        height,
        &label,
    );
    dxf.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_cabinet_side_dxf_has_outline_holes_and_groove() {
        let spec = include_str!("../../../../fixtures/basic_cabinet/input.json");
        let plan = crate::compile_json(spec);
        let side = plan.parts.iter().find(|p| p.role == "side_left").unwrap();
        let dxf = part_dxf(side);
        assert!(dxf.starts_with("0\nSECTION\n2\nHEADER"));
        assert!(dxf.ends_with("0\nEOF\n"));
        assert!(dxf.contains("\nOUTLINE\n"));
        assert!(dxf.contains("DRILL_FRONT_D8_L13"));
        assert!(dxf.contains("DRILL_FRONT_D5_L11.5"));
        assert!(dxf.contains("GROOVE_FRONT_W3.2_L8"));
        assert!(dxf.contains("EDGE_BAND_1"));
        assert_eq!(dxf.matches("\nCIRCLE\n").count(), 16);
        // Deterministic: same part, same bytes.
        assert_eq!(dxf, part_dxf(side));
    }

    #[test]
    fn top_dxf_draws_edge_holes_as_lines_from_the_edge() {
        let spec = include_str!("../../../../fixtures/basic_cabinet/input.json");
        let plan = crate::compile_json(spec);
        let top = plan.parts.iter().find(|p| p.role == "top").unwrap();
        let dxf = part_dxf(top);
        assert!(dxf.contains("DRILL_EDGE_LEFT_D8_L34"));
        assert!(dxf.contains("DRILL_EDGE_RIGHT_D8_L18"));
        // Bolt hole at u=350 on the left edge: line from (0,350) to (34,350).
        assert!(dxf.contains("10\n0.0\n20\n350.0\n30\n0.0\n11\n34.0\n21\n350.0\n"));
    }
}
