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
    /// Cut to size by its supplier (glass, mirror): not nested, not
    /// banded, not machined in the shop; ordered piece by piece and priced
    /// by the square metre.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub outsourced: bool,
    /// Price per m² of an outsourced material; 0 = unknown.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub price_per_m2: f64,
    /// Decor a part of this material gets when the spec names none. Only
    /// sheets sold in several designs (melamine) have one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_decor: Option<String>,
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

/// A design (colour or print) a sheet material is sold in: "Blanco
/// Nature", "Nogal Terracota". It changes what is ordered and how parts
/// group on sheets, never geometry. The edge band is ordered in the same
/// design: the maker sells a matching band for each one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Decor {
    pub id: String,
    pub name: String,
    pub brand: String,
    /// The maker's product line ("Nature", "Lisos").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub line: String,
    /// The maker's design code, what a supplier looks it up by.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub code: String,
    /// The print has a direction (wood): the supplier must keep it along
    /// the part's length. A plain colour may be turned.
    #[serde(default)]
    pub grain: bool,
    /// Approximate colour for the 3D view, `#rrggbb`. Display only.
    pub hex: String,
    /// Material ids it is sold in.
    pub materials: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialLibrary {
    pub version: String,
    materials: BTreeMap<String, Material>,
    edge_materials: BTreeMap<String, EdgeMaterial>,
    #[serde(default)]
    decors: BTreeMap<String, Decor>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaterialFile {
    version: String,
    materials: Vec<Material>,
    edge_materials: Vec<EdgeMaterial>,
    #[serde(default)]
    decors: Vec<Decor>,
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
            decors: file.decors.into_iter().map(|d| (d.id.clone(), d)).collect(),
        }
    }

    pub fn decor(&self, id: &str) -> Option<&Decor> {
        self.decors.get(id)
    }

    /// Decors sold in a material, by id.
    pub fn decors_for<'a>(&'a self, material: &'a str) -> impl Iterator<Item = &'a Decor> + 'a {
        self.decors
            .values()
            .filter(move |d| d.materials.iter().any(|m| m == material))
    }

    pub fn material(&self, id: &str) -> Option<&Material> {
        self.materials.get(id)
    }

    /// Edge materials, by id (sorted).
    pub fn edges(&self) -> impl Iterator<Item = &EdgeMaterial> {
        self.edge_materials.values()
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
