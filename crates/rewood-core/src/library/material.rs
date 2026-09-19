use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrainKind {
    /// Sheet has a visible direction (melamine wood prints, plywood).
    Directional,
    /// Isotropic sheet (MDF, HDF): parts may rotate freely in nesting.
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Material {
    pub id: String,
    pub name: String,
    /// Design thickness: what the parametric model uses.
    pub nominal_thickness: f64,
    /// Measured thickness of the real sheet: what depth checks use.
    pub actual_thickness: f64,
    pub sheet_length: f64,
    pub sheet_width: f64,
    pub grain: GrainKind,
    /// kg/m³, for part weights.
    pub density: f64,
    /// Multiplier applied to net area when estimating sheets without nesting.
    #[serde(default = "default_waste")]
    pub waste_factor: f64,
    /// Purchase price of one sheet, in the profile's currency; 0 = unknown.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub price_per_sheet: f64,
    /// Supplier id (`libraries.suppliers`); empty = no supplier.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub supplier: String,
    /// Longest unsupported span a horizontal panel of this sheet should
    /// bridge before it sags visibly (mm); `None` = 50 × thickness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_span: Option<f64>,
}

pub(crate) fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

fn default_waste() -> f64 {
    1.15
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EdgeMaterial {
    pub id: String,
    pub name: String,
    pub thickness: f64,
    /// Per metre, in the profile's currency; 0 = unknown.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub price_per_metre: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub supplier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialLibrary {
    pub version: String,
    materials: BTreeMap<String, Material>,
    edge_materials: BTreeMap<String, EdgeMaterial>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaterialFile {
    version: String,
    materials: Vec<Material>,
    edge_materials: Vec<EdgeMaterial>,
}

impl MaterialLibrary {
    pub fn defaults() -> MaterialLibrary {
        let file: MaterialFile = serde_json::from_str(include_str!("../../data/materials.json"))
            .expect("embedded materials.json is valid");
        MaterialLibrary {
            version: file.version,
            materials: file
                .materials
                .into_iter()
                .map(|m| (m.id.clone(), m))
                .collect(),
            edge_materials: file
                .edge_materials
                .into_iter()
                .map(|e| (e.id.clone(), e))
                .collect(),
        }
    }

    pub fn material(&self, id: &str) -> Option<&Material> {
        self.materials.get(id)
    }

    pub fn edge(&self, id: &str) -> Option<&EdgeMaterial> {
        self.edge_materials.get(id)
    }

    pub fn upsert_material(&mut self, m: Material) {
        self.materials.insert(m.id.clone(), m);
    }

    pub fn upsert_edge(&mut self, e: EdgeMaterial) {
        self.edge_materials.insert(e.id.clone(), e);
    }
}
