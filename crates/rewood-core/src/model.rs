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
}

impl OpGeometry {
    pub fn kind(&self) -> OperationKind {
        match self {
            OpGeometry::Drill { .. } => OperationKind::Drill,
            OpGeometry::Groove { .. } => OperationKind::Groove,
            OpGeometry::EdgeBand { .. } => OperationKind::EdgeBand,
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
