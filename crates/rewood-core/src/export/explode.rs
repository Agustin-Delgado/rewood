//! Exploded view (§29): every part is pushed away from the furniture along
//! its own thickness axis, fronts and drawers a little further forward, so
//! the assembly reads as "what goes where". A display rule, not geometry
//! the plan owns; the UI applies the same rule in `geometry.ts`, keep them
//! in step.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::geometry::Vec3;
use crate::model::Part;
use crate::plan::ManufacturingPlan;
use crate::units::round3;

/// Explosion offset of every part, world mm, for a `factor` of 1 = a
/// modest spread (fronts 1.5× a panel's travel). `factor` scales it.
pub fn offsets(plan: &ManufacturingPlan, factor: f64) -> BTreeMap<String, Vec3> {
    let (mut lo, mut hi) = (
        Vec3(f64::MAX, f64::MAX, f64::MAX),
        Vec3(f64::MIN, f64::MIN, f64::MIN),
    );
    for p in &plan.parts {
        lo = Vec3(
            lo.0.min(p.aabb.min.0),
            lo.1.min(p.aabb.min.1),
            lo.2.min(p.aabb.min.2),
        );
        hi = Vec3(
            hi.0.max(p.aabb.max.0),
            hi.1.max(p.aabb.max.1),
            hi.2.max(p.aabb.max.2),
        );
    }
    let centre = Vec3(
        (lo.0 + hi.0) / 2.0,
        (lo.1 + hi.1) / 2.0,
        (lo.2 + hi.2) / 2.0,
    );
    // A panel travels a fixed distance, not proportional to the furniture:
    // a wardrobe and a nightstand explode the same way.
    let step = 120.0 * factor;
    let mut out = BTreeMap::new();
    for p in &plan.parts {
        out.insert(p.id.clone(), offset_of(p, centre, step));
    }
    out
}

fn offset_of(p: &Part, centre: Vec3, step: f64) -> Vec3 {
    let c = Vec3(
        (p.aabb.min.0 + p.aabb.max.0) / 2.0,
        (p.aabb.min.1 + p.aabb.max.1) / 2.0,
        (p.aabb.min.2 + p.aabb.max.2) / 2.0,
    );
    // Along the part's thickness axis, away from the centre; a part on the
    // centre plane (a middle shelf) goes up.
    let normal = p.placement.z().vec();
    let d = (c.0 - centre.0) * normal.0 + (c.1 - centre.1) * normal.1 + (c.2 - centre.2) * normal.2;
    let sign = if d.abs() < 1e-6 { 1.0 } else { d.signum() };
    let mut off = Vec3(
        normal.0 * sign * step,
        normal.1 * sign * step,
        normal.2 * sign * step,
    );
    // Fronts and drawers come out of the cabinet first.
    let role = p.role.as_str();
    if role.contains("door") {
        off.1 += 1.5 * step;
    } else if role.contains("drawer") {
        off.1 += if role.ends_with("_front") && !role.contains("box_front") {
            2.0 * step
        } else {
            1.2 * step
        };
    }
    Vec3(round3(off.0), round3(off.1), round3(off.2))
}

/// Isometric projection: X to the right and down, Y (depth, towards the
/// viewer) to the left and down, Z up. Visible faces: top, front, right.
fn project(v: Vec3) -> (f64, f64) {
    ((v.0 - v.1) * 0.866, (v.0 + v.1) * 0.5 - v.2)
}

/// The exploded view as a standalone SVG.
pub fn exploded_svg(plan: &ManufacturingPlan) -> String {
    let body = exploded_body(plan);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">{style}<text class=\"title\" x=\"30\" y=\"18\">Vista explotada — {}</text>{}</svg>\n",
        esc(&plan.furniture.name),
        body.svg,
        style = super::svg::STYLE,
        w = n(body.width.max(30.0 + 8.0 * (plan.furniture.name.chars().count() as f64 + 18.0))),
        h = n(body.height)
    )
}

pub struct ExplodedBody {
    pub svg: String,
    pub width: f64,
    pub height: f64,
}

/// Boxes at their exploded positions, projected and painted far to near,
/// origin at (ox, oy) = top-left of the drawing; `max_px` bounds it.
pub fn exploded_body(plan: &ManufacturingPlan) -> ExplodedBody {
    exploded_body_at(plan, 30.0, 40.0, 560.0)
}

pub fn exploded_body_at(plan: &ManufacturingPlan, ox: f64, oy: f64, max_px: f64) -> ExplodedBody {
    let offs = offsets(plan, 1.0);
    struct Box3 {
        id: String,
        name: String,
        lo: Vec3,
        hi: Vec3,
        front: bool,
    }
    let mut boxes: Vec<Box3> = plan
        .parts
        .iter()
        .map(|p| {
            let o = offs[&p.id];
            Box3 {
                id: p.id.clone(),
                name: p.name.clone(),
                lo: Vec3(p.aabb.min.0 + o.0, p.aabb.min.1 + o.1, p.aabb.min.2 + o.2),
                hi: Vec3(p.aabb.max.0 + o.0, p.aabb.max.1 + o.1, p.aabb.max.2 + o.2),
                front: super::svg::is_front(p),
            }
        })
        .collect();
    // Painter's order: the viewer sits at +X +Y +Z, so the smallest
    // (x + y + z) is the farthest and goes first.
    boxes.sort_by(|a, b| {
        let ka = a.lo.0 + a.lo.1 + a.lo.2;
        let kb = b.lo.0 + b.lo.1 + b.lo.2;
        ka.partial_cmp(&kb).unwrap().then(a.id.cmp(&b.id))
    });

    // Projected extent, then scale to fit.
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for b in &boxes {
        for v in corners(b.lo, b.hi) {
            let (x, y) = project(v);
            minx = minx.min(x);
            miny = miny.min(y);
            maxx = maxx.max(x);
            maxy = maxy.max(y);
        }
    }
    let span = (maxx - minx).max(maxy - miny).max(1.0);
    let scale = max_px / span;
    let sx = |x: f64| ox + (x - minx) * scale;
    let sy = |y: f64| oy + (y - miny) * scale;
    let mut s = String::new();
    for b in &boxes {
        let (l, h) = (b.lo, b.hi);
        let faces = [
            // top
            (
                [
                    Vec3(l.0, l.1, h.2),
                    Vec3(h.0, l.1, h.2),
                    Vec3(h.0, h.1, h.2),
                    Vec3(l.0, h.1, h.2),
                ],
                "iso-top",
            ),
            // front (y = max, towards the viewer)
            (
                [
                    Vec3(l.0, h.1, l.2),
                    Vec3(h.0, h.1, l.2),
                    Vec3(h.0, h.1, h.2),
                    Vec3(l.0, h.1, h.2),
                ],
                "iso-front",
            ),
            // right (x = max)
            (
                [
                    Vec3(h.0, l.1, l.2),
                    Vec3(h.0, h.1, l.2),
                    Vec3(h.0, h.1, h.2),
                    Vec3(h.0, l.1, h.2),
                ],
                "iso-right",
            ),
        ];
        for (pts, class) in faces {
            let path: Vec<String> = pts
                .iter()
                .map(|v| {
                    let (x, y) = project(*v);
                    format!("{},{}", n(sx(x)), n(sy(y)))
                })
                .collect();
            writeln!(
                s,
                "<polygon class=\"{class}{}\" points=\"{}\"><title>{} {}</title></polygon>",
                if b.front { " iso-tinted" } else { "" },
                path.join(" "),
                esc(&b.id),
                esc(&b.name)
            )
            .unwrap();
        }
        // Label on the largest projected face.
        let (cx, cy) = project(Vec3((l.0 + h.0) / 2.0, h.1, (l.2 + h.2) / 2.0));
        let (tx, ty) = project(Vec3((l.0 + h.0) / 2.0, (l.1 + h.1) / 2.0, h.2));
        let front_area = (h.0 - l.0) * (h.2 - l.2);
        let top_area = (h.0 - l.0) * (h.1 - l.1);
        let (lx, ly) = if front_area >= top_area {
            (cx, cy)
        } else {
            (tx, ty)
        };
        if front_area.max(top_area) * scale * scale > 400.0 {
            writeln!(
                s,
                "<text class=\"tiny\" text-anchor=\"middle\" x=\"{}\" y=\"{}\">{}</text>",
                n(sx(lx)),
                n(sy(ly) + 2.5),
                esc(&b.id)
            )
            .unwrap();
        }
    }
    ExplodedBody {
        svg: s,
        width: ox + (maxx - minx) * scale + 30.0,
        height: oy + (maxy - miny) * scale + 30.0,
    }
}

fn corners(l: Vec3, h: Vec3) -> [Vec3; 8] {
    [
        Vec3(l.0, l.1, l.2),
        Vec3(h.0, l.1, l.2),
        Vec3(l.0, h.1, l.2),
        Vec3(h.0, h.1, l.2),
        Vec3(l.0, l.1, h.2),
        Vec3(h.0, l.1, h.2),
        Vec3(l.0, h.1, h.2),
        Vec3(h.0, h.1, h.2),
    ]
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_move_away_from_the_centre_and_fronts_come_forward() {
        let plan = crate::compile_json(include_str!(
            "../../../../fixtures/basic_cabinet/input.json"
        ));
        let offs = offsets(&plan, 1.0);
        let by_role = |role: &str| {
            let p = plan.parts.iter().find(|p| p.role == role).unwrap();
            offs[&p.id]
        };
        assert_eq!(by_role("side_left"), Vec3(-120.0, 0.0, 0.0));
        assert_eq!(by_role("side_right"), Vec3(120.0, 0.0, 0.0));
        assert_eq!(by_role("top"), Vec3(0.0, 0.0, 120.0));
        assert_eq!(by_role("bottom"), Vec3(0.0, 0.0, -120.0));
        assert_eq!(by_role("back"), Vec3(0.0, -120.0, 0.0));
        let door = plan.parts.iter().find(|p| p.role.contains("door")).unwrap();
        assert!(offs[&door.id].1 > 120.0);
        let svg = exploded_svg(&plan);
        assert!(svg.contains("iso-front"));
        assert_eq!(svg, exploded_svg(&plan));
    }
}
