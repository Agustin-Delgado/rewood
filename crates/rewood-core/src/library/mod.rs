//! Versioned libraries the compiler reads but never edits: materials, edge
//! bands, hardware and the manufacturing profile. Defaults ship embedded in
//! the crate as JSON; a spec may override or extend any entry by id.

pub mod hardware;
pub mod material;
pub mod profile;
pub mod supplier;

pub use hardware::{HardwareDef, HardwareLibrary, HoleLocation, HoleSpec, JointSide};
pub use material::{EdgeMaterial, Material, MaterialLibrary};
pub use profile::ManufacturingProfile;
pub use supplier::{Supplier, SupplierLibrary};

use serde::{Deserialize, Serialize};

/// Everything the compiler needs besides the furniture spec itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Libraries {
    pub materials: MaterialLibrary,
    pub hardware: HardwareLibrary,
    pub profile: ManufacturingProfile,
    #[serde(default = "SupplierLibrary::defaults")]
    pub suppliers: SupplierLibrary,
}

impl Default for Libraries {
    fn default() -> Self {
        Libraries {
            materials: MaterialLibrary::defaults(),
            hardware: HardwareLibrary::defaults(),
            profile: ManufacturingProfile::defaults(),
            suppliers: SupplierLibrary::defaults(),
        }
    }
}

/// Optional overrides carried inside a spec. Entries replace defaults with
/// the same id and add new ones; nothing is removed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LibraryOverrides {
    /// Each entry has an `id`: an existing id is patched (only the keys
    /// given change, e.g. a price), a new one has to be complete.
    #[serde(default)]
    pub materials: Vec<serde_json::Value>,
    #[serde(default)]
    pub edge_materials: Vec<serde_json::Value>,
    #[serde(default)]
    pub hardware: Vec<serde_json::Value>,
    #[serde(default)]
    pub suppliers: Vec<serde_json::Value>,
    /// A patch over the default profile: only the keys given change
    /// (`{ "nesting": { "mode": "guillotine" } }`), nested objects merge,
    /// arrays (tools) replace. A whole profile is a patch of everything.
    #[serde(default)]
    pub profile: Option<serde_json::Value>,
}

/// Deep-merge `patch` onto `base`: objects merge key by key, anything
/// else replaces.
fn merge(base: &mut serde_json::Value, patch: &serde_json::Value) {
    match (base, patch) {
        (serde_json::Value::Object(b), serde_json::Value::Object(p)) => {
            for (k, v) in p {
                match b.get_mut(k) {
                    Some(slot) if slot.is_object() && v.is_object() => merge(slot, v),
                    _ => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (b, p) => *b = p.clone(),
    }
}

impl Libraries {
    /// Apply a spec's overrides. A profile patch that does not describe a
    /// valid profile is an error the caller reports (LIB-105).
    pub fn with_overrides(mut self, o: &LibraryOverrides) -> Result<Libraries, String> {
        fn patched<T: serde::Serialize + serde::de::DeserializeOwned>(
            what: &str,
            existing: Option<&T>,
            patch: &serde_json::Value,
        ) -> Result<T, String> {
            let id = patch["id"].as_str().unwrap_or("?");
            let mut v = match existing {
                Some(e) => serde_json::to_value(e).map_err(|e| e.to_string())?,
                None => serde_json::Value::Object(Default::default()),
            };
            merge(&mut v, patch);
            serde_json::from_value(v).map_err(|e| format!("{what} '{id}': {e}"))
        }
        for m in &o.materials {
            let id = m["id"].as_str().unwrap_or_default();
            let full: Material = patched("material", self.materials.material(id), m)?;
            self.materials.upsert_material(full);
        }
        for e in &o.edge_materials {
            let id = e["id"].as_str().unwrap_or_default();
            let full: EdgeMaterial = patched("canto", self.materials.edge(id), e)?;
            self.materials.upsert_edge(full);
        }
        for h in &o.hardware {
            let id = h["id"].as_str().unwrap_or_default();
            let full: HardwareDef = patched("herraje", self.hardware.get(id), h)?;
            self.hardware.upsert(full);
        }
        for sp in &o.suppliers {
            let id = sp["id"].as_str().unwrap_or_default();
            let full: Supplier = patched("proveedor", self.suppliers.get(id), sp)?;
            self.suppliers.upsert(full);
        }
        if let Some(p) = &o.profile {
            let mut v = serde_json::to_value(&self.profile).map_err(|e| e.to_string())?;
            merge(&mut v, p);
            self.profile = serde_json::from_value(v).map_err(|e| e.to_string())?;
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn libraries_serialise_with_ids_as_keys() {
        let v = serde_json::to_value(Libraries::default()).unwrap();
        assert!(v["materials"]["materials"]["melamine_18"].is_object());
        assert!(v["materials"]["edgeMaterials"]["abs_1mm"].is_object());
        assert!(v["hardware"]["items"]["minifix_15"].is_object());
        assert_eq!(v["profile"]["id"], "generic_cnc");
    }

    #[test]
    fn a_profile_override_is_a_patch() {
        let o: LibraryOverrides = serde_json::from_str(
            r#"{ "profile": { "nesting": { "mode": "guillotine", "kerf": 3.2 }, "minEdgeDistance": 6 } }"#,
        )
        .unwrap();
        let libs = Libraries::default().with_overrides(&o).unwrap();
        assert_eq!(
            libs.profile.nesting.mode,
            crate::nesting::NestingMode::Guillotine
        );
        assert_eq!(libs.profile.nesting.kerf, 3.2);
        assert_eq!(libs.profile.nesting.margin, 10.0);
        assert_eq!(libs.profile.min_edge_distance, 6.0);
        assert_eq!(libs.profile.id, "generic_cnc");
        assert!(!libs.profile.tools.is_empty());

        let bad: LibraryOverrides =
            serde_json::from_str(r#"{ "profile": { "nesting": { "mode": "laser" } } }"#).unwrap();
        assert!(Libraries::default().with_overrides(&bad).is_err());
    }

    #[test]
    fn material_and_hardware_overrides_patch_by_id() {
        let o: LibraryOverrides = serde_json::from_str(
            r#"{ "materials": [ { "id": "melamine_18", "pricePerSheet": 48000 } ],
                 "edgeMaterials": [ { "id": "abs_1mm", "pricePerMetre": 350 } ],
                 "hardware": [ { "id": "minifix_15", "bomItems": [ { "name": "Excéntrica Minifix 15", "quantity": 1, "unitPrice": 120 }, { "name": "Perno Minifix B34", "quantity": 1, "unitPrice": 80 } ] } ] }"#,
        )
        .unwrap();
        let libs = Libraries::default().with_overrides(&o).unwrap();
        let m = libs.materials.material("melamine_18").unwrap();
        assert_eq!(m.price_per_sheet, 48000.0);
        assert_eq!(m.nominal_thickness, 18.0);
        assert_eq!(
            libs.materials.edge("abs_1mm").unwrap().price_per_metre,
            350.0
        );
        let h = libs.hardware.get("minifix_15").unwrap();
        assert_eq!(h.bom_items[0].unit_price, 120.0);
        assert!(!h.holes.is_empty());
        // A new id has to be complete.
        let incomplete: LibraryOverrides =
            serde_json::from_str(r#"{ "materials": [ { "id": "oak_20", "pricePerSheet": 1 } ] }"#)
                .unwrap();
        assert!(Libraries::default().with_overrides(&incomplete).is_err());
    }
}
