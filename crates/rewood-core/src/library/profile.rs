//! What the workshop can actually do. Decouples the furniture from the
//! provider: the same spec compiled against two profiles yields two plans.

use serde::{Deserialize, Serialize};

use crate::model::OperationKind;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tolerances {
    pub length: f64,
    pub hole_position: f64,
    pub hole_diameter: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Drill,
    EndMill,
    CompressionBit,
}

/// A cutting tool the machine has loaded, with the feeds it runs at.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDef {
    pub id: String,
    /// Tool changer position (`T<number>`).
    pub number: u32,
    pub kind: ToolKind,
    pub diameter: f64,
    /// Deepest single pass; deeper cuts are pecked or stepped.
    pub max_depth_per_pass: f64,
    /// mm/min in the plane and plunging.
    pub feed_xy: f64,
    pub feed_z: f64,
    pub rpm: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManufacturingProfile {
    pub id: String,
    pub name: String,
    pub version: String,
    /// [length, width] of the largest part the machine can process.
    pub max_part_size: [f64; 2],
    /// [length, width] below which a part cannot be held.
    pub min_part_size: [f64; 2],
    pub min_hole_diameter: f64,
    /// Tools loaded on the machine. A hole whose diameter matches no drill
    /// is an error; grooves need an end mill no wider than they are.
    pub tools: Vec<ToolDef>,
    /// The machine has a horizontal drilling aggregate.
    pub horizontal_drilling: bool,
    /// Postprocessor id (`generic_iso`).
    pub post_processor: String,
    /// Minimum distance from a hole wall to the nearest edge of its face.
    pub min_edge_distance: f64,
    /// Minimum wall-to-wall distance between two holes on the same face.
    pub min_hole_spacing: f64,
    /// A blind hole must leave at least this much material below it.
    pub min_remaining_thickness: f64,
    pub allowed_operations: Vec<OperationKind>,
    pub tolerances: Tolerances,
    #[serde(default)]
    pub nesting: crate::nesting::NestingRules,
    /// Travel, table clearance and kinematics for the NC simulation.
    #[serde(default)]
    pub machine: crate::simulation::MachineLimits,
    /// Currency of every price in the libraries (a label, e.g. "ARS").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub currency: String,
    /// How the shop takes a panel from sheet to part, which decides what
    /// the CNC programs start from.
    #[serde(default, skip_serializing_if = "Workflow::is_default")]
    pub workflow: Workflow,
}

/// The route a part takes through the shop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Workflow {
    /// Panel saw → edge bander → CNC drilling: the CNC gets the part cut
    /// and banded, at its finished size, and only drills and grooves it.
    #[default]
    BandedPanels,
    /// A nesting router cuts and drills the raw panel before banding: the
    /// program works on the cut size (holes shifted by the band on the
    /// left and bottom edges) and cuts the outline; edge drilling waits
    /// for the band, in a setup of its own (`E`).
    NestedRouter,
}

impl Workflow {
    pub fn is_default(&self) -> bool {
        *self == Workflow::BandedPanels
    }
}

impl ManufacturingProfile {
    pub fn defaults() -> ManufacturingProfile {
        serde_json::from_str(include_str!("../../data/profile.json"))
            .expect("embedded profile.json is valid")
    }

    pub fn has_drill(&self, diameter: f64) -> bool {
        self.tools.iter().any(|t| {
            t.kind == ToolKind::Drill
                && (t.diameter - diameter).abs() <= self.tolerances.hole_diameter
        })
    }
}
