//! The colour of the furniture: body and fronts may differ, parts of two
//! colours never share a sheet, and the supplier's order names each one.

use rewood_core::diagnostics::Severity;
use rewood_core::plan::ManufacturingPlan;

fn wardrobe(decor: Option<&str>, front: Option<&str>) -> ManufacturingPlan {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/wardrobe_1800/input.json"
    );
    let mut spec: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    if let Some(d) = decor {
        spec["decor"] = d.into();
    }
    if let Some(f) = front {
        spec["frontDecor"] = f.into();
    }
    rewood_core::compile_json(&spec.to_string())
}

#[test]
fn without_a_colour_the_melamine_is_white_and_the_back_has_none() {
    let plan = wardrobe(None, None);
    for p in &plan.parts {
        match p.material.as_str() {
            "melamine_18" => assert_eq!(p.decor.as_deref(), Some("blanco_nature"), "{}", p.id),
            _ => assert_eq!(p.decor, None, "{}", p.id),
        }
    }
}

#[test]
fn fronts_take_their_own_colour_and_never_share_a_sheet_with_the_body() {
    let plan = wardrobe(Some("grafito"), Some("nogal_terracota"));
    assert!(!plan.manufacturing_blocked);
    let decor_of = |role: &str| {
        plan.parts
            .iter()
            .find(|p| p.role == role)
            .unwrap()
            .decor
            .clone()
    };
    assert_eq!(decor_of("side_left").as_deref(), Some("grafito"));
    assert_eq!(
        decor_of("drawer_1_front").as_deref(),
        Some("nogal_terracota")
    );
    assert_eq!(decor_of("drawer_1_box_front").as_deref(), Some("grafito"));
    assert!(plan
        .parts
        .iter()
        .filter(|p| p.role.contains("door"))
        .all(|p| p.decor.as_deref() == Some("nogal_terracota")));

    for layout in &plan.nesting {
        for np in &layout.parts {
            let part = plan.parts.iter().find(|p| p.id == np.part).unwrap();
            assert_eq!(
                part.decor, layout.decor,
                "{} on a {:?} sheet",
                part.id, layout.decor
            );
        }
    }
    let melamine: Vec<_> = plan
        .bom
        .sheets
        .iter()
        .filter(|s| s.material == "melamine_18")
        .collect();
    assert_eq!(melamine.len(), 2);
    assert!(melamine
        .iter()
        .any(|s| s.name.ends_with("· Nogal Terracota")));
    // The edge band is bought in each colour.
    assert!(plan
        .bom
        .consumables
        .iter()
        .any(|c| c.decor.as_deref() == Some("nogal_terracota")));

    let files = rewood_core::export::supplier::files(&plan);
    let csv = &files
        .iter()
        .find(|f| f.path == "proveedor/despiece.csv")
        .unwrap()
        .contents;
    // A wood print keeps its grain; a plain colour may turn.
    let door = csv.lines().find(|l| l.contains(";Puerta 1;")).unwrap();
    assert!(
        door.contains(";Nogal Terracota;Faplac 046NAT;18;2096;599;Sí;"),
        "{door}"
    );
    let side = csv
        .lines()
        .find(|l| l.contains(";Lateral izquierdo;"))
        .unwrap();
    assert!(
        side.contains(";Grafito;Faplac 107TXT;18;2100;500;No;"),
        "{side}"
    );
}

#[test]
fn an_unknown_colour_is_an_error_and_falls_back_to_white() {
    let plan = wardrobe(Some("fucsia"), None);
    let d = plan
        .diagnostics
        .items
        .iter()
        .find(|d| d.code == "LIB-106")
        .expect("LIB-106");
    assert_eq!(d.severity, Severity::Error);
    assert!(plan
        .parts
        .iter()
        .filter(|p| p.material == "melamine_18")
        .all(|p| p.decor.as_deref() == Some("blanco_nature")));
}
