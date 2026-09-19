//! The input: a furniture specification. This is the engine's own versioned
//! format, independent of any UI and of any exchange format (STEP, DXF are
//! adapters, never the model).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::diagnostics::Severity;
use crate::library::hardware::PlacementRule;
use crate::library::LibraryOverrides;
use crate::params::ParamInput;

pub const SCHEMA_VERSION: &str = "1.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FurnitureSpec {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    /// Named values and expressions. Every relevant dimension should come
    /// from here, never be typed as a literal into a component.
    #[serde(default)]
    pub parameters: BTreeMap<String, ParamInput>,
    /// Default panel material for every component that does not name one.
    pub material: String,
    /// Default edge band material; `None` = no banding anywhere.
    #[serde(default)]
    pub edge_material: Option<String>,
    pub components: Vec<ComponentSpec>,
    #[serde(default)]
    pub constraints: Vec<ConstraintSpec>,
    #[serde(default)]
    pub libraries: LibraryOverrides,
}

fn default_version() -> String {
    "1.0".into()
}

/// A number or an expression over the parameters.
pub type NumOrExpr = ParamInput;

fn expr(s: &str) -> NumOrExpr {
    ParamInput::Expr(s.to_string())
}

fn num(v: f64) -> NumOrExpr {
    ParamInput::Number(v)
}

/// Where a front (door, drawer front) sits relative to the carcass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontMount {
    /// Over the carcass edges, covering them (the usual kitchen door).
    #[default]
    Overlay,
    /// Inside the opening, flush with the carcass front. A door needs an
    /// inset hinge; a drawer becomes an inner drawer (behind a door).
    Inset,
}

/// Which edges of a component's panels get the default edge band.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeBanding {
    /// What the component considers sensible: the visible front edge on a
    /// carcass or shelf, all four edges on a door.
    #[default]
    Default,
    None,
    /// Only the edge facing the front of the furniture.
    Front,
    /// All four edges.
    All,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JointSpec {
    /// Hardware ids, all applied to every joint of the component. Each
    /// hardware distributes its own fasteners along the joint line.
    pub hardware: Vec<String>,
    /// Overrides every listed hardware's own distribution rule for the
    /// joints of this component (two dowels 40 mm from the ends of a short
    /// drawer box instead of the library's 120 mm rule).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<PlacementRule>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GrooveSpec {
    /// Distance from the back edge of the carcass panels to the groove.
    #[serde(default = "groove_inset")]
    pub inset: NumOrExpr,
    #[serde(default = "groove_depth")]
    pub depth: NumOrExpr,
    /// Added to the back panel thickness to get the groove width.
    #[serde(default = "groove_clearance")]
    pub clearance: NumOrExpr,
}

fn groove_inset() -> NumOrExpr {
    num(10.0)
}
fn groove_depth() -> NumOrExpr {
    num(8.0)
}
fn groove_clearance() -> NumOrExpr {
    num(0.2)
}

impl Default for GrooveSpec {
    fn default() -> Self {
        GrooveSpec {
            inset: groove_inset(),
            depth: groove_depth(),
            clearance: groove_clearance(),
        }
    }
}

/// A handle on a door or drawer front.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HandleSpec {
    pub hardware: Vec<String>,
    /// Doors: distance from the opening edge to the handle's centre line.
    #[serde(default = "handle_from_edge")]
    pub from_edge: NumOrExpr,
    /// Distance from the panel's bottom to the handle's centre; omitted =
    /// the panel's middle.
    #[serde(default)]
    pub position: Option<NumOrExpr>,
}

fn handle_from_edge() -> NumOrExpr {
    num(40.0)
}

/// A height range inside a carcass, in mm from the carcass bottom. Lets
/// drawers take the lower part of a bay and shelves and a door the rest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ZoneSpec {
    pub from: NumOrExpr,
    pub to: NumOrExpr,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BackSpec {
    pub material: String,
    #[serde(default)]
    pub groove: GrooveSpec,
}

// Drawers carry many more fields than shelves; the spec is parsed once and
// never stored in bulk, so the size difference is irrelevant.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ComponentSpec {
    /// Two sides, a top and a bottom, optionally a back in a groove.
    /// Placed with its back-bottom-left corner at the furniture origin.
    #[serde(rename_all = "camelCase")]
    Carcass {
        id: String,
        #[serde(default = "expr_width")]
        width: NumOrExpr,
        #[serde(default = "expr_height")]
        height: NumOrExpr,
        #[serde(default = "expr_depth")]
        depth: NumOrExpr,
        #[serde(default)]
        material: Option<String>,
        joint: JointSpec,
        #[serde(default)]
        back: Option<BackSpec>,
        /// Number of bays; `bays - 1` vertical dividers split the inner
        /// width evenly. Dependent components pick a bay with `bay`.
        #[serde(default = "one")]
        bays: NumOrExpr,
        /// Explicit inner width of each bay, overriding the even split.
        /// One entry may be `"auto"` and takes what is left.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        bay_widths: Vec<NumOrExpr>,
        #[serde(default)]
        edges: EdgeBanding,
        /// Legs under the bottom panel, optionally with a plinth.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        legs: Option<LegsSpec>,
        /// Where this carcass stands in furniture space (its bottom-left-
        /// back corner); default the origin. Several carcasses side by side
        /// make a run of modules. Everything that refers to the carcass
        /// (shelves, doors, drawers) moves with it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin: Option<OriginSpec>,
    },
    /// `count` shelves spread evenly inside a carcass bay, or shelves at
    /// explicit heights (`positions`): a fixed shelf / horizontal divider.
    #[serde(rename_all = "camelCase")]
    Shelves {
        id: String,
        #[serde(default)]
        carcass: Option<String>,
        /// 1-based bay index; omitted = every bay.
        #[serde(default)]
        bay: Option<NumOrExpr>,
        #[serde(default)]
        zone: Option<ZoneSpec>,
        /// Shelves spread evenly over the zone. Exclusive with `positions`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        count: Option<NumOrExpr>,
        /// Underside height of each shelf, mm from the carcass bottom (outer
        /// face), like zones. A shelf here is a fixed shelf: it takes the
        /// full inner depth minus `setback`, which defaults to 0 for it.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        positions: Vec<NumOrExpr>,
        /// Distance the shelf front sits back from the carcass front
        /// (default 20 for spread shelves, 0 for fixed ones).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        setback: Option<NumOrExpr>,
        #[serde(default)]
        material: Option<String>,
        joint: JointSpec,
        #[serde(default)]
        edges: EdgeBanding,
    },
    /// `count` overlay doors across the carcass front, each hung on its
    /// carcass side.
    #[serde(rename_all = "camelCase")]
    Doors {
        id: String,
        #[serde(default)]
        carcass: Option<String>,
        /// 1-based bay index; omitted = every bay.
        #[serde(default)]
        bay: Option<NumOrExpr>,
        /// Consecutive bays one door set covers, from `bay` (a wide door
        /// over two narrow bays). Default 1.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        span: Option<NumOrExpr>,
        #[serde(default)]
        zone: Option<ZoneSpec>,
        count: NumOrExpr,
        /// Gap between doors and around the door set.
        #[serde(default = "door_gap")]
        gap: NumOrExpr,
        #[serde(default)]
        mount: FrontMount,
        #[serde(default)]
        material: Option<String>,
        /// Hinges hanging each door on its carcass side. `null` = none.
        /// With `mount: inset` the default overlay hinge is swapped for
        /// the library's inset one.
        #[serde(default = "door_hinge")]
        hinge: Option<JointSpec>,
        /// Vertical handle on the opening edge.
        #[serde(default)]
        handle: Option<HandleSpec>,
        #[serde(default)]
        edges: EdgeBanding,
    },
    /// `count` drawers stacked from the bottom of a carcass, each with an
    /// overlay front and a box (two sides, front, back, grooved bottom)
    /// running on side-mounted slides.
    #[serde(rename_all = "camelCase")]
    Drawers {
        id: String,
        #[serde(default)]
        carcass: Option<String>,
        /// 1-based bay index; omitted = every bay.
        #[serde(default)]
        bay: Option<NumOrExpr>,
        #[serde(default)]
        zone: Option<ZoneSpec>,
        count: NumOrExpr,
        /// Height of each drawer front. Default: fronts share the carcass
        /// height evenly.
        #[serde(default)]
        front_height: Option<NumOrExpr>,
        /// Gap between fronts and around the set.
        #[serde(default = "door_gap")]
        gap: NumOrExpr,
        /// `inset` = inner drawer: the front sits inside the opening, so a
        /// door can close over the whole stack.
        #[serde(default)]
        mount: FrontMount,
        /// Inner drawers only: how far the fronts sit back from the carcass
        /// front (room for an inset door). Default 0.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        setback: Option<NumOrExpr>,
        /// Box height. Default: front height minus 40 mm.
        #[serde(default)]
        box_height: Option<NumOrExpr>,
        /// Material of the fronts (default: the furniture material).
        #[serde(default)]
        material: Option<String>,
        /// Material of the box sides, front and back.
        #[serde(default)]
        box_material: Option<String>,
        /// Material of the box bottom, housed in a groove.
        #[serde(default = "drawer_bottom_material")]
        bottom_material: String,
        #[serde(default)]
        bottom_groove: GrooveSpec,
        /// Fasteners for the box corners.
        joint: JointSpec,
        /// Slides, one hardware id; its `slide` block sets the box depth.
        slide: JointSpec,
        /// Screws fixing the front panel to the box front, from inside the
        /// box. `null` = not modelled.
        #[serde(default = "drawer_front_fixing")]
        front_fixing: Option<JointSpec>,
        /// Horizontal handle centred on the front.
        #[serde(default)]
        handle: Option<HandleSpec>,
        #[serde(default)]
        edges: EdgeBanding,
    },

    /// A hanging rail across a bay: two supports on the panels bounding
    /// the bay and a bar cut to its width. Generates no panel.
    #[serde(rename_all = "camelCase")]
    Rail {
        id: String,
        #[serde(default)]
        carcass: Option<String>,
        /// 1-based bay index; omitted = every bay.
        #[serde(default)]
        bay: Option<NumOrExpr>,
        #[serde(default)]
        zone: Option<ZoneSpec>,
        /// Rail centre below the top of its zone (the carcass top).
        #[serde(default = "rail_from_top")]
        from_top: NumOrExpr,
        /// The bar, for the BOM (kind `rail`).
        #[serde(default = "default_rail")]
        hardware: Vec<String>,
        /// End supports screwed to the bay's panels.
        #[serde(default = "default_rail_support")]
        supports: Vec<String>,
    },
}

fn rail_from_top() -> NumOrExpr {
    num(60.0)
}
fn default_rail() -> Vec<String> {
    vec!["rail_oval_30".into()]
}
fn default_rail_support() -> Vec<String> {
    vec!["rail_support_oval".into()]
}

impl ComponentSpec {
    pub fn kind(&self) -> &'static str {
        match self {
            ComponentSpec::Carcass { .. } => "carcass",
            ComponentSpec::Shelves { .. } => "shelves",
            ComponentSpec::Doors { .. } => "doors",
            ComponentSpec::Drawers { .. } => "drawers",
            ComponentSpec::Rail { .. } => "rail",
        }
    }
}

fn drawer_bottom_material() -> String {
    "hdf_3".into()
}
fn drawer_front_fixing() -> Option<JointSpec> {
    Some(JointSpec {
        hardware: vec!["screw_4x30_face".into()],
        placement: None,
    })
}

fn one() -> NumOrExpr {
    num(1.0)
}
fn expr_width() -> NumOrExpr {
    expr("width")
}
fn expr_height() -> NumOrExpr {
    expr("height")
}
fn expr_depth() -> NumOrExpr {
    expr("depth")
}
fn door_gap() -> NumOrExpr {
    num(2.0)
}
fn door_hinge() -> Option<JointSpec> {
    Some(JointSpec {
        hardware: vec!["hinge_35_overlay".into()],
        placement: None,
    })
}

impl ComponentSpec {
    pub fn id(&self) -> &str {
        match self {
            ComponentSpec::Carcass { id, .. }
            | ComponentSpec::Shelves { id, .. }
            | ComponentSpec::Doors { id, .. }
            | ComponentSpec::Drawers { id, .. }
            | ComponentSpec::Rail { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OriginSpec {
    #[serde(default = "zero")]
    pub x: NumOrExpr,
    #[serde(default = "zero")]
    pub y: NumOrExpr,
    #[serde(default = "zero")]
    pub z: NumOrExpr,
}

fn zero() -> NumOrExpr {
    num(0.0)
}

/// Legs screwed to the underside of the carcass bottom: rows at `inset`
/// from the front and back edges, spread along the width by the leg's own
/// placement rule (or `maxSpacing`). The carcass keeps its origin at the
/// bottom panel's underside; the legs reach below z = 0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LegsSpec {
    #[serde(default = "default_leg_hardware")]
    pub hardware: Vec<String>,
    /// Leg centre from the carcass outer edges (front/back and sides).
    #[serde(default = "leg_inset")]
    pub inset: NumOrExpr,
    /// Largest distance between legs along the width.
    #[serde(default = "leg_spacing")]
    pub max_spacing: NumOrExpr,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plinth: Option<PlinthSpec>,
}

/// A recessed panel between the front legs, clipped to them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlinthSpec {
    #[serde(default)]
    pub material: Option<String>,
    /// Plinth front face back from the carcass front.
    #[serde(default = "plinth_setback")]
    pub setback: NumOrExpr,
    #[serde(default = "default_clip_hardware")]
    pub clips: Vec<String>,
    #[serde(default)]
    pub edges: EdgeBanding,
}

fn default_leg_hardware() -> Vec<String> {
    vec!["leg_adjustable_100".into()]
}
fn default_clip_hardware() -> Vec<String> {
    vec!["plinth_clip".into()]
}
fn leg_inset() -> NumOrExpr {
    num(50.0)
}
fn leg_spacing() -> NumOrExpr {
    num(600.0)
}
fn plinth_setback() -> NumOrExpr {
    num(40.0)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConstraintSpec {
    pub id: String,
    /// Boolean expression over parameters and component-derived values
    /// (`carcass.inner_width`, `doors.door_width`). False = finding.
    pub expr: String,
    #[serde(default = "constraint_severity")]
    pub severity: Severity,
    #[serde(default)]
    pub message: Option<String>,
}

fn constraint_severity() -> Severity {
    Severity::Error
}

impl FurnitureSpec {
    pub fn from_json(json: &str) -> Result<FurnitureSpec, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Apply a [`Fix`](crate::diagnostics::Fix) to a spec as JSON: find the
/// component by id (or take the root), walk the dotted path creating
/// objects on the way, set the value (`null` removes the key). Works on
/// the JSON and not on `FurnitureSpec` so the UI's own document, with its
/// key order, is what changes.
pub fn apply_fix(
    spec: &mut serde_json::Value,
    fix: &crate::diagnostics::Fix,
) -> Result<(), String> {
    let target = if fix.component.is_empty() {
        spec
    } else {
        spec.get_mut("components")
            .and_then(|c| c.as_array_mut())
            .and_then(|list| {
                list.iter_mut()
                    .find(|c| c.get("id").and_then(|v| v.as_str()) == Some(&fix.component))
            })
            .ok_or_else(|| format!("no hay un componente '{}'", fix.component))?
    };
    let mut keys: Vec<&str> = fix.field.split('.').collect();
    let last = keys.pop().ok_or("campo vacío")?;
    let mut node = target;
    for k in keys {
        if !node.is_object() {
            return Err(format!("'{k}' no es un objeto"));
        }
        node = node
            .as_object_mut()
            .unwrap()
            .entry(k)
            .or_insert_with(|| serde_json::json!({}));
    }
    let obj = node
        .as_object_mut()
        .ok_or_else(|| format!("'{}' no es un objeto", fix.field))?;
    if fix.value.is_null() {
        obj.remove(last);
    } else {
        obj.insert(last.to_string(), fix.value.clone());
    }
    Ok(())
}
