//! Exploded view (§29): every part is pushed away from the furniture along
//! its own thickness axis, fronts and drawers a little further forward,
//! and the modules of a run apart from each other, so the assembly reads
//! as "what goes where". A display rule, not geometry
//! the plan owns; the UI applies the same offsets in `geometry.ts`, keep
//! them in step. The printed view goes one step further and takes the
//! doors out to the sides of the cabinet: a door drawn in front of it, open
//! or closed, hides what the drawing is there to show. The UI does not
//! (the camera turns instead).

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::geometry::Vec3;
use crate::model::Part;
use crate::plan::ManufacturingPlan;
use crate::units::round3;

/// Explosion offset of every part, world mm, for a `factor` of 1 = a
/// modest spread (fronts 1.5× a panel's travel). `factor` scales it.
pub fn offsets(plan: &ManufacturingPlan, factor: f64) -> BTreeMap<String, Vec3> {
    // A panel travels a fixed distance, not proportional to the furniture:
    // a wardrobe and a nightstand explode the same way.
    let step = 120.0 * factor;
    let modules = modules(&plan.parts);
    let mut out = BTreeMap::new();
    for p in &plan.parts {
        let m = module_of(&modules, p);
        let mut off = offset_of(p, modules[m].centre, step);
        off.0 += modules[m].spread * 2.0 * step;
        out.insert(
            p.id.clone(),
            Vec3(round3(off.0), round3(off.1), round3(off.2)),
        );
    }
    out
}

/// A carcass and what hangs in it. Parts move away from their own
/// module's centre, not the furniture's: in a run of three modules the
/// sides between two modules would otherwise both move inwards, into the
/// shelves of the bay they bound.
struct Module {
    centre: Vec3,
    /// Whole modules drift apart along X, two steps each: -1, 0, +1 for three.
    spread: f64,
}

fn modules(parts: &[Part]) -> Vec<Module> {
    let mut by_carcass: BTreeMap<&str, (Vec3, Vec3)> = BTreeMap::new();
    for p in parts {
        if !matches!(
            p.role.as_str(),
            "side_left" | "side_right" | "top" | "bottom"
        ) {
            continue;
        }
        let e = by_carcass.entry(p.component.as_str()).or_insert((
            Vec3(f64::MAX, f64::MAX, f64::MAX),
            Vec3(f64::MIN, f64::MIN, f64::MIN),
        ));
        e.0 = Vec3(
            e.0 .0.min(p.aabb.min.0),
            e.0 .1.min(p.aabb.min.1),
            e.0 .2.min(p.aabb.min.2),
        );
        e.1 = Vec3(
            e.1 .0.max(p.aabb.max.0),
            e.1 .1.max(p.aabb.max.1),
            e.1 .2.max(p.aabb.max.2),
        );
    }
    let boxes: Vec<(Vec3, Vec3)> = if by_carcass.is_empty() {
        let mut lo = Vec3(f64::MAX, f64::MAX, f64::MAX);
        let mut hi = Vec3(f64::MIN, f64::MIN, f64::MIN);
        for p in parts {
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
        vec![(lo, hi)]
    } else {
        by_carcass.into_values().collect()
    };
    let mut modules: Vec<Module> = boxes
        .iter()
        .map(|(lo, hi)| Module {
            centre: Vec3(
                (lo.0 + hi.0) / 2.0,
                (lo.1 + hi.1) / 2.0,
                (lo.2 + hi.2) / 2.0,
            ),
            spread: 0.0,
        })
        .collect();
    modules.sort_by(|a, b| a.centre.0.partial_cmp(&b.centre.0).unwrap());
    let n = modules.len() as f64;
    for (i, m) in modules.iter_mut().enumerate() {
        m.spread = i as f64 - (n - 1.0) / 2.0;
    }
    modules
}

/// The module a part belongs to: the one whose centre is nearest along X
/// (a door overlaying its carcass, a drawer inside it).
fn module_of(modules: &[Module], p: &Part) -> usize {
    let cx = (p.aabb.min.0 + p.aabb.max.0) / 2.0;
    let mut best = 0;
    for (i, m) in modules.iter().enumerate() {
        if (m.centre.0 - cx).abs() < (modules[best].centre.0 - cx).abs() {
            best = i;
        }
    }
    best
}

fn offset_of(p: &Part, centre: Vec3, step: f64) -> Vec3 {
    let c = Vec3(
        (p.aabb.min.0 + p.aabb.max.0) / 2.0,
        (p.aabb.min.1 + p.aabb.max.1) / 2.0,
        (p.aabb.min.2 + p.aabb.max.2) / 2.0,
    );
    // Along the part's thickness axis, away from its module's centre. A
    // part on the centre plane (a middle shelf, the divider of a two-bay
    // carcass) stays: pushed either way it would run into the parts of the
    // bay beside it.
    let normal = p.placement.z().vec();
    let d = (c.0 - centre.0) * normal.0 + (c.1 - centre.1) * normal.1 + (c.2 - centre.2) * normal.2;
    let sign = if d.abs() < 1e-6 { 0.0 } else { d.signum() };
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
    off
}

/// Isometric projection: X to the right and down, Y (depth, towards the
/// viewer) to the left and down, Z up. Visible faces: top, front, right.
fn project(v: Vec3) -> (f64, f64) {
    ((v.0 - v.1) * 0.866, (v.0 + v.1) * 0.5 - v.2)
}

/// A box drawn like a part but owned by no part: the rail bar.
pub(super) struct Bar {
    pub id: String,
    pub name: String,
    pub lo: Vec3,
    pub hi: Vec3,
}

/// Rail bar cross-section (oval rail, mm): tall × deep.
const RAIL_SECTION: (f64, f64) = (30.0, 15.0);

/// Rails have no panel: a bar between the two supports of each bay, at
/// the height the supports are screwed at. With `offs`, each end follows
/// the panel its support is on, so the bar stays hung in the exploded
/// view.
pub(super) fn rail_bars(
    plan: &ManufacturingPlan,
    offs: Option<&BTreeMap<String, Vec3>>,
) -> Vec<Bar> {
    let mut by_component: BTreeMap<&str, Vec<&crate::model::Joint>> = BTreeMap::new();
    for j in &plan.joints {
        if j.kind == "fixture" && j.hardware.iter().any(|h| h.starts_with("rail_support")) {
            by_component
                .entry(j.component.as_str())
                .or_default()
                .push(j);
        }
    }
    let zero = Vec3(0.0, 0.0, 0.0);
    let mut out = Vec::new();
    for (component, joints) in by_component {
        for (i, pair) in joints.chunks(2).enumerate() {
            let [a, b] = pair else { continue };
            let (Some(pa), Some(pb)) = (a.fasteners.first(), b.fasteners.first()) else {
                continue;
            };
            let oa = offs
                .and_then(|o| o.get(&a.face_part))
                .copied()
                .unwrap_or(zero);
            let ob = offs
                .and_then(|o| o.get(&b.face_part))
                .copied()
                .unwrap_or(zero);
            let y = (pa.position.1 + oa.1 + pb.position.1 + ob.1) / 2.0;
            let z = (pa.position.2 + oa.2 + pb.position.2 + ob.2) / 2.0;
            let (x0, x1) = (pa.position.0 + oa.0, pb.position.0 + ob.0);
            out.push(Bar {
                id: format!("{component}:{i}"),
                name: "Barral".into(),
                lo: Vec3(
                    x0.min(x1),
                    y - RAIL_SECTION.1 / 2.0,
                    z - RAIL_SECTION.0 / 2.0,
                ),
                hi: Vec3(
                    x0.max(x1),
                    y + RAIL_SECTION.1 / 2.0,
                    z + RAIL_SECTION.0 / 2.0,
                ),
            });
        }
    }
    out
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
        tint: &'static str,
    }
    let mut boxes: Vec<Box3> = plan
        .parts
        .iter()
        .map(|p| {
            let o = offs[&p.id];
            let lo = Vec3(p.aabb.min.0 + o.0, p.aabb.min.1 + o.1, p.aabb.min.2 + o.2);
            let hi = Vec3(p.aabb.max.0 + o.0, p.aabb.max.1 + o.1, p.aabb.max.2 + o.2);
            Box3 {
                id: p.id.clone(),
                name: p.name.clone(),
                lo,
                hi,
                tint: if super::svg::is_front(p) {
                    " iso-tinted"
                } else {
                    ""
                },
            }
        })
        .collect();
    // Doors go out to the side they are on, beyond everything else, the
    // one nearest the centre first and the rest stacked outwards. They lose
    // their forward push: beside the cabinet, a door pushed forward would
    // slide back over it in the projection. On the right that projection
    // also needs the cabinet's depth cleared, since depth draws leftwards.
    let step = 120.0;
    let door_of = |b: &Box3| {
        plan.parts
            .iter()
            .find(|p| p.id == b.id && p.role.contains("door"))
    };
    let (mut ext_lo, mut ext_hi, mut ext_lo_y) = (f64::MAX, f64::MIN, f64::MAX);
    for b in boxes.iter().filter(|b| door_of(b).is_none()) {
        ext_lo = ext_lo.min(b.lo.0);
        ext_hi = ext_hi.max(b.hi.0);
        ext_lo_y = ext_lo_y.min(b.lo.1);
    }
    let mid = (ext_lo + ext_hi) / 2.0;
    let mut order: Vec<usize> = (0..boxes.len())
        .filter(|&i| door_of(&boxes[i]).is_some())
        .collect();
    // Nearest the centre first, on either side.
    order.sort_by(|&a, &b| {
        let da = ((boxes[a].lo.0 + boxes[a].hi.0) / 2.0 - mid).abs();
        let db = ((boxes[b].lo.0 + boxes[b].hi.0) / 2.0 - mid).abs();
        da.partial_cmp(&db)
            .unwrap()
            .then(boxes[a].id.cmp(&boxes[b].id))
    });
    let (mut left, mut right) = (ext_lo - step, f64::MIN);
    for i in order {
        let part = door_of(&boxes[i]).unwrap();
        let b = &mut boxes[i];
        let w = b.hi.0 - b.lo.0;
        b.lo.1 = part.aabb.min.1;
        b.hi.1 = part.aabb.max.1;
        if (b.lo.0 + b.hi.0) / 2.0 <= mid {
            b.hi.0 = left;
            b.lo.0 = left - w;
            left -= w + step;
        } else {
            right = right.max(ext_hi + step + (b.hi.1 - ext_lo_y));
            b.lo.0 = right;
            b.hi.0 = right + w;
            right += w + step;
        }
    }
    boxes.extend(rail_bars(plan, Some(&offs)).into_iter().map(|b| Box3 {
        id: b.id,
        name: b.name,
        lo: b.lo,
        hi: b.hi,
        tint: " iso-hw",
    }));
    // Painter's order: the viewer sits at +X +Y +Z, so the box whose centre
    // has the smallest (x + y + z) is the farthest and goes first.
    boxes.sort_by(|a, b| {
        let key = |v: &Box3| v.lo.0 + v.hi.0 + v.lo.1 + v.hi.1 + v.lo.2 + v.hi.2;
        key(a).partial_cmp(&key(b)).unwrap().then(a.id.cmp(&b.id))
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
                b.tint,
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
        if front_area.max(top_area) * scale * scale > 400.0 && !b.id.contains(':') {
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

    #[test]
    fn a_divider_on_the_centre_plane_stays_and_doors_flank_the_cabinet() {
        let plan = crate::compile_json(include_str!(
            "../../../../fixtures/wardrobe_rail/input.json"
        ));
        let offs = offsets(&plan, 1.0);
        let divider = plan.parts.iter().find(|p| p.role == "divider_1").unwrap();
        assert_eq!(offs[&divider.id], Vec3(0.0, 0.0, 0.0));
        // The drawn door polygons sit outside every other part's projection
        // on their side: the left door's right edge left of the cabinet, the
        // right door's left edge right of it.
        let body = exploded_body(&plan);
        let door_x: Vec<f64> = body
            .svg
            .lines()
            .filter(|l| l.contains("iso-front iso-tinted") && l.contains("Puerta"))
            .flat_map(|l| {
                let pts = l
                    .split("points=\"")
                    .nth(1)
                    .unwrap()
                    .split('"')
                    .next()
                    .unwrap();
                pts.split(' ')
                    .map(|p| p.split(',').next().unwrap().parse::<f64>().unwrap())
                    .collect::<Vec<_>>()
            })
            .collect();
        let carcass_x: Vec<f64> = body
            .svg
            .lines()
            .filter(|l| l.contains("class=\"iso-front\""))
            .flat_map(|l| {
                let pts = l
                    .split("points=\"")
                    .nth(1)
                    .unwrap()
                    .split('"')
                    .next()
                    .unwrap();
                pts.split(' ')
                    .map(|p| p.split(',').next().unwrap().parse::<f64>().unwrap())
                    .collect::<Vec<_>>()
            })
            .collect();
        let cmin = carcass_x.iter().cloned().fold(f64::MAX, f64::min);
        let cmax = carcass_x.iter().cloned().fold(f64::MIN, f64::max);
        assert!(
            door_x.iter().all(|&x| x < cmin || x > cmax),
            "{door_x:?} vs {cmin}..{cmax}"
        );
    }
}
