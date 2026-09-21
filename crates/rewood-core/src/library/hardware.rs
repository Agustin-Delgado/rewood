//! A fastener is not a 3D model: it is a set of holes it needs, on which
//! part of the joint, plus what it adds to the BOM. "Add minifix" becomes
//! "drill cam, drill bolt path, drill thread hole, add two BOM lines".

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Which of the two joined parts a hole belongs to. In a butt joint the
/// *edge part* is the one whose edge meets the other part's large face
/// (a top meeting a side); the *face part* receives the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JointSide {
    EdgePart,
    FacePart,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HoleLocation {
    /// On the face part's large face, at the fastener point.
    ContactFace,
    /// Into the edge part's edge, centred on its thickness.
    Edge,
    /// On the edge part's preferred large face (local +Z), at
    /// `offset_from_edge` back from the contact edge. The Minifix cam, the
    /// hinge cup.
    FaceOffset,
    /// On the face part's face, `offset_from_edge` away from the joint line
    /// towards the back of the carcass. The hinge mounting plate.
    FaceInset,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Countersink {
    pub diameter: f64,
    pub depth: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HoleSpec {
    pub label: String,
    pub side: JointSide,
    pub location: HoleLocation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_from_edge: Option<f64>,
    /// Shift along the joint line from the fastener point (hinge pilot
    /// holes sit 22.5 mm either side of the cup).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_along: Option<f64>,
    /// Shift across the joint line, on the face, for fixtures with a 2D
    /// screw pattern (a leg's base plate).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_across: Option<f64>,
    pub diameter: f64,
    /// `None` means a through hole.
    #[serde(default)]
    pub depth: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub countersink: Option<Countersink>,
}

/// How fastener points are distributed along a joint line.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlacementRule {
    /// Distance from each end of the joint to the first fastener.
    pub end_offset: f64,
    /// Fasteners are added until consecutive spacing is at most this.
    pub max_spacing: f64,
    /// Optional table `[[max_length, count], ...]`, ascending: the first row
    /// whose length is not exceeded gives the count (hinges: 2 up to 900,
    /// 3 up to 1500...). Beyond the last row `max_spacing` decides.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub count_by_length: Vec<[f64; 2]>,
    /// Explicit positions from the joint start, ignoring the rules above.
    /// A slide has one fastener point at the front; its holes hang off it
    /// with `offset_along`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixed: Vec<f64>,
    /// A fixed pitch from the first position (a System 32 row of shelf
    /// pin holes): fasteners every `pitch` from `end_offset` as long as
    /// they stay `end_offset` clear of the far end. Overrides `max_spacing`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch: Option<f64>,
}

impl PlacementRule {
    /// Fastener positions along a joint of length `len`, measured from its
    /// start. Two at the ends when they fit, one centred when they do not,
    /// intermediate ones as needed to respect `max_spacing` (or as the
    /// count table says). Deterministic.
    pub fn positions(&self, len: f64) -> Vec<f64> {
        if !self.fixed.is_empty() {
            return self.fixed.clone();
        }
        if let Some(pitch) = self.pitch.filter(|p| *p > 0.0) {
            let mut out = Vec::new();
            let mut x = self.end_offset;
            while x <= len - self.end_offset + crate::units::EPS {
                out.push(x);
                x += pitch;
            }
            return out;
        }
        let usable = len - 2.0 * self.end_offset;
        if usable < 0.0 {
            return vec![len / 2.0];
        }
        let from_table = self
            .count_by_length
            .iter()
            .find(|[max_len, _]| len <= *max_len)
            .map(|[_, n]| (*n as usize).max(1));
        let n =
            from_table.unwrap_or_else(|| ((usable / self.max_spacing).ceil() as usize).max(1) + 1);
        if n == 1 {
            return vec![len / 2.0];
        }
        (0..n)
            .map(|i| self.end_offset + usable * i as f64 / (n - 1) as f64)
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BomItem {
    pub name: String,
    pub quantity: f64,
    /// Per unit, in the profile's currency; 0 = unknown.
    #[serde(default, skip_serializing_if = "crate::library::material::is_zero")]
    pub unit_price: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HardwareDef {
    pub id: String,
    pub name: String,
    pub kind: String,
    /// [min, max] panel thickness this fastener is made for.
    pub compatible_thickness: [f64; 2],
    pub placement: PlacementRule,
    /// Slides only: how long the slide is (the drawer box is that deep)
    /// and how much room it needs between box side and carcass side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slide: Option<SlideSpec>,
    /// Legs only: how tall they stand (the plinth is that tall too).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub leg: Option<LegSpec>,
    /// Hinges only: which door mount the arm is made for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hinge: Option<HingeSpec>,
    /// Catches only (magnetic catch, push latch): the plate that goes on
    /// the door to meet it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catch: Option<CatchSpec>,
    /// What one unit carries, kg: a hinge its share of the door, a slide
    /// (per pair) the drawer with its contents, a leg its share of the
    /// furniture. `None` = the library does not say, no check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_load_kg: Option<f64>,
    pub holes: Vec<HoleSpec>,
    #[serde(default)]
    pub bom_items: Vec<BomItem>,
    /// Supplier id (`libraries.suppliers`); empty = no supplier.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub supplier: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SlideSpec {
    pub length: f64,
    /// Gap between the drawer box side and the carcass side, per side.
    pub side_clearance: f64,
    /// Height of the hole line above the drawer box bottom.
    pub axis_from_box_bottom: f64,
    /// Same slide with a damper: the variant `softClose: true` on the
    /// drawers swaps in.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub soft_close: bool,
    /// `ball` (telescopic, full extension) or `roller` (three-quarter
    /// extension, the cheap one). Variants are swapped within a style.
    #[serde(default = "slide_style_ball")]
    pub style: String,
}

fn slide_style_ball() -> String {
    "ball".into()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HingeSpec {
    /// `overlay`, `half_overlay` (a door meeting another on a divider) or
    /// `inset`. The doors generator picks the mount from the geometry and
    /// swaps the hinge for the library's variant of the same opening
    /// angle.
    pub mount: String,
    /// Built-in damper; swapped in by `softClose: true` on the doors.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub soft_close: bool,
    /// Opening angle, degrees; variants are swapped within one angle.
    #[serde(default = "hinge_opening")]
    pub opening: f64,
}

fn hinge_opening() -> f64 {
    110.0
}

/// A catch on a carcass panel and the plate on the door that meets it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatchSpec {
    /// Hardware id of the plate screwed to the door (`kind: strike`);
    /// `None` = nothing on the door (an adhesive plate is a BOM item).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub strike: Option<String>,
    /// A push latch: the door opens by pressing it, so it needs unsprung
    /// hinges and no handle.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub push: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegSpec {
    pub height: f64,
    /// Base plate diameter, for the edge-distance check and the drawing.
    pub base_diameter: f64,
}

impl HardwareDef {
    /// Hinges only hang doors, slides only carry drawers; everything else
    /// is a butt-joint fastener.
    pub fn kind_matches(&self, kind: crate::components::JointKind) -> bool {
        use crate::components::JointKind;
        match kind {
            JointKind::Hinge { .. } => self.kind == "hinge",
            JointKind::Slide => self.kind == "slide",
            JointKind::Handle { .. } => self.kind == "handle",
            JointKind::Fixture { .. } => {
                matches!(
                    self.kind.as_str(),
                    "leg" | "clip" | "rail_support" | "hanger" | "catch" | "strike"
                )
            }
            JointKind::Row { .. } => self.kind == "pin_row",
            JointKind::Butt | JointKind::FaceToFace => !matches!(
                self.kind.as_str(),
                "hinge"
                    | "slide"
                    | "handle"
                    | "leg"
                    | "clip"
                    | "rail_support"
                    | "rail"
                    | "hanger"
                    | "pin_row"
                    | "pin"
                    | "catch"
                    | "strike"
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareLibrary {
    pub version: String,
    items: BTreeMap<String, HardwareDef>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HardwareFile {
    version: String,
    hardware: Vec<HardwareDef>,
}

impl HardwareLibrary {
    pub fn defaults() -> HardwareLibrary {
        let file: HardwareFile = serde_json::from_str(include_str!("../../data/hardware.json"))
            .expect("embedded hardware.json is valid");
        HardwareLibrary {
            version: file.version,
            items: file
                .hardware
                .into_iter()
                .map(|h| (h.id.clone(), h))
                .collect(),
        }
    }

    pub fn get(&self, id: &str) -> Option<&HardwareDef> {
        self.items.get(id)
    }

    /// Every item, by id (sorted).
    pub fn iter(&self) -> impl Iterator<Item = &HardwareDef> {
        self.items.values()
    }

    pub fn upsert(&mut self, h: HardwareDef) {
        self.items.insert(h.id.clone(), h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_positions() {
        let r = PlacementRule {
            end_offset: 50.0,
            max_spacing: 600.0,
            count_by_length: vec![],
            fixed: vec![],
            pitch: None,
        };
        assert_eq!(r.positions(400.0), vec![50.0, 350.0]);
        assert_eq!(r.positions(964.0), vec![50.0, 482.0, 914.0]);
        assert_eq!(r.positions(80.0), vec![40.0]);
        let r = PlacementRule {
            end_offset: 120.0,
            max_spacing: 300.0,
            count_by_length: vec![],
            fixed: vec![],
            pitch: None,
        };
        assert_eq!(r.positions(400.0), vec![120.0, 280.0]);
        assert_eq!(r.positions(964.0).len(), 4);
        let hinges = PlacementRule {
            end_offset: 100.0,
            max_spacing: 800.0,
            count_by_length: vec![[900.0, 2.0], [1500.0, 3.0], [2000.0, 4.0]],
            fixed: vec![],
            pitch: None,
        };
        assert_eq!(hinges.positions(796.0), vec![100.0, 696.0]);
        assert_eq!(hinges.positions(1200.0), vec![100.0, 600.0, 1100.0]);
        assert_eq!(hinges.positions(2000.0).len(), 4);
        assert_eq!(hinges.positions(2300.0).len(), 4); // beyond the table: spacing rule
    }

    #[test]
    fn defaults_load() {
        let lib = HardwareLibrary::defaults();
        let m = lib.get("minifix_15").unwrap();
        assert_eq!(m.holes.len(), 3);
        assert!(lib.get("dowel_8x30").is_some());
        assert!(lib.get("confirmat_7x50").unwrap().holes[0].depth.is_none());
    }
}
