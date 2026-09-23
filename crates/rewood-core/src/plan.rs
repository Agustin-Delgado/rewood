//! The output: everything a workshop needs, plus the diagnostics that say
//! whether it may be manufactured.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::diagnostics::{Diagnostics, Severity};
use crate::expr::Value;
use crate::geometry::Face;
use crate::library::Libraries;
use crate::model::{Grain, Joint, OpGeometry, Part};
use crate::units::round3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    Ok,
    Warnings,
    Errors,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FurnitureRef {
    pub id: String,
    pub name: String,
    pub version: String,
}

/// Exact versions of everything that shaped this plan, so a historical
/// order can be reproduced and a silent change explained.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Versions {
    pub schema: String,
    pub engine: String,
    pub materials: String,
    pub hardware: String,
    pub profile: String,
    #[serde(default)]
    pub suppliers: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartListRow {
    /// Parts that are interchangeable: same material, cut, grain, edges
    /// and operations.
    pub part_ids: Vec<String>,
    pub name: String,
    pub quantity: usize,
    pub material: String,
    pub cut_length: f64,
    pub cut_width: f64,
    pub finished_length: f64,
    pub finished_width: f64,
    pub thickness: f64,
    pub grain: Grain,
    /// Edge band per edge, as `left/right/bottom/top` ids or `-`.
    pub edges: String,
    pub operations: usize,
    pub weight_kg: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetLine {
    pub material: String,
    pub name: String,
    pub parts: usize,
    pub net_area_m2: f64,
    pub sheet_length: f64,
    pub sheet_width: f64,
    /// Sheets the nesting used. Falls back to net area × waste factor /
    /// sheet area when nothing could be nested.
    pub estimated_sheets: usize,
    /// Net area / (sheets × sheet area).
    pub yield_ratio: f64,
    /// Sheets × price per sheet; 0 when the material has no price.
    #[serde(default)]
    pub cost: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareLine {
    pub hardware: String,
    pub name: String,
    pub quantity: usize,
    pub items: Vec<BomItemLine>,
    /// Sum of the items' costs.
    #[serde(default)]
    pub cost: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BomItemLine {
    pub name: String,
    pub quantity: f64,
    #[serde(default)]
    pub cost: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsumableLine {
    pub material: String,
    pub name: String,
    pub length_m: f64,
    #[serde(default)]
    pub cost: f64,
}

/// Costs are only as good as the prices in the libraries: a 0 means "no
/// price given", never "free". `currency` is the profile's label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bom {
    pub sheets: Vec<SheetLine>,
    pub hardware: Vec<HardwareLine>,
    pub consumables: Vec<ConsumableLine>,
    pub total_weight_kg: f64,
    /// Sheets + hardware + edge banding.
    #[serde(default)]
    pub materials_cost: f64,
    /// Simulated machining time × the machine's hourly rate.
    #[serde(default)]
    pub machining_cost: f64,
    #[serde(default)]
    pub total_cost: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub currency: String,
    /// Materials or hardware used without a price, so the total is a floor.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unpriced: Vec<String>,
}

/// One NC program as the simulation ran it: what it cuts and how long it
/// takes on the profile's feeds. The estimate is machining time only, no
/// loading or edge banding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramSummary {
    pub part: String,
    pub setup: String,
    pub operations: usize,
    pub tool_changes: usize,
    pub cut_mm: f64,
    pub rapid_mm: f64,
    pub seconds: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Machining {
    pub post_processor: String,
    pub total_seconds: f64,
    pub programs: Vec<ProgramSummary>,
}

/// One line of a purchase order: a sheet material, an edge band or a
/// hardware sub-item, with the quantity the BOM needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseLine {
    /// `sheet`, `edge_band` or `hardware`.
    pub kind: String,
    /// Library id of the material / hardware the line comes from.
    pub id: String,
    pub name: String,
    pub quantity: f64,
    /// `placas`, `m`, `u`.
    pub unit: String,
    pub unit_price: f64,
    pub cost: f64,
}

/// The BOM split by supplier (§52): what to order from whom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseOrder {
    /// Supplier id, or empty for items without one.
    pub supplier: String,
    pub name: String,
    pub lead_days: u32,
    pub lines: Vec<PurchaseLine>,
    pub cost: f64,
}

/// Purchase orders from a BOM, suppliers in id order, "sin proveedor"
/// last. Hardware sub-items of the same name and supplier merge (the
/// 4×16 screws of legs and clips are one line).
pub fn purchasing(bom: &Bom, libs: &Libraries) -> Vec<PurchaseOrder> {
    let mut by_supplier: BTreeMap<String, Vec<PurchaseLine>> = BTreeMap::new();
    for s in &bom.sheets {
        let sup = libs
            .materials
            .material(&s.material)
            .map(|m| m.supplier.clone())
            .unwrap_or_default();
        by_supplier.entry(sup).or_default().push(PurchaseLine {
            kind: "sheet".into(),
            id: s.material.clone(),
            name: format!(
                "{} ({}×{})",
                s.name,
                crate::units::round3(s.sheet_length),
                crate::units::round3(s.sheet_width)
            ),
            quantity: s.estimated_sheets as f64,
            unit: "placas".into(),
            unit_price: if s.estimated_sheets > 0 {
                s.cost / s.estimated_sheets as f64
            } else {
                0.0
            },
            cost: s.cost,
        });
    }
    for c in &bom.consumables {
        let edge = libs.materials.edge(&c.material);
        by_supplier
            .entry(edge.map(|e| e.supplier.clone()).unwrap_or_default())
            .or_default()
            .push(PurchaseLine {
                kind: "edge_band".into(),
                id: c.material.clone(),
                name: c.name.clone(),
                quantity: crate::units::round3(c.length_m),
                unit: "m".into(),
                unit_price: edge.map(|e| e.price_per_metre).unwrap_or(0.0),
                cost: c.cost,
            });
    }
    for h in &bom.hardware {
        let def = libs.hardware.get(&h.hardware);
        let sup = def.map(|d| d.supplier.clone()).unwrap_or_default();
        let lines = by_supplier.entry(sup).or_default();
        for item in &h.items {
            let unit_price = def
                .and_then(|d| d.bom_items.iter().find(|b| b.name == item.name))
                .map(|b| b.unit_price)
                .unwrap_or(0.0);
            match lines
                .iter_mut()
                .find(|l| l.kind == "hardware" && l.name == item.name && l.unit_price == unit_price)
            {
                Some(l) => {
                    l.quantity += item.quantity;
                    l.cost += item.cost;
                    if !l.id.split(',').any(|x| x == h.hardware) {
                        l.id = format!("{},{}", l.id, h.hardware);
                    }
                }
                None => lines.push(PurchaseLine {
                    kind: "hardware".into(),
                    id: h.hardware.clone(),
                    name: item.name.clone(),
                    quantity: item.quantity,
                    // A rail bar is bought by the metre.
                    unit: if def
                        .is_some_and(|d| matches!(d.kind.as_str(), "rail" | "sliding_track"))
                    {
                        "m"
                    } else {
                        "u"
                    }
                    .into(),
                    unit_price,
                    cost: item.cost,
                }),
            }
        }
    }
    let mut orders: Vec<PurchaseOrder> = by_supplier
        .into_iter()
        .map(|(id, mut lines)| {
            lines.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.name.cmp(&b.name)));
            for l in &mut lines {
                l.quantity = crate::units::round3(l.quantity);
                // What the order says has to multiply out: the cost of the
                // quantity as written, not of the unrounded one.
                l.cost = if l.unit_price > 0.0 {
                    crate::units::round3(l.quantity * l.unit_price)
                } else {
                    crate::units::round3(l.cost)
                };
            }
            let sup = libs.suppliers.get(&id);
            PurchaseOrder {
                name: sup.map(|s| s.name.clone()).unwrap_or_else(|| {
                    if id.is_empty() {
                        "Sin proveedor".into()
                    } else {
                        id.clone()
                    }
                }),
                lead_days: sup.map(|s| s.lead_days).unwrap_or(0),
                cost: crate::units::round3(lines.iter().map(|l| l.cost).sum()),
                lines,
                supplier: id,
            }
        })
        .collect();
    // Unassigned last.
    orders.sort_by(|a, b| {
        a.supplier
            .is_empty()
            .cmp(&b.supplier.is_empty())
            .then(a.supplier.cmp(&b.supplier))
    });
    orders
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManufacturingPlan {
    pub schema_version: String,
    pub furniture: FurnitureRef,
    pub versions: Versions,
    /// The manufacturing profile this plan was compiled against, in full:
    /// part of the snapshot, so the package can be regenerated later.
    pub profile: crate::library::ManufacturingProfile,
    pub status: PlanStatus,
    pub manufacturing_blocked: bool,
    pub parameters: BTreeMap<String, Value>,
    /// The spec's options, resolved (§ template options). Defaulted: a
    /// plan frozen before options existed has none.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<crate::options::PlanOption>,
    /// Components left out by their `when`, in declaration order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inactive: Vec<String>,
    pub derived: BTreeMap<String, f64>,
    pub parts: Vec<Part>,
    pub joints: Vec<Joint>,
    pub part_list: Vec<PartListRow>,
    pub bom: Bom,
    /// Sheet layouts, material by material.
    pub nesting: Vec<crate::nesting::SheetLayout>,
    /// NC programs as simulated: cut length, rapids, time. Defaulted so
    /// an order frozen before it existed still loads (§32).
    #[serde(default)]
    pub machining: Machining,
    /// The BOM split by supplier (§52).
    #[serde(default)]
    pub purchasing: Vec<PurchaseOrder>,
    pub diagnostics: Diagnostics,
}

impl Serialize for Value {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Value::Number(v) => s.serialize_f64(*v),
            Value::Bool(b) => s.serialize_bool(*b),
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Value, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            N(f64),
            B(bool),
        }
        Ok(match Raw::deserialize(d)? {
            Raw::N(v) => Value::Number(v),
            Raw::B(b) => Value::Bool(b),
        })
    }
}

impl ManufacturingPlan {
    pub fn status_from(diags: &Diagnostics) -> (PlanStatus, bool) {
        match diags.max_severity() {
            Some(Severity::Fatal) => (PlanStatus::Blocked, true),
            Some(Severity::Error) => (PlanStatus::Errors, false),
            Some(Severity::Warning) => (PlanStatus::Warnings, false),
            _ => (PlanStatus::Ok, false),
        }
    }

    /// JSON with every number rounded to 0.001 mm. This is the canonical
    /// serialisation: what the CLI prints, what fixtures compare against.
    pub fn to_json_value(&self) -> serde_json::Value {
        let mut v = serde_json::to_value(self).expect("plan serialises");
        round_numbers(&mut v);
        v
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(&self.to_json_value()).expect("plan serialises")
    }
}

fn round_numbers(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if !n.is_i64() && !n.is_u64() {
                    *v = serde_json::json!(round3(f));
                }
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(round_numbers),
        serde_json::Value::Object(map) => map.values_mut().for_each(round_numbers),
        _ => {}
    }
}

fn edges_summary(part: &Part) -> String {
    [Face::Left, Face::Right, Face::Bottom, Face::Top]
        .iter()
        .map(|f| part.edges.get(f).map(String::as_str).unwrap_or("-"))
        .collect::<Vec<_>>()
        .join("/")
}

/// Key that decides whether two parts are interchangeable on the cut list.
fn part_key(part: &Part) -> String {
    let ops: Vec<String> = part
        .operations
        .iter()
        .map(|op| {
            // Rounded like the plan output, so two parts whose holes differ
            // by floating-point noise still count as the same part.
            let mut g = serde_json::to_value(&op.geometry).unwrap();
            round_numbers(&mut g);
            format!("{:?}:{}", op.face, g)
        })
        .collect();
    format!(
        "{}|{}|{}|{:?}|{}|{}",
        part.material,
        round3(part.cut.length),
        round3(part.cut.width),
        part.grain,
        edges_summary(part),
        ops.join(";")
    )
}

/// The name a group of equal parts shares: without the trailing
/// "(component)" when that is what differs ("Lateral izquierdo (m1)" and
/// "(m3)"), and without the words that differ between them ("Frente cajón
/// 1", "… 2" → "Frente cajón"; "Lateral cajón 1 izq." → "Lateral cajón
/// izq.").
fn common_name(names: &[String]) -> String {
    let bare = |s: &str| match s.rfind(" (") {
        Some(i) if s.ends_with(')') => s[..i].to_string(),
        _ => s.to_string(),
    };
    let bares: Vec<String> = names.iter().map(|n| bare(n)).collect();
    if bares.iter().all(|b| *b == bares[0]) {
        return bares[0].clone();
    }
    let words: Vec<Vec<&str>> = bares.iter().map(|b| b.split(' ').collect()).collect();
    let first = &words[0];
    let kept: Vec<&str> = if words.iter().all(|w| w.len() == first.len()) {
        (0..first.len())
            .filter(|&i| words.iter().all(|w| w[i] == first[i]))
            .map(|i| first[i])
            .collect()
    } else {
        (0..first.len())
            .take_while(|&i| words.iter().all(|w| w.get(i) == Some(&first[i])))
            .map(|i| first[i])
            .collect()
    };
    if kept.is_empty() {
        names[0].clone()
    } else {
        kept.join(" ")
    }
}

pub fn part_list(parts: &[Part], libs: &Libraries) -> Vec<PartListRow> {
    let mut rows: Vec<(String, PartListRow)> = Vec::new();
    for part in parts {
        let key = part_key(part);
        if let Some((_, row)) = rows.iter_mut().find(|(k, _)| *k == key) {
            row.part_ids.push(part.id.clone());
            row.quantity += 1;
            continue;
        }
        let density = libs
            .materials
            .material(&part.material)
            .map(|m| m.density)
            .unwrap_or(0.0);
        let weight_kg = part.dims.volume() / 1e9 * density;
        rows.push((
            key,
            PartListRow {
                part_ids: vec![part.id.clone()],
                name: part.name.clone(),
                quantity: 1,
                material: part.material.clone(),
                cut_length: part.cut.length,
                cut_width: part.cut.width,
                finished_length: part.dims.length,
                finished_width: part.dims.width,
                thickness: part.dims.thickness,
                grain: part.grain,
                edges: edges_summary(part),
                operations: part.operations.len(),
                weight_kg,
            },
        ));
    }
    rows.into_iter()
        .map(|(_, mut r)| {
            // Equal parts of different modules or drawers: the row is
            // named by what they share, not after the first one.
            let names: Vec<String> = r
                .part_ids
                .iter()
                .filter_map(|id| parts.iter().find(|p| &p.id == id))
                .map(|p| p.name.clone())
                .collect();
            if names.len() > 1 {
                r.name = common_name(&names);
            }
            r
        })
        .collect()
}

pub fn bom(
    parts: &[Part],
    joints: &[Joint],
    nesting: &[crate::nesting::SheetLayout],
    machining_seconds: f64,
    libs: &Libraries,
    extra: &[crate::components::ExtraBom],
) -> Bom {
    let mut sheets: BTreeMap<String, SheetLine> = BTreeMap::new();
    let mut total_weight = 0.0;
    let mut unpriced: Vec<String> = Vec::new();
    for part in parts {
        let Some(m) = libs.materials.material(&part.material) else {
            continue;
        };
        total_weight += part.dims.volume() / 1e9 * m.density;
        let line = sheets
            .entry(part.material.clone())
            .or_insert_with(|| SheetLine {
                material: part.material.clone(),
                name: m.name.clone(),
                parts: 0,
                net_area_m2: 0.0,
                sheet_length: m.sheet_length,
                sheet_width: m.sheet_width,
                estimated_sheets: 0,
                yield_ratio: 0.0,
                cost: 0.0,
            });
        line.parts += 1;
        line.net_area_m2 += part.cut.length * part.cut.width / 1e6;
    }
    for line in sheets.values_mut() {
        let m = libs.materials.material(&line.material).unwrap();
        let sheet_area = m.sheet_length * m.sheet_width / 1e6;
        let nested = nesting
            .iter()
            .filter(|l| l.material == line.material)
            .count();
        line.estimated_sheets = if nested > 0 {
            nested
        } else {
            (line.net_area_m2 * m.waste_factor / sheet_area).ceil() as usize
        };
        line.yield_ratio = if line.estimated_sheets > 0 {
            line.net_area_m2 / (line.estimated_sheets as f64 * sheet_area)
        } else {
            0.0
        };
        line.cost = line.estimated_sheets as f64 * m.price_per_sheet;
        if m.price_per_sheet == 0.0 {
            unpriced.push(line.material.clone());
        }
    }

    // Every fastener, with how much panel its through holes cross (for
    // items that depend on it, like a screw's length).
    let mut crossed: BTreeMap<(&str, &str, usize), f64> = BTreeMap::new();
    for part in parts {
        // Once per part: a handle's two screws both cross the same door.
        let mut seen = std::collections::BTreeSet::new();
        for op in &part.operations {
            let (Some(s), OpGeometry::Drill { through: true, .. }) = (&op.source, &op.geometry)
            else {
                continue;
            };
            let key = (s.joint.as_str(), s.hardware.as_str(), s.fastener);
            if seen.insert(key) {
                *crossed.entry(key).or_default() += part.dims.thickness;
            }
        }
    }
    let mut hardware: BTreeMap<String, usize> = BTreeMap::new();
    let mut through_of: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for joint in joints {
        for f in &joint.fasteners {
            *hardware.entry(f.hardware.clone()).or_default() += 1;
            through_of.entry(f.hardware.clone()).or_default().push(
                crossed
                    .get(&(joint.id.as_str(), f.hardware.as_str(), f.index))
                    .copied()
                    .unwrap_or(0.0),
            );
        }
    }
    let mut hardware: Vec<HardwareLine> = hardware
        .into_iter()
        // A System 32 row is a drilling pattern, not something to buy:
        // its "fasteners" are holes (the pins go by count, below).
        .filter(|(id, _)| libs.hardware.get(id).is_none_or(|d| d.kind != "pin_row"))
        .map(|(id, quantity)| {
            let def = libs.hardware.get(&id);
            let throughs = through_of.get(&id).map(Vec::as_slice).unwrap_or(&[]);
            let items: Vec<BomItemLine> = def
                .map(|d| {
                    d.bom_items
                        .iter()
                        .filter_map(|i| {
                            // How many of these fasteners the item is for.
                            let n = match i.through {
                                None => quantity,
                                Some([lo, hi]) => throughs
                                    .iter()
                                    .filter(|t| {
                                        **t >= lo - crate::units::EPS
                                            && **t < hi - crate::units::EPS
                                    })
                                    .count(),
                            };
                            (n > 0).then(|| BomItemLine {
                                name: i.name.clone(),
                                quantity: i.quantity * n as f64,
                                cost: i.quantity * n as f64 * i.unit_price,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            if def.is_some_and(|d| d.bom_items.iter().any(|i| i.unit_price == 0.0)) {
                unpriced.push(id.clone());
            }
            HardwareLine {
                name: def.map(|d| d.name.clone()).unwrap_or_else(|| id.clone()),
                cost: items.iter().map(|i| i.cost).sum(),
                items,
                hardware: id,
                quantity,
            }
        })
        .collect();
    // Items no joint counts: bars by the metre (metres summed) or loose
    // pieces by count (shelf pins). One line per hardware id.
    let mut by_metre: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    for e in extra {
        let entry = by_metre.entry(e.hardware.clone()).or_default();
        entry.0 += e.quantity;
        entry.1 += e.metres;
    }
    for (id, (quantity, metres)) in by_metre {
        let def = libs.hardware.get(&id);
        let metres = crate::units::round3(metres);
        let factor = if metres > 0.0 {
            metres
        } else {
            quantity as f64
        };
        let items: Vec<BomItemLine> = def
            .map(|d| {
                d.bom_items
                    .iter()
                    .map(|i| BomItemLine {
                        name: i.name.clone(),
                        quantity: crate::units::round3(i.quantity * factor),
                        cost: crate::units::round3(i.quantity * factor * i.unit_price),
                    })
                    .collect()
            })
            .unwrap_or_default();
        if def.is_some_and(|d| d.bom_items.iter().any(|i| i.unit_price == 0.0)) {
            unpriced.push(id.clone());
        }
        hardware.push(HardwareLine {
            name: def.map(|d| d.name.clone()).unwrap_or_else(|| id.clone()),
            cost: items.iter().map(|i| i.cost).sum(),
            items,
            hardware: id,
            quantity,
        });
    }
    hardware.sort_by(|a, b| a.hardware.cmp(&b.hardware));

    let mut consumables: BTreeMap<String, f64> = BTreeMap::new();
    for part in parts {
        for op in &part.operations {
            if let OpGeometry::EdgeBand {
                material, length, ..
            } = &op.geometry
            {
                *consumables.entry(material.clone()).or_default() += length;
            }
        }
    }
    let consumables: Vec<ConsumableLine> = consumables
        .into_iter()
        .map(|(id, mm)| {
            let edge = libs.materials.edge(&id);
            let price = edge.map(|e| e.price_per_metre).unwrap_or(0.0);
            if price == 0.0 {
                unpriced.push(id.clone());
            }
            ConsumableLine {
                name: edge.map(|e| e.name.clone()).unwrap_or_else(|| id.clone()),
                material: id,
                length_m: mm / 1000.0,
                cost: mm / 1000.0 * price,
            }
        })
        .collect();

    let sheets: Vec<SheetLine> = sheets.into_values().collect();
    let materials_cost = sheets.iter().map(|s| s.cost).sum::<f64>()
        + hardware.iter().map(|h| h.cost).sum::<f64>()
        + consumables.iter().map(|c| c.cost).sum::<f64>();
    let machining_cost = machining_seconds / 3600.0 * libs.profile.machine.hourly_rate;
    if libs.profile.machine.hourly_rate == 0.0 && machining_seconds > 0.0 {
        unpriced.push("máquina".into());
    }
    unpriced.sort();
    unpriced.dedup();
    Bom {
        sheets,
        hardware,
        consumables,
        total_weight_kg: total_weight,
        materials_cost: crate::units::round3(materials_cost),
        machining_cost: crate::units::round3(machining_cost),
        total_cost: crate::units::round3(materials_cost + machining_cost),
        currency: libs.profile.currency.clone(),
        unpriced,
    }
}
