//! Nesting: lay the cut list out on sheets. A separate module, as the
//! specification asks — it reads parts and the profile, never the geometry
//! kernel, and its output is a list of sheet layouts.
//!
//! Algorithm: MaxRects with best-short-side-fit, parts sorted by area
//! (largest first, ids break ties). Sheets of a material are filled in
//! order; a part goes on the first sheet where it fits, else opens a new
//! one. Grain is respected: on a directional sheet a part with grain along
//! its length is never rotated; isotropic parts may turn 90°. Kerf is added
//! between parts, a margin is kept around the sheet. Deterministic.
//!
//! `mode: guillotine` instead packs in levels (strips across the sheet
//! width, parts left to right, tallest first) and emits the cut sequence a
//! panel saw needs: full-length rips between levels, cross cuts between
//! parts, then a trim where a part is shorter than its level. Worse yield,
//! but every cut goes edge to edge.

use serde::{Deserialize, Serialize};

use crate::library::material::GrainKind;
use crate::library::Libraries;
use crate::model::{Grain, Part};
use crate::units::EPS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NestingMode {
    /// Free rectangle packing for a CNC router.
    #[default]
    MaxRects,
    /// Edge-to-edge cuts for a panel / sliding table saw.
    Guillotine,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NestingRules {
    /// Width of the cut removed by the tool, kept between neighbouring parts.
    pub kerf: f64,
    /// Untouchable border around the sheet.
    pub margin: f64,
    #[serde(default)]
    pub mode: NestingMode,
}

impl Default for NestingRules {
    fn default() -> Self {
        NestingRules {
            kerf: 4.0,
            margin: 10.0,
            mode: NestingMode::MaxRects,
        }
    }
}

/// One saw cut on the sheet, in sheet coordinates, in the order they are
/// made. `stage` 1 = rips between levels (full sheet), 2 = cross cuts
/// inside a level, 3 = trims inside a strip.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SawCut {
    pub order: usize,
    pub stage: u8,
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NestedPart {
    pub part: String,
    /// Lower-left corner on the sheet: x along the sheet length, y along
    /// its width.
    pub x: f64,
    pub y: f64,
    /// Footprint on the sheet, after rotation.
    pub length: f64,
    pub width: f64,
    /// The part's own length runs along the sheet width.
    pub rotated: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetLayout {
    pub material: String,
    /// Colour of the sheet: parts of another decor never share it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decor: Option<String>,
    /// 1-based, per material and decor.
    pub index: usize,
    pub sheet_length: f64,
    pub sheet_width: f64,
    pub parts: Vec<NestedPart>,
    pub used_area_m2: f64,
    /// 0..1, of the whole sheet.
    pub waste_ratio: f64,
    /// Guillotine mode only: the cut sequence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cuts: Vec<SawCut>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Rect {
    fn contains(&self, o: &Rect) -> bool {
        o.x >= self.x - EPS
            && o.y >= self.y - EPS
            && o.x + o.w <= self.x + self.w + EPS
            && o.y + o.h <= self.y + self.h + EPS
    }

    fn overlaps(&self, o: &Rect) -> bool {
        o.x < self.x + self.w - EPS
            && o.x + o.w > self.x + EPS
            && o.y < self.y + self.h - EPS
            && o.y + o.h > self.y + EPS
    }
}

struct Sheet {
    length: f64,
    width: f64,
    free: Vec<Rect>,
    placed: Vec<NestedPart>,
    kerf: f64,
}

impl Sheet {
    /// Every part is packed as if a kerf longer and wider, so two
    /// neighbours always have a saw blade between them whichever side
    /// they meet on; the bin grows by the same kerf so the last part can
    /// still reach the margin.
    fn new(length: f64, width: f64, rules: &NestingRules) -> Sheet {
        let m = rules.margin;
        Sheet {
            length,
            width,
            free: vec![Rect {
                x: m,
                y: m,
                w: length - 2.0 * m + rules.kerf,
                h: width - 2.0 * m + rules.kerf,
            }],
            placed: Vec::new(),
            kerf: rules.kerf,
        }
    }

    /// Best short-side fit over the free rectangles; `(rect index, rotated,
    /// score)` of the best option, if any.
    fn best_fit(&self, w: f64, h: f64, allow_rotate: bool) -> Option<(usize, bool, (f64, f64))> {
        let mut best: Option<(usize, bool, (f64, f64))> = None;
        for (i, r) in self.free.iter().enumerate() {
            for rotated in [false, true] {
                if rotated && !allow_rotate {
                    continue;
                }
                let (pw, ph) = if rotated { (h, w) } else { (w, h) };
                let (pw, ph) = (pw + self.kerf, ph + self.kerf);
                if pw <= r.w + EPS && ph <= r.h + EPS {
                    let short = (r.w - pw).min(r.h - ph);
                    let long = (r.w - pw).max(r.h - ph);
                    let score = (short, long);
                    let better = match &best {
                        None => true,
                        Some((_, _, s)) => score < *s,
                    };
                    if better {
                        best = Some((i, rotated, score));
                    }
                }
            }
        }
        best
    }

    fn place(&mut self, id: &str, w: f64, h: f64, rotated: bool, rect_index: usize, kerf: f64) {
        let r = self.free[rect_index];
        let (pw, ph) = if rotated { (h, w) } else { (w, h) };
        let used = Rect {
            x: r.x,
            y: r.y,
            w: pw,
            h: ph,
        };
        self.placed.push(NestedPart {
            part: id.to_string(),
            x: used.x,
            y: used.y,
            length: pw,
            width: ph,
            rotated,
        });
        // The area the part blocks includes the kerf on its far sides.
        let blocked = Rect {
            x: used.x,
            y: used.y,
            w: pw + kerf,
            h: ph + kerf,
        };
        let mut next = Vec::new();
        for f in &self.free {
            if !f.overlaps(&blocked) {
                next.push(*f);
                continue;
            }
            // Split the free rectangle around the blocked area (MaxRects).
            if blocked.x > f.x {
                next.push(Rect {
                    x: f.x,
                    y: f.y,
                    w: blocked.x - f.x,
                    h: f.h,
                });
            }
            if blocked.x + blocked.w < f.x + f.w {
                next.push(Rect {
                    x: blocked.x + blocked.w,
                    y: f.y,
                    w: f.x + f.w - (blocked.x + blocked.w),
                    h: f.h,
                });
            }
            if blocked.y > f.y {
                next.push(Rect {
                    x: f.x,
                    y: f.y,
                    w: f.w,
                    h: blocked.y - f.y,
                });
            }
            if blocked.y + blocked.h < f.y + f.h {
                next.push(Rect {
                    x: f.x,
                    y: blocked.y + blocked.h,
                    w: f.w,
                    h: f.y + f.h - (blocked.y + blocked.h),
                });
            }
        }
        // Drop degenerate and contained rectangles; keep a stable order.
        next.retain(|r| r.w > EPS && r.h > EPS);
        let mut pruned: Vec<Rect> = Vec::new();
        for (i, r) in next.iter().enumerate() {
            let contained = next
                .iter()
                .enumerate()
                .any(|(j, o)| j != i && o != r && o.contains(r) || (j < i && o == r));
            if !contained {
                pruned.push(*r);
            }
        }
        self.free = pruned;
    }
}

/// Lay every part out, material by material. Parts whose cut size does not
/// fit the sheet at all are skipped here — rule `FAB-301` already reports
/// them — so the layouts only ever contain feasible placements.
/// A part's footprint on a sheet of `m`, honouring grain: `(w, h,
/// may_rotate)`, or `None` when it fits no sheet either way.
fn footprint(
    part: &Part,
    m: &crate::library::material::Material,
    rules: &NestingRules,
) -> Option<(f64, f64, bool)> {
    let (w, h) = (part.cut.length, part.cut.width);
    let (w, h, allow_rotate) = match (m.grain, part.grain) {
        (GrainKind::Directional, Grain::Length) => (w, h, false),
        (GrainKind::Directional, Grain::Width) => (h, w, false),
        _ => (w, h, true),
    };
    let fits_sheet = |w: f64, h: f64| {
        w <= m.sheet_length - 2.0 * rules.margin + EPS
            && h <= m.sheet_width - 2.0 * rules.margin + EPS
    };
    (fits_sheet(w, h) || (allow_rotate && fits_sheet(h, w))).then_some((w, h, allow_rotate))
}

/// Levels across the sheet width; each level as tall as its first part.
struct Level {
    y: f64,
    height: f64,
    /// Next free x.
    cursor: f64,
    parts: Vec<NestedPart>,
}

struct SawSheet {
    length: f64,
    width: f64,
    levels: Vec<Level>,
    /// Next free y for a new level.
    top: f64,
}

fn nest_guillotine(
    queue: &[&Part],
    m: &crate::library::material::Material,
    rules: &NestingRules,
) -> Vec<SawSheet> {
    let (kerf, margin) = (rules.kerf, rules.margin);
    let usable_len = m.sheet_length - 2.0 * margin;
    let usable_wid = m.sheet_width - 2.0 * margin;
    // Tallest first so a level's first part sets its height; ties by
    // length then id.
    let mut items: Vec<(&Part, f64, f64, bool)> = queue
        .iter()
        .filter_map(|p| footprint(p, m, rules).map(|(w, h, r)| (*p, w, h, r)))
        .collect();
    items.sort_by(|a, b| {
        b.2.partial_cmp(&a.2)
            .unwrap()
            .then(b.1.partial_cmp(&a.1).unwrap())
            .then(a.0.id.cmp(&b.0.id))
    });
    let mut sheets: Vec<SawSheet> = Vec::new();
    for (part, w, h, allow_rotate) in items {
        let orientations: Vec<(f64, f64, bool)> = if allow_rotate && (w - h).abs() > EPS {
            vec![(w, h, false), (h, w, true)]
        } else {
            vec![(w, h, false)]
        };
        let mut done = false;
        // 1. An existing level with room: the one that wastes the least height.
        let mut best: Option<(usize, usize, f64, f64, bool, f64)> = None;
        for (si, sheet) in sheets.iter().enumerate() {
            for (li, level) in sheet.levels.iter().enumerate() {
                for (pw, ph, rot) in &orientations {
                    let free = usable_len + margin - level.cursor;
                    if *ph <= level.height + EPS && *pw <= free + EPS {
                        let waste = level.height - ph;
                        if best.is_none_or(|b| waste < b.5 - EPS) {
                            best = Some((si, li, *pw, *ph, *rot, waste));
                        }
                    }
                }
            }
        }
        if let Some((si, li, pw, ph, rot, _)) = best {
            let level = &mut sheets[si].levels[li];
            level.parts.push(NestedPart {
                part: part.id.clone(),
                x: level.cursor,
                y: level.y,
                length: pw,
                width: ph,
                rotated: rot,
            });
            level.cursor += pw + kerf;
            done = true;
        }
        // 2. A new level on a sheet with height left.
        if !done {
            for sheet in sheets.iter_mut() {
                for (pw, ph, rot) in &orientations {
                    if sheet.top + ph <= margin + usable_wid + EPS && *pw <= usable_len + EPS {
                        sheet.levels.push(Level {
                            y: sheet.top,
                            height: *ph,
                            cursor: margin + pw + kerf,
                            parts: vec![NestedPart {
                                part: part.id.clone(),
                                x: margin,
                                y: sheet.top,
                                length: *pw,
                                width: *ph,
                                rotated: *rot,
                            }],
                        });
                        sheet.top += ph + kerf;
                        done = true;
                        break;
                    }
                }
                if done {
                    break;
                }
            }
        }
        // 3. A new sheet.
        if !done {
            let (pw, ph, rot) = orientations
                .iter()
                .copied()
                .find(|(pw, ph, _)| *pw <= usable_len + EPS && *ph <= usable_wid + EPS)
                .expect("footprint() said it fits");
            sheets.push(SawSheet {
                length: m.sheet_length,
                width: m.sheet_width,
                levels: vec![Level {
                    y: margin,
                    height: ph,
                    cursor: margin + pw + kerf,
                    parts: vec![NestedPart {
                        part: part.id.clone(),
                        x: margin,
                        y: margin,
                        length: pw,
                        width: ph,
                        rotated: rot,
                    }],
                }],
                top: margin + ph + kerf,
            });
        }
    }
    sheets
}

/// The saw sequence of a guillotine sheet: rips, cross cuts, trims.
fn saw_cuts(sheet: &SawSheet, rules: &NestingRules) -> Vec<SawCut> {
    let half = rules.kerf / 2.0;
    let mut cuts = Vec::new();
    let mut push = |stage: u8, x0: f64, y0: f64, x1: f64, y1: f64| {
        cuts.push(SawCut {
            order: 0,
            stage,
            x0: crate::units::round3(x0),
            y0: crate::units::round3(y0),
            x1: crate::units::round3(x1),
            y1: crate::units::round3(y1),
        });
    };
    // Stage 1: one rip above each level, full sheet length (the margin
    // strip at the bottom comes off first).
    push(
        1,
        0.0,
        rules.margin - half,
        sheet.length,
        rules.margin - half,
    );
    for level in &sheet.levels {
        let y = level.y + level.height + half;
        push(1, 0.0, y, sheet.length, y);
    }
    // Stage 2: cross cuts between parts of a level, the strip's height.
    for level in &sheet.levels {
        let (y0, y1) = (level.y - half, level.y + level.height + half);
        push(2, rules.margin - half, y0, rules.margin - half, y1);
        for p in &level.parts {
            let x = p.x + p.length + half;
            push(2, x, y0, x, y1);
        }
    }
    // Stage 3: a part shorter than its level is trimmed to height.
    for level in &sheet.levels {
        for p in &level.parts {
            if level.height - p.width > EPS {
                let y = p.y + p.width + half;
                push(3, p.x - half, y, p.x + p.length + half, y);
            }
        }
    }
    for (i, c) in cuts.iter_mut().enumerate() {
        c.order = i + 1;
    }
    cuts
}

pub fn nest(parts: &[Part], libs: &Libraries, rules: &NestingRules) -> Vec<SheetLayout> {
    let mut stocks: Vec<(&str, Option<&str>)> = parts
        .iter()
        .map(|p| (p.material.as_str(), p.decor.as_deref()))
        .collect();
    stocks.sort();
    stocks.dedup();

    let mut layouts = Vec::new();
    for (material, decor) in stocks {
        let Some(m) = libs.materials.stock(material, decor) else {
            continue;
        };
        let m = &m;
        // Cut to size by the supplier: nothing to lay out on a sheet.
        if m.outsourced {
            continue;
        }
        let mut queue: Vec<&Part> = parts
            .iter()
            .filter(|p| p.material == material && p.decor.as_deref() == decor)
            .collect();
        // Largest first; ties by id so the result never depends on input order.
        queue.sort_by(|a, b| {
            let area = |p: &Part| p.cut.length * p.cut.width;
            area(b)
                .partial_cmp(&area(a))
                .unwrap()
                .then_with(|| a.id.cmp(&b.id))
        });

        if rules.mode == NestingMode::Guillotine {
            for (k, sheet) in nest_guillotine(&queue, m, rules).iter().enumerate() {
                let placed: Vec<NestedPart> =
                    sheet.levels.iter().flat_map(|l| l.parts.clone()).collect();
                let used: f64 = placed.iter().map(|p| p.length * p.width).sum::<f64>() / 1e6;
                let total = sheet.length * sheet.width / 1e6;
                layouts.push(SheetLayout {
                    material: material.to_string(),
                    decor: decor.map(str::to_string),
                    index: k + 1,
                    sheet_length: sheet.length,
                    sheet_width: sheet.width,
                    parts: placed,
                    used_area_m2: used,
                    waste_ratio: 1.0 - used / total,
                    cuts: saw_cuts(sheet, rules),
                });
            }
            continue;
        }

        let mut sheets: Vec<Sheet> = Vec::new();
        for part in queue {
            // Directional grain: a part with grain along its length keeps
            // that length along the sheet length; grain along its width
            // must be rotated; no grain may go either way.
            let Some((w, h, allow_rotate)) = footprint(part, m, rules) else {
                continue;
            };
            let mut placed = false;
            for sheet in sheets.iter_mut() {
                if let Some((i, rotated, _)) = sheet.best_fit(w, h, allow_rotate) {
                    sheet.place(&part.id, w, h, rotated, i, rules.kerf);
                    placed = true;
                    break;
                }
            }
            if !placed {
                let mut sheet = Sheet::new(m.sheet_length, m.sheet_width, rules);
                let (i, rotated, _) = sheet
                    .best_fit(w, h, allow_rotate)
                    .expect("an empty sheet fits a part that passed fits_sheet");
                sheet.place(&part.id, w, h, rotated, i, rules.kerf);
                sheets.push(sheet);
            }
        }

        for (k, sheet) in sheets.iter().enumerate() {
            let used: f64 = sheet.placed.iter().map(|p| p.length * p.width).sum::<f64>() / 1e6;
            let total = sheet.length * sheet.width / 1e6;
            layouts.push(SheetLayout {
                material: material.to_string(),
                decor: decor.map(str::to_string),
                index: k + 1,
                sheet_length: sheet.length,
                sheet_width: sheet.width,
                parts: sheet.placed.clone(),
                used_area_m2: used,
                waste_ratio: 1.0 - used / total,
                cuts: Vec::new(),
            });
        }
    }
    // `rotated` says what the sheet shows: the part laid across its cut
    // length. A part with grain along its width is placed turned without
    // choosing to, and has to say so as well.
    for layout in &mut layouts {
        for np in &mut layout.parts {
            if let Some(part) = parts.iter().find(|p| p.id == np.part) {
                np.rotated = (part.cut.length - part.cut.width).abs() > EPS
                    && (np.length - part.cut.width).abs() < EPS
                    && (np.width - part.cut.length).abs() < EPS;
            }
        }
    }
    layouts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlap(a: &NestedPart, b: &NestedPart) -> bool {
        a.x < b.x + b.length && b.x < a.x + a.length && a.y < b.y + b.width && b.y < a.y + a.width
    }

    #[test]
    fn wardrobe_nests_without_overlaps_and_within_sheets() {
        let spec = include_str!("../../../fixtures/wardrobe_1800/input.json");
        let plan = crate::compile_json(spec);
        let libs = Libraries::default();
        let layouts = nest(&plan.parts, &libs, &NestingRules::default());
        let placed: usize = layouts.iter().map(|l| l.parts.len()).sum();
        assert_eq!(placed, plan.parts.len(), "every part lands on a sheet");
        for l in &layouts {
            for (i, a) in l.parts.iter().enumerate() {
                assert!(a.x >= 10.0 - EPS && a.y >= 10.0 - EPS);
                assert!(
                    a.x + a.length <= l.sheet_length - 10.0 + EPS,
                    "{} sticks out",
                    a.part
                );
                assert!(
                    a.y + a.width <= l.sheet_width - 10.0 + EPS,
                    "{} sticks out",
                    a.part
                );
                for b in &l.parts[i + 1..] {
                    assert!(!overlap(a, b), "{} overlaps {}", a.part, b.part);
                }
            }
            assert!(l.waste_ratio >= 0.0 && l.waste_ratio < 1.0);
        }
        // Directional melamine: parts with grain along their length are
        // never rotated.
        for l in layouts.iter().filter(|l| l.material == "melamine_18") {
            for p in &l.parts {
                let part = plan.parts.iter().find(|q| q.id == p.part).unwrap();
                if part.grain == Grain::Length {
                    assert!(!p.rotated, "{} rotated against the grain", p.part);
                }
            }
        }
        let melamine = layouts
            .iter()
            .filter(|l| l.material == "melamine_18")
            .count();
        assert!((3..=5).contains(&melamine), "{melamine} sheets");
        // Deterministic.
        assert_eq!(layouts, nest(&plan.parts, &libs, &NestingRules::default()));
    }

    #[test]
    fn kerf_separates_neighbours() {
        // Every fixture: a part fitting in a gap left or under an earlier
        // one used to sit closer than a kerf to it.
        let dir = format!("{}/../../fixtures", env!("CARGO_MANIFEST_DIR"));
        let mut names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        names.sort();
        let libs = Libraries::default();
        let rules = NestingRules {
            kerf: 4.0,
            margin: 10.0,
            mode: NestingMode::MaxRects,
        };
        for path in names {
            let spec = std::fs::read_to_string(path.join("input.json")).unwrap();
            let plan = crate::compile_json(&spec);
            let layouts = nest(&plan.parts, &libs, &rules);
            for l in &layouts {
                for a in &l.parts {
                    // Inside the margin.
                    assert!(a.x >= rules.margin - EPS && a.y >= rules.margin - EPS);
                    assert!(
                        a.x + a.length <= l.sheet_length - rules.margin + EPS,
                        "{:?} {}",
                        path,
                        a.part
                    );
                    assert!(
                        a.y + a.width <= l.sheet_width - rules.margin + EPS,
                        "{:?} {}",
                        path,
                        a.part
                    );
                }
                for a in &l.parts {
                    for b in &l.parts {
                        if a.part == b.part {
                            continue;
                        }
                        // Any two parts sharing a row or column are at least a kerf apart.
                        let x_gap = (b.x - (a.x + a.length)).max(a.x - (b.x + b.length));
                        let y_gap = (b.y - (a.y + a.width)).max(a.y - (b.y + b.width));
                        assert!(
                            x_gap >= rules.kerf - EPS || y_gap >= rules.kerf - EPS,
                            "{:?}: {} vs {}",
                            path,
                            a.part,
                            b.part
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn guillotine_levels_do_not_overlap_and_every_cut_is_edge_to_edge() {
        let spec = include_str!("../../../fixtures/wardrobe_1800/input.json");
        let plan = crate::compile_json(spec);
        let libs = Libraries::default();
        let rules = NestingRules {
            mode: NestingMode::Guillotine,
            ..NestingRules::default()
        };
        let layouts = nest(&plan.parts, &libs, &rules);
        let placed: usize = layouts.iter().map(|l| l.parts.len()).sum();
        assert_eq!(placed, plan.parts.len());
        for l in &layouts {
            assert!(!l.cuts.is_empty());
            for (i, a) in l.parts.iter().enumerate() {
                assert!(
                    a.x + a.length <= l.sheet_length - 10.0 + EPS,
                    "{} sticks out",
                    a.part
                );
                assert!(
                    a.y + a.width <= l.sheet_width - 10.0 + EPS,
                    "{} sticks out",
                    a.part
                );
                for b in &l.parts[i + 1..] {
                    assert!(!overlap(a, b), "{} overlaps {}", a.part, b.part);
                }
            }
            // Every stage-1 rip spans the sheet; no cut crosses a part.
            for c in &l.cuts {
                if c.stage == 1 {
                    assert_eq!((c.x0, c.x1), (0.0, l.sheet_length));
                }
                for p in &l.parts {
                    let inside_x =
                        c.x0.min(c.x1) < p.x + p.length - EPS && c.x0.max(c.x1) > p.x + EPS;
                    let inside_y =
                        c.y0.min(c.y1) < p.y + p.width - EPS && c.y0.max(c.y1) > p.y + EPS;
                    assert!(
                        !(inside_x && inside_y),
                        "cut {} crosses {}",
                        c.order,
                        p.part
                    );
                }
            }
            // Grain: a directional part keeps its length along the sheet.
            for p in &l.parts {
                let part = plan.parts.iter().find(|q| q.id == p.part).unwrap();
                if part.grain == Grain::Length && l.material.starts_with("melamine") {
                    assert!(!p.rotated, "{} rotated against the grain", p.part);
                }
            }
        }
        // Same input, same layout.
        assert_eq!(layouts, nest(&plan.parts, &libs, &rules));
        // The router packs at least as well as the saw.
        let cnc = nest(&plan.parts, &libs, &NestingRules::default());
        assert!(cnc.len() <= layouts.len());
    }
}
