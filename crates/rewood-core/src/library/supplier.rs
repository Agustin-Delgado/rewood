//! Suppliers (§52): who sells each material and hardware item, so the BOM
//! can be split into purchase orders. Materials and hardware name their
//! supplier by id; an item without one lands in the "sin proveedor" order.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Supplier {
    pub id: String,
    pub name: String,
    /// Typical days from order to delivery.
    #[serde(default)]
    pub lead_days: u32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub contact: String,
    /// Free text: minimum order, delivery terms.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplierLibrary {
    pub version: String,
    suppliers: BTreeMap<String, Supplier>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SupplierFile {
    version: String,
    suppliers: Vec<Supplier>,
}

impl SupplierLibrary {
    pub fn defaults() -> SupplierLibrary {
        let file: SupplierFile = serde_json::from_str(include_str!("../../data/suppliers.json"))
            .expect("embedded suppliers.json is valid");
        SupplierLibrary {
            version: file.version,
            suppliers: file
                .suppliers
                .into_iter()
                .map(|s| (s.id.clone(), s))
                .collect(),
        }
    }

    pub fn get(&self, id: &str) -> Option<&Supplier> {
        self.suppliers.get(id)
    }

    pub fn upsert(&mut self, s: Supplier) {
        self.suppliers.insert(s.id.clone(), s);
    }
}
