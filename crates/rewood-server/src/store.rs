//! Persistence behind the API. The specification names PostgreSQL; this
//! first store is a directory of JSON files with the same shape the
//! Postgres tables would have (projects, furniture with versions, orders
//! with their snapshot), so the service is usable without infrastructure
//! and the swap is an adapter, not a redesign.
//!
//! Ids are content-free (`fur-000001`) and sequential per collection; every
//! write goes through a single mutex so two requests never race on the
//! same counter.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use rewood_core::plan::ManufacturingPlan;
use rewood_core::spec::FurnitureSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("no existe: {0}")]
    NotFound(String),
    #[error("error de almacenamiento: {0}")]
    Io(#[from] std::io::Error),
    #[error("registro corrupto: {0}")]
    Corrupt(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

/// A furniture record: the current spec plus every earlier version, so a
/// recalculation of an old version is always possible.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Furniture {
    pub id: String,
    pub project_id: String,
    /// 1-based, bumps on every PUT.
    pub version: u32,
    pub spec: FurnitureSpec,
    pub versions: Vec<FurnitureVersion>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FurnitureVersion {
    pub version: u32,
    pub spec: FurnitureSpec,
    pub saved_at: String,
}

/// An immutable manufacturing order: the spec as it was, the plan the
/// engine produced from it, the engine and library versions, and a hash
/// of the package so a later regeneration can be checked byte for byte.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManufacturingOrder {
    pub id: String,
    pub furniture_id: String,
    pub furniture_version: u32,
    pub created_at: String,
    pub status: rewood_core::plan::PlanStatus,
    pub manufacturing_blocked: bool,
    pub snapshot: Snapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub spec: FurnitureSpec,
    pub plan: ManufacturingPlan,
    /// SHA-256 of the package files, in path order.
    pub package_sha256: String,
    pub package_files: Vec<String>,
}

pub struct FsStore {
    root: PathBuf,
    lock: Mutex<()>,
}

fn now() -> String {
    // Wall-clock only for audit fields; nothing the engine sees.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

impl FsStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<FsStore, StoreError> {
        let root = root.into();
        for dir in ["projects", "furniture", "orders", "packages", "production"] {
            std::fs::create_dir_all(root.join(dir))?;
        }
        Ok(FsStore {
            root,
            lock: Mutex::new(()),
        })
    }

    fn next_id(&self, collection: &str, prefix: &str) -> Result<String, StoreError> {
        let n = std::fs::read_dir(self.root.join(collection))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .count();
        Ok(format!("{prefix}-{:06}", n + 1))
    }

    fn write<T: Serialize>(&self, collection: &str, id: &str, value: &T) -> Result<(), StoreError> {
        let path = self.root.join(collection).join(format!("{id}.json"));
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
        std::fs::rename(tmp, path)?;
        Ok(())
    }

    fn read<T: for<'de> Deserialize<'de>>(
        &self,
        collection: &str,
        id: &str,
    ) -> Result<T, StoreError> {
        let path = self.root.join(collection).join(format!("{id}.json"));
        if !path.exists() {
            return Err(StoreError::NotFound(format!("{collection}/{id}")));
        }
        Ok(serde_json::from_slice(&std::fs::read(path)?)?)
    }

    fn list<T: for<'de> Deserialize<'de>>(&self, collection: &str) -> Result<Vec<T>, StoreError> {
        let mut names: Vec<PathBuf> = std::fs::read_dir(self.root.join(collection))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        names.sort();
        names
            .into_iter()
            .map(|p| Ok(serde_json::from_slice(&std::fs::read(p)?)?))
            .collect()
    }

    pub fn create_project(&self, name: &str) -> Result<Project, StoreError> {
        let _g = self.lock.lock().unwrap();
        let p = Project {
            id: self.next_id("projects", "prj")?,
            name: name.to_string(),
            created_at: now(),
        };
        self.write("projects", &p.id, &p)?;
        Ok(p)
    }

    pub fn project(&self, id: &str) -> Result<Project, StoreError> {
        self.read("projects", id)
    }

    pub fn projects(&self) -> Result<Vec<Project>, StoreError> {
        self.list("projects")
    }

    pub fn create_furniture(
        &self,
        project_id: &str,
        spec: FurnitureSpec,
    ) -> Result<Furniture, StoreError> {
        let _g = self.lock.lock().unwrap();
        let _: Project = self.read("projects", project_id)?;
        let ts = now();
        let f = Furniture {
            id: self.next_id("furniture", "fur")?,
            project_id: project_id.to_string(),
            version: 1,
            versions: vec![FurnitureVersion {
                version: 1,
                spec: spec.clone(),
                saved_at: ts.clone(),
            }],
            spec,
            created_at: ts.clone(),
            updated_at: ts,
        };
        self.write("furniture", &f.id, &f)?;
        Ok(f)
    }

    pub fn furniture(&self, id: &str) -> Result<Furniture, StoreError> {
        self.read("furniture", id)
    }

    pub fn furniture_list(&self) -> Result<Vec<Furniture>, StoreError> {
        self.list("furniture")
    }

    pub fn update_furniture(&self, id: &str, spec: FurnitureSpec) -> Result<Furniture, StoreError> {
        let _g = self.lock.lock().unwrap();
        let mut f: Furniture = self.read("furniture", id)?;
        f.version += 1;
        f.updated_at = now();
        f.versions.push(FurnitureVersion {
            version: f.version,
            spec: spec.clone(),
            saved_at: f.updated_at.clone(),
        });
        f.spec = spec;
        self.write("furniture", &f.id, &f)?;
        Ok(f)
    }

    /// Freeze an order: spec, plan and the package files as they are now.
    pub fn create_order(
        &self,
        furniture: &Furniture,
        plan: ManufacturingPlan,
        files: &[rewood_core::export::PackageFile],
    ) -> Result<ManufacturingOrder, StoreError> {
        let _g = self.lock.lock().unwrap();
        let id = self.next_id("orders", "ord")?;
        let dir = self.root.join("packages").join(&id);
        let mut hasher = sha2::Sha256::new();
        use sha2::Digest;
        for f in files {
            hasher.update(f.path.as_bytes());
            hasher.update(f.contents.as_bytes());
            let path = dir.join(&f.path);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, &f.contents)?;
        }
        let order = ManufacturingOrder {
            id: id.clone(),
            furniture_id: furniture.id.clone(),
            furniture_version: furniture.version,
            created_at: now(),
            status: plan.status,
            manufacturing_blocked: plan.manufacturing_blocked,
            snapshot: Snapshot {
                spec: furniture.spec.clone(),
                plan,
                package_sha256: hex::encode(hasher.finalize()),
                package_files: files.iter().map(|f| f.path.clone()).collect(),
            },
        };
        self.write("orders", &id, &order)?;
        Ok(order)
    }

    pub fn order(&self, id: &str) -> Result<ManufacturingOrder, StoreError> {
        self.read("orders", id)
    }

    /// The mutable production record of an order, created on first use.
    pub fn production(&self, order_id: &str) -> Result<crate::production::Production, StoreError> {
        match self.read("production", order_id) {
            Ok(p) => Ok(p),
            Err(StoreError::NotFound(_)) => {
                let order = self.order(order_id)?;
                Ok(crate::production::Production::new(
                    order_id,
                    &order.snapshot.plan,
                ))
            }
            Err(e) => Err(e),
        }
    }

    pub fn save_production(&self, p: &crate::production::Production) -> Result<(), StoreError> {
        let _g = self.lock.lock().unwrap();
        self.write("production", &p.order_id, p)
    }

    pub fn now_string() -> String {
        now()
    }

    pub fn orders(&self) -> Result<Vec<ManufacturingOrder>, StoreError> {
        self.list("orders")
    }

    /// One frozen package file of an order.
    pub fn package_file(&self, order_id: &str, path: &str) -> Result<Vec<u8>, StoreError> {
        let order = self.order(order_id)?;
        if !order.snapshot.package_files.iter().any(|p| p == path) {
            return Err(StoreError::NotFound(format!("{order_id}/{path}")));
        }
        Ok(std::fs::read(
            self.root.join("packages").join(order_id).join(path),
        )?)
    }

    /// Every frozen file of an order, path → bytes.
    pub fn package_files(&self, order_id: &str) -> Result<BTreeMap<String, Vec<u8>>, StoreError> {
        let order = self.order(order_id)?;
        let mut out = BTreeMap::new();
        for p in &order.snapshot.package_files {
            out.insert(
                p.clone(),
                std::fs::read(self.root.join("packages").join(order_id).join(p))?,
            );
        }
        Ok(out)
    }
}
