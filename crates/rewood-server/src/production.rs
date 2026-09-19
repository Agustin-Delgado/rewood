//! Production tracking and quality control (§52), the mutable side of an
//! order. The snapshot never changes; this record next to it says how far
//! the shop got and what was measured. Steps are per part (cut, machined,
//! edged) and per order (assembled, delivered); a QC measurement is
//! judged against the plan's finished dimensions and the profile's
//! length tolerance, and kept whatever the verdict.

use std::collections::BTreeMap;

use rewood_core::plan::ManufacturingPlan;
use serde::{Deserialize, Serialize};

pub const PART_STEPS: [&str; 3] = ["cut", "machined", "edged"];
pub const ORDER_STEPS: [&str; 2] = ["assembled", "delivered"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProductionStatus {
    #[default]
    Planned,
    InProgress,
    Done,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub at: String,
    pub what: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QcRecord {
    pub at: String,
    pub part: String,
    /// Measured finished dimensions, mm.
    pub length: f64,
    pub width: f64,
    pub thickness: f64,
    #[serde(default)]
    pub notes: String,
    /// Nominal from the plan, for the record.
    pub nominal: [f64; 3],
    /// Measured − nominal, mm.
    pub deviation: [f64; 3],
    pub tolerance: f64,
    pub pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Production {
    pub order_id: String,
    pub status: ProductionStatus,
    /// part id → step → done.
    pub parts: BTreeMap<String, BTreeMap<String, bool>>,
    pub order_steps: BTreeMap<String, bool>,
    pub events: Vec<Event>,
    pub qc: Vec<QcRecord>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionSummary {
    pub parts: usize,
    pub cut: usize,
    pub machined: usize,
    pub edged: usize,
    pub assembled: bool,
    pub delivered: bool,
    /// 0..1 over every part step plus the order steps.
    pub progress: f64,
    pub qc_records: usize,
    pub qc_failed: usize,
}

impl Production {
    pub fn new(order_id: &str, plan: &ManufacturingPlan) -> Production {
        Production {
            order_id: order_id.to_string(),
            status: ProductionStatus::Planned,
            parts: plan
                .parts
                .iter()
                .map(|p| {
                    (
                        p.id.clone(),
                        PART_STEPS.iter().map(|s| (s.to_string(), false)).collect(),
                    )
                })
                .collect(),
            order_steps: ORDER_STEPS.iter().map(|s| (s.to_string(), false)).collect(),
            events: Vec::new(),
            qc: Vec::new(),
        }
    }

    pub fn summary(&self) -> ProductionSummary {
        let count = |step: &str| {
            self.parts
                .values()
                .filter(|steps| steps.get(step).copied().unwrap_or(false))
                .count()
        };
        let total_steps = self.parts.len() * PART_STEPS.len() + ORDER_STEPS.len();
        let done_steps = self
            .parts
            .values()
            .flat_map(|s| s.values())
            .filter(|d| **d)
            .count()
            + self.order_steps.values().filter(|d| **d).count();
        ProductionSummary {
            parts: self.parts.len(),
            cut: count("cut"),
            machined: count("machined"),
            edged: count("edged"),
            assembled: self.order_steps.get("assembled").copied().unwrap_or(false),
            delivered: self.order_steps.get("delivered").copied().unwrap_or(false),
            progress: if total_steps == 0 {
                0.0
            } else {
                (done_steps as f64 / total_steps as f64 * 1000.0).round() / 1000.0
            },
            qc_records: self.qc.len(),
            qc_failed: self.qc.iter().filter(|q| !q.pass).count(),
        }
    }

    /// Mark a step; unknown parts or steps are errors. The status follows
    /// the steps unless it was set to cancelled.
    pub fn set_step(
        &mut self,
        part: Option<&str>,
        step: &str,
        done: bool,
        at: &str,
    ) -> Result<(), String> {
        match part {
            Some(part) => {
                if !PART_STEPS.contains(&step) {
                    return Err(format!(
                        "paso de pieza desconocido '{step}' (cut, machined, edged)"
                    ));
                }
                let steps = self
                    .parts
                    .get_mut(part)
                    .ok_or_else(|| format!("la orden no tiene la pieza '{part}'"))?;
                steps.insert(step.to_string(), done);
                self.events.push(Event {
                    at: at.to_string(),
                    what: format!("{part} {step} = {done}"),
                });
            }
            None => {
                if !ORDER_STEPS.contains(&step) {
                    return Err(format!(
                        "paso de orden desconocido '{step}' (assembled, delivered)"
                    ));
                }
                self.order_steps.insert(step.to_string(), done);
                self.events.push(Event {
                    at: at.to_string(),
                    what: format!("{step} = {done}"),
                });
            }
        }
        if self.status != ProductionStatus::Cancelled {
            let s = self.summary();
            self.status = if s.progress >= 1.0 {
                ProductionStatus::Done
            } else if s.progress > 0.0 {
                ProductionStatus::InProgress
            } else {
                ProductionStatus::Planned
            };
        }
        Ok(())
    }

    /// Record a measurement against the plan.
    pub fn record_qc(
        &mut self,
        plan: &ManufacturingPlan,
        part: &str,
        measured: [f64; 3],
        notes: &str,
        at: &str,
    ) -> Result<&QcRecord, String> {
        let p = plan
            .parts
            .iter()
            .find(|p| p.id == part)
            .ok_or_else(|| format!("la orden no tiene la pieza '{part}'"))?;
        let nominal = [p.dims.length, p.dims.width, p.dims.thickness];
        let tolerance = plan.profile.tolerances.length;
        let deviation = [
            round3(measured[0] - nominal[0]),
            round3(measured[1] - nominal[1]),
            round3(measured[2] - nominal[2]),
        ];
        // Thickness is the sheet's business, not the shop's: only length
        // and width are judged.
        let pass = deviation[0].abs() <= tolerance + 1e-9 && deviation[1].abs() <= tolerance + 1e-9;
        self.qc.push(QcRecord {
            at: at.to_string(),
            part: part.to_string(),
            length: measured[0],
            width: measured[1],
            thickness: measured[2],
            notes: notes.to_string(),
            nominal,
            deviation,
            tolerance,
            pass,
        });
        self.events.push(Event {
            at: at.to_string(),
            what: format!(
                "qc {part}: {}",
                if pass { "ok" } else { "fuera de tolerancia" }
            ),
        });
        Ok(self.qc.last().unwrap())
    }
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Which package files each kind of provider gets (§52 multi-provider):
/// a CNC shop needs programs and DXF, a saw shop the cut list and the
/// nesting, an assembler the documentation, purchasing the orders.
pub fn files_for_role(role: &str, path: &str) -> Option<bool> {
    let keep = match role {
        "cnc" => {
            path.starts_with("cnc/")
                || path.starts_with("parts/")
                || path == "bom/parts.csv"
                || path.starts_with("labels/")
                || path.starts_with("documentation/parts/")
                || path == "documentation/operations.csv"
        }
        "cutting" => {
            path == "documentation/nesting.svg"
                || path == "documentation/cutlist.txt"
                || path == "bom/parts.csv"
                || path.starts_with("labels/")
        }
        "assembly" => {
            path.starts_with("documentation/") && !path.starts_with("documentation/parts/")
                || path == "bom/hardware.csv"
                || path.starts_with("labels/")
        }
        "purchasing" => path.starts_with("purchasing/") || path.starts_with("bom/"),
        "all" => true,
        _ => return None,
    };
    Some(keep || path == "manifest.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_drive_the_status_and_qc_judges_length_and_width() {
        let plan =
            rewood_core::compile_json(include_str!("../../../fixtures/basic_cabinet/input.json"));
        let mut prod = Production::new("ord-000001", &plan);
        assert_eq!(prod.status, ProductionStatus::Planned);
        assert_eq!(prod.summary().parts, plan.parts.len());
        prod.set_step(Some("P001"), "cut", true, "1").unwrap();
        assert_eq!(prod.status, ProductionStatus::InProgress);
        assert!(prod.set_step(Some("P999"), "cut", true, "1").is_err());
        assert!(prod.set_step(Some("P001"), "painted", true, "1").is_err());
        for p in &plan.parts {
            for s in PART_STEPS {
                prod.set_step(Some(&p.id), s, true, "2").unwrap();
            }
        }
        prod.set_step(None, "assembled", true, "3").unwrap();
        assert_eq!(prod.status, ProductionStatus::InProgress);
        prod.set_step(None, "delivered", true, "4").unwrap();
        assert_eq!(prod.status, ProductionStatus::Done);
        assert_eq!(prod.summary().progress, 1.0);

        let p = &plan.parts[0];
        let ok = prod
            .record_qc(
                &plan,
                &p.id,
                [p.dims.length + 0.1, p.dims.width, p.dims.thickness],
                "",
                "5",
            )
            .unwrap();
        assert!(ok.pass);
        let bad = prod
            .record_qc(
                &plan,
                &p.id,
                [p.dims.length + 1.0, p.dims.width, p.dims.thickness],
                "sierra",
                "6",
            )
            .unwrap();
        assert!(!bad.pass);
        assert_eq!(bad.deviation[0], 1.0);
        assert_eq!(prod.summary().qc_failed, 1);
    }

    #[test]
    fn provider_roles_split_the_package() {
        assert_eq!(files_for_role("cnc", "cnc/P001_A.nc"), Some(true));
        assert_eq!(
            files_for_role("cnc", "documentation/report.html"),
            Some(false)
        );
        assert_eq!(
            files_for_role("cutting", "documentation/nesting.svg"),
            Some(true)
        );
        assert_eq!(
            files_for_role("assembly", "documentation/assembly.txt"),
            Some(true)
        );
        assert_eq!(files_for_role("assembly", "cnc/P001_A.nc"), Some(false));
        assert_eq!(
            files_for_role("purchasing", "purchasing/boards_supplier.csv"),
            Some(true)
        );
        assert_eq!(files_for_role("all", "manifest.json"), Some(true));
        assert_eq!(files_for_role("painter", "manifest.json"), None);
    }
}
