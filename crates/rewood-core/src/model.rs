//! The resolved model: physically manufacturable parts with their placement,
//! edge banding and machining operations, and the joints that produced them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::geometry::{Aabb, Axis, Dims, Face, Placement, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grain {
    /// Grain runs along the part's length (local X).
    Length,
    /// Grain runs along the part's width (local Y).
    Width,
    /// Isotropic material or grain irrelevant.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OperationKind {
    Cut,
    Drill,
    Bore,
    Pocket,
    Groove,
    Dado,
    Rabbet,
    Countersink,
    Contour,
    Chamfer,
    Round,
    EdgeBand,
    Cutout,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CountersinkGeom {
    pub diameter: f64,
    pub depth: f64,
}

/// Geometry of an operation, expressed in the (u, v) frame of its face.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OpGeometry {
    #[serde(rename_all = "camelCase")]
    Drill {
        u: f64,
        v: f64,
        diameter: f64,
        /// `None` = through hole.
        depth: Option<f64>,
        through: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        countersink: Option<CountersinkGeom>,
    },
    #[serde(rename_all = "camelCase")]
    Groove {
        from: [f64; 2],
        to: [f64; 2],
        width: f64,
        depth: f64,
    },
    #[serde(rename_all = "camelCase")]
    EdgeBand {
        material: String,
        thickness: f64,
        length: f64,
    },
    /// A through opening cut out of the panel: a rectangle `width` (along
    /// u) by `height` (along v) centred on (u, v), its corners rounded to
    /// `radius` (half the smaller side = a circle or a slot). A sink, a
    /// drain, a pipe passage, a cable grommet.
    #[serde(rename_all = "camelCase")]
    Cutout {
        u: f64,
        v: f64,
        width: f64,
        height: f64,
        radius: f64,
    },
}

/// The outline of a cutout centred on (u, v), `inset` inside its edge (a
/// tool's radius, for the path it follows): a closed polygon, the first
/// point repeated at the end, counter-clockwise, each rounded corner in
/// six segments. Rounded to thousandths, like the NC.
pub fn cutout_path(
    u: f64,
    v: f64,
    width: f64,
    height: f64,
    radius: f64,
    inset: f64,
) -> Vec<[f64; 2]> {
    const CORNER_STEPS: usize = 6;
    let hw = width / 2.0 - inset;
    let hh = height / 2.0 - inset;
    let r = (radius - inset).max(0.0).min(hw).min(hh);
    let r3 = |x: f64| (x * 1000.0).round() / 1000.0 + 0.0;
    let mut out = Vec::new();
    // Corner centres, counter-clockwise from the upper right.
    let corners = [
        (hw - r, hh - r),
        (-(hw - r), hh - r),
        (-(hw - r), -(hh - r)),
        (hw - r, -(hh - r)),
    ];
    for (k, (cx, cy)) in corners.iter().enumerate() {
        if r <= 0.0 {
            out.push([r3(u + cx), r3(v + cy)]);
            continue;
        }
        for s in 0..=CORNER_STEPS {
            let a = (k as f64 * 90.0 + s as f64 * 90.0 / CORNER_STEPS as f64).to_radians();
            out.push([r3(u + cx + r * a.cos()), r3(v + cy + r * a.sin())]);
        }
    }
    out.dedup();
    if let Some(first) = out.first().copied() {
        out.push(first);
    }
    out
}

impl OpGeometry {
    pub fn kind(&self) -> OperationKind {
        match self {
            OpGeometry::Drill { .. } => OperationKind::Drill,
            OpGeometry::Groove { .. } => OperationKind::Groove,
            OpGeometry::EdgeBand { .. } => OperationKind::EdgeBand,
            OpGeometry::Cutout { .. } => OperationKind::Cutout,
        }
    }
}

/// Where an operation came from, so a finding on a hole can point back to
/// the joint and fastener that asked for it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpSource {
    pub joint: String,
    pub hardware: String,
    /// Index of the fastener along the joint; holes of the same fastener
    /// are allowed to intersect each other (a cam and its bolt path).
    pub fastener: usize,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub id: String,
    pub face: Face,
    #[serde(flatten)]
    pub geometry: OpGeometry,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<OpSource>,
}

impl Operation {
    pub fn kind(&self) -> OperationKind {
        self.geometry.kind()
    }
}

/// Raw panel size before edge banding is applied.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CutSize {
    pub length: f64,
    pub width: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    pub id: String,
    pub name: String,
    pub component: String,
    pub role: String,
    pub material: String,
    /// Decor (colour) of the sheet, for materials sold in several; its
    /// edge band is ordered in the same design.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decor: Option<String>,
    /// Finished dimensions (after edge banding).
    pub dims: Dims,
    /// Raw dimensions the panel is cut to, before banding.
    pub cut: CutSize,
    pub grain: Grain,
    /// Edge material by edge face. Only `Left`/`Right`/`Bottom`/`Top`.
    pub edges: BTreeMap<Face, String>,
    pub placement: Placement,
    pub aabb: Aabb,
    pub operations: Vec<Operation>,
    /// Cut to size by the material's supplier (glass, mirror): no nesting,
    /// no banding, no CNC program.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub outsourced: bool,
    /// Parts this one is allowed to overlap in space (a back panel sitting
    /// inside the groove of the part it is housed in).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlap_exempt: Vec<String>,
}

impl Part {
    pub fn world_point_to_face_uv(&self, face: Face, world: Vec3) -> (f64, f64) {
        self.dims.local_to_uv(face, self.placement.to_local(world))
    }

    /// The local face whose outward normal points along a world axis.
    pub fn face_facing(&self, world: Axis) -> Face {
        self.placement.face_facing(world)
    }

    pub fn next_op_id(&self) -> String {
        format!("{}-OP{:02}", self.id, self.operations.len() + 1)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fastener {
    pub hardware: String,
    pub index: usize,
    /// Point on the joint line, in furniture space.
    pub position: Vec3,
}

/// A resolved butt joint between an edge part and a face part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Joint {
    pub id: String,
    /// `butt` or `hinge`.
    pub kind: String,
    pub component: String,
    pub edge_part: String,
    pub face_part: String,
    pub hardware: Vec<String>,
    /// Contact rectangle in furniture space (flat along the face normal).
    pub contact: Aabb,
    /// Direction the joint line runs along.
    pub axis: Axis,
    pub length: f64,
    pub fasteners: Vec<Fastener>,
}
