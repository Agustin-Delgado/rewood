//! Design and layout checks (SPEC-21x, DESIGN-1xx): the engine builds what
//! it is asked and says what a cabinetmaker would say about it.

use rewood_core::diagnostics::Severity;
use rewood_core::plan::PlanStatus;

/// A spec from a JSON fragment that closes the carcass object and may add
/// components after it, with sensible defaults.
fn cabinet(params: &str, components: &str) -> String {
    format!(
        r#"{{
  "schemaVersion": "1.0", "id": "t", "name": "t",
  "parameters": {{ "width": 800, "height": 720, "depth": 400 {params} }},
  "material": "melamine_18", "edgeMaterial": "abs_1mm",
  "components": [
    {{ "type": "carcass", "id": "c", "joint": {{ "hardware": ["dowel_8x30"] }},
      "back": {{ "material": "hdf_3" }} {components}
  ]
}}"#
    )
}

fn findings(json: &str, code: &str) -> Vec<(Severity, String)> {
    let plan = rewood_core::compile_json(json);
    plan.diagnostics
        .items
        .iter()
        .filter(|d| d.code == code)
        .map(|d| (d.severity, d.message.clone()))
        .collect()
}

#[test]
fn fronts_over_the_same_height_of_a_bay_are_an_error() {
    let json = cabinet(
        r#", "depth": 500"#,
        r#"}, { "type": "drawers", "id": "dr", "count": 2, "zone": { "from": 0, "to": 400 },
                 "joint": { "hardware": ["dowel_8x30"] }, "slide": { "hardware": ["slide_ball_450"] } },
              { "type": "doors", "id": "do", "count": 2 }"#,
    );
    let f = findings(&json, "SPEC-210");
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!(f[0].0, Severity::Error);
    assert!(f[0].1.contains("se pisan 400 mm"), "{}", f[0].1);
    // The parts still overlap physically, and that is reported too.
    assert!(!findings(&json, "FAB-101").is_empty());
}

#[test]
fn drawers_through_shelves_clash_but_doors_over_shelves_do_not() {
    let json = cabinet(
        "",
        r#"}, { "type": "shelves", "id": "sh", "count": 1, "joint": { "hardware": ["dowel_8x30"] } },
              { "type": "doors", "id": "do", "count": 2 }"#,
    );
    assert!(findings(&json, "SPEC-210").is_empty());
    let json = cabinet(
        "",
        r#"}, { "type": "shelves", "id": "sh", "count": 1, "joint": { "hardware": ["dowel_8x30"] } },
              { "type": "drawers", "id": "dr", "count": 2, "zone": { "from": 0, "to": 400 },
                 "joint": { "hardware": ["dowel_8x30"] }, "slide": { "hardware": ["slide_ball_450"] } }"#,
    );
    assert_eq!(findings(&json, "SPEC-210").len(), 1);
}

#[test]
fn a_bay_with_fronts_that_leave_it_open_is_a_warning() {
    let json = cabinet(
        "",
        r#"}, { "type": "doors", "id": "do", "count": 2, "zone": { "from": 300, "to": "height" } }"#,
    );
    let f = findings(&json, "SPEC-212");
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(f[0].1.contains("entre 0 y 300 mm"), "{}", f[0].1);
    // Open shelving has no fronts at all: nothing to say.
    let json = cabinet(
        "",
        r#"}, { "type": "shelves", "id": "sh", "count": 2, "joint": { "hardware": ["dowel_8x30"] } }"#,
    );
    assert!(findings(&json, "SPEC-212").is_empty());
}

#[test]
fn an_empty_bay_is_pointed_out() {
    let json = cabinet(
        "",
        r#", "bays": 2 }, { "type": "shelves", "id": "sh", "bay": 1, "count": 2, "joint": { "hardware": ["dowel_8x30"] } }"#,
    );
    let f = findings(&json, "SPEC-211");
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!(f[0].0, Severity::Info);
    assert!(f[0].1.contains("bahía 2"), "{}", f[0].1);
}

#[test]
fn modules_of_a_run_that_overlap_or_leave_a_slit_are_reported() {
    let run = |x2: f64| {
        format!(
            r#"{{
  "schemaVersion": "1.0", "id": "t", "name": "t",
  "parameters": {{ "width": 600, "height": 720, "depth": 560 }},
  "material": "melamine_18", "edgeMaterial": "abs_1mm",
  "components": [
    {{ "type": "carcass", "id": "m1", "joint": {{ "hardware": ["dowel_8x30"] }}, "back": {{ "material": "hdf_3" }} }},
    {{ "type": "carcass", "id": "m2", "joint": {{ "hardware": ["dowel_8x30"] }}, "back": {{ "material": "hdf_3" }},
      "origin": {{ "x": {x2} }} }},
    {{ "type": "doors", "id": "d1", "carcass": "m1", "count": 1 }},
    {{ "type": "doors", "id": "d2", "carcass": "m2", "count": 1 }}
  ]
}}"#
        )
    };
    assert!(findings(&run(600.0), "SPEC-213").is_empty());
    let f = findings(&run(590.0), "SPEC-213");
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!(f[0].0, Severity::Error);
    assert!(f[0].1.contains("se superponen 10 mm"), "{}", f[0].1);
    let f = findings(&run(603.0), "SPEC-213");
    assert_eq!(f[0].0, Severity::Warning);
    assert!(f[0].1.contains("quedan 3 mm"), "{}", f[0].1);
    // Standing apart on purpose: not a slit.
    assert!(findings(&run(900.0), "SPEC-213").is_empty());
}

#[test]
fn shelves_and_carcass_spans_are_checked_against_the_sheet() {
    let json = cabinet(
        r#", "width": 1200"#,
        r#"}, { "type": "shelves", "id": "sh", "count": 2, "joint": { "hardware": ["dowel_8x30"] } }"#,
    );
    let f = findings(&json, "DESIGN-101");
    assert_eq!(f.len(), 2, "{f:?}");
    assert!(f.iter().any(|(_, m)| m.starts_with("tapa y base de 'c'")));
    assert!(f
        .iter()
        .any(|(_, m)| m.contains("2 paneles de 'sh'") && m.contains("1164 mm")));
    // Two bays: every span halves and the warning goes away.
    let json = cabinet(
        r#", "width": 1200"#,
        r#", "bays": 2 }, { "type": "shelves", "id": "sh", "count": 2, "joint": { "hardware": ["dowel_8x30"] } }"#,
    );
    assert!(findings(&json, "DESIGN-101").is_empty());
    // A stiffer sheet says so through its own maxSpan.
    let json = cabinet(
        r#", "width": 1200"#,
        r#"}, { "type": "shelves", "id": "sh", "count": 2, "joint": { "hardware": ["dowel_8x30"] } }"#,
    )
    .replace(
        r#""material": "melamine_18","#,
        r#""material": "melamine_18", "libraries": { "materials": [{ "id": "melamine_18", "maxSpan": 1200 }] },"#,
    );
    assert!(findings(&json, "DESIGN-101").is_empty());
}

#[test]
fn door_proportions_gap_and_handle_side_are_checked() {
    let wide = cabinet(
        r#", "width": 700"#,
        r#"}, { "type": "doors", "id": "do", "count": 1 }"#,
    );
    let f = findings(&wide, "DESIGN-102");
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(f[0].1.contains("696 mm de ancho; más de 600"), "{}", f[0].1);

    let narrow = cabinet(
        r#", "width": 380"#,
        r#"}, { "type": "doors", "id": "do", "count": 2 }"#,
    );
    assert!(findings(&narrow, "DESIGN-102")[0].1.contains("angostas"));

    let tight = cabinet(
        "",
        r#"}, { "type": "doors", "id": "do", "count": 2, "gap": 1 }"#,
    );
    assert_eq!(findings(&tight, "DESIGN-103").len(), 1);

    let handle = cabinet(
        "",
        r#"}, { "type": "doors", "id": "do", "count": 2, "handle": { "hardware": ["handle_bar_128"], "fromEdge": 300 } }"#,
    );
    let f = findings(&handle, "DESIGN-110");
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(f[0].1.contains("lado de la bisagra"));

    let ok = cabinet(
        "",
        r#"}, { "type": "doors", "id": "do", "count": 2, "handle": { "hardware": ["handle_bar_128"] } }"#,
    );
    let plan = rewood_core::compile_json(&ok);
    assert_eq!(plan.status, PlanStatus::Ok, "{:#?}", plan.diagnostics);
}

#[test]
fn drawer_heights_and_widths_are_checked() {
    let shallow = cabinet(
        r#", "depth": 500"#,
        r#"}, { "type": "drawers", "id": "dr", "count": 8,
                 "joint": { "hardware": ["dowel_8x30"] }, "slide": { "hardware": ["slide_ball_450"] } }"#,
    );
    let f = findings(&shallow, "DESIGN-104");
    assert_eq!(f.len(), 2, "{f:?}");
    assert!(
        f.iter().any(|(_, m)| m.contains("frentes de 87.75 mm")),
        "{f:?}"
    );
    assert!(
        f.iter().any(|(_, m)| m.contains("cajas de 47.75 mm")),
        "{f:?}"
    );

    let wide = cabinet(
        r#", "width": 1000, "depth": 500"#,
        r#"}, { "type": "drawers", "id": "dr", "count": 3,
                 "joint": { "hardware": ["dowel_8x30"] }, "slide": { "hardware": ["slide_ball_450"] } }"#,
    );
    let f = findings(&wide, "DESIGN-105");
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(f[0].1.contains("938.6 mm"), "{}", f[0].1);
}

#[test]
fn cramped_shelves_are_a_warning() {
    let json = cabinet(
        "",
        r#"}, { "type": "shelves", "id": "sh", "count": 5, "joint": { "hardware": ["dowel_8x30"] } }"#,
    );
    let f = findings(&json, "DESIGN-106");
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(f[0].1.contains("99 mm libres"), "{}", f[0].1);
    let json = cabinet(
        "",
        r#"}, { "type": "shelves", "id": "sh", "positions": [100, 300], "joint": { "hardware": ["dowel_8x30"] } }"#,
    );
    assert_eq!(findings(&json, "DESIGN-106").len(), 1);
}

#[test]
fn carcass_sanity_back_units_depth_and_edges() {
    let json = cabinet("", "}").replace(r#""back": { "material": "hdf_3" }"#, r#""bays": 1"#);
    let f = findings(&json, "DESIGN-107");
    assert_eq!(f.len(), 1, "{f:?}");

    let json = cabinet(r#", "width": 0.8, "height": 0.72, "depth": 0.4"#, "}");
    // Too small to have an inside: fatal, and the units hint alongside.
    let plan = rewood_core::compile_json(&json);
    assert!(plan.diagnostics.items.iter().any(|d| d.code == "SPEC-301"));

    let json = cabinet(r#", "width": 120"#, "}");
    assert!(findings(&json, "DESIGN-108")[0].1.contains("milímetros"));
    let json = cabinet(r#", "depth": 1100"#, "}");
    assert!(findings(&json, "DESIGN-108")[0].1.contains("profundidad"));

    let json = cabinet("", "}").replace(r#""edgeMaterial": "abs_1mm","#, "");
    let f = findings(&json, "DESIGN-109");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].0, Severity::Info);
    let json = cabinet(
        "",
        r#"}, { "type": "doors", "id": "do", "count": 2, "edges": "none" }"#,
    );
    let f = findings(&json, "DESIGN-109");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].0, Severity::Warning);
}

#[test]
fn a_clean_cabinet_has_nothing_to_say() {
    let json = cabinet(
        "",
        r#"}, { "type": "shelves", "id": "sh", "count": 2, "joint": { "hardware": ["dowel_8x30"] } },
              { "type": "doors", "id": "do", "count": 2, "handle": { "hardware": ["handle_bar_128"] } }"#,
    );
    let plan = rewood_core::compile_json(&json);
    assert!(plan.diagnostics.items.is_empty(), "{:#?}", plan.diagnostics);
}

#[test]
fn a_fastener_landing_in_the_back_groove_is_an_error() {
    let json = cabinet("", "}").replace(
        r#""joint": { "hardware": ["dowel_8x30"] }"#,
        r#""joint": { "hardware": ["dowel_8x30"], "placement": { "endOffset": 8, "maxSpacing": 300 } }"#,
    );
    let f = findings(&json, "FAB-208");
    // Both sides and both horizontals: the dowel 8 mm from the back edge
    // crosses the groove at 10 mm.
    assert_eq!(f.len(), 8, "{f:?}");
    assert!(f[0].1.contains("cae dentro de la ranura"));
    assert!(findings(&cabinet("", "}"), "FAB-208").is_empty());
}

#[test]
fn hinges_and_slides_are_checked_against_their_load() {
    let door = cabinet(
        r#", "width": 600, "height": 2400, "depth": 500"#,
        r#"}, { "type": "doors", "id": "do", "count": 1,
              "hinge": { "hardware": ["hinge_35_overlay"], "placement": { "endOffset": 100, "maxSpacing": 2000 } } }"#,
    )
    .replace(r#""material": "melamine_18","#, r#""material": "mdf_18","#);
    let f = findings(&door, "DESIGN-111");
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(
        f[0].1.contains("19.3 kg colgados de 3 bisagras"),
        "{}",
        f[0].1
    );
    // The library's own count (4 hinges for 2396 mm) carries it.
    let door = cabinet(
        r#", "width": 600, "height": 2400, "depth": 500"#,
        r#"}, { "type": "doors", "id": "do", "count": 1 }"#,
    )
    .replace(r#""material": "melamine_18","#, r#""material": "mdf_18","#);
    assert!(findings(&door, "DESIGN-111").is_empty());

    let drawer = cabinet(
        r#", "width": 900, "depth": 500"#,
        r#"}, { "type": "drawers", "id": "dr", "count": 1, "boxHeight": 300,
              "joint": { "hardware": ["dowel_8x30"] }, "slide": { "hardware": ["slide_ball_450"] } }"#,
    );
    assert!(findings(&drawer, "DESIGN-111").is_empty());
    let weak = drawer.replace(
        r#""material": "melamine_18","#,
        r#""material": "melamine_18", "libraries": { "hardware": [{ "id": "slide_ball_450", "maxLoadKg": 15 }] },"#,
    );
    let f = findings(&weak, "DESIGN-111");
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(
        f[0].1.contains("con 10 kg de contenido supera los 15 kg"),
        "{}",
        f[0].1
    );
}

#[test]
fn dimensions_typed_as_numbers_get_a_hint() {
    let json =
        cabinet("", "}").replace(r#""id": "c","#, r#""id": "c", "width": 700, "depth": 350,"#);
    let f = findings(&json, "DESIGN-113");
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!(f[0].0, Severity::Info);
    assert!(f[0].1.contains("width = 700, depth = 350"), "{}", f[0].1);
    // Zero and expressions are fine.
    let json = cabinet(
        "",
        r#"}, { "type": "doors", "id": "do", "count": 1, "zone": { "from": 0, "to": "height" } }"#,
    );
    assert!(findings(&json, "DESIGN-113").is_empty());
}

/// Apply every fix the plan offers, one at a time, and check each one
/// makes its finding go away.
#[test]
fn fixes_resolve_the_findings_they_come_with() {
    let json = cabinet(
        r#", "width": 1300"#,
        r#"}, { "type": "doors", "id": "do", "count": 1, "gap": 1, "edges": "none", "handle": { "hardware": ["handle_bar_128"], "fromEdge": 900 } }"#,
    )
    .replace(r#""back": { "material": "hdf_3" }"#, r#""bays": 1"#)
    .replace(r#""edgeMaterial": "abs_1mm","#, "");
    let plan = rewood_core::compile_json(&json);
    let with_fix: Vec<_> = plan
        .diagnostics
        .items
        .iter()
        .filter(|d| d.fix.is_some())
        .collect();
    let mut codes: Vec<&str> = with_fix.iter().map(|d| d.code.as_str()).collect();
    codes.sort();
    assert_eq!(
        codes,
        [
            "DESIGN-101",
            "DESIGN-102",
            "DESIGN-103",
            "DESIGN-107",
            "DESIGN-109",
            "DESIGN-109",
            "DESIGN-110"
        ],
        "{codes:?}"
    );
    for d in with_fix {
        let fix = d.fix.as_ref().unwrap();
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        rewood_core::spec::apply_fix(&mut v, fix).unwrap();
        let after = rewood_core::compile_json(&v.to_string());
        let same = after
            .diagnostics
            .items
            .iter()
            .filter(|x| x.code == d.code && x.entity == d.entity && x.message == d.message)
            .count();
        assert_eq!(
            same, 0,
            "{} '{}' did not resolve: {:#?}",
            d.code, fix.label, after.diagnostics
        );
    }
}

#[test]
fn a_slide_that_does_not_fit_offers_the_longest_that_does() {
    let json = cabinet(
        r#", "depth": 400"#,
        r#"}, { "type": "drawers", "id": "dr", "count": 2,
              "joint": { "hardware": ["dowel_8x30"] }, "slide": { "hardware": ["slide_ball_450"] } }"#,
    )
    .replace(
        r#""material": "melamine_18","#,
        r#""material": "melamine_18", "libraries": { "hardware": [
            { "id": "slide_ball_350", "name": "Corredera 350", "kind": "slide", "compatibleThickness": [12, 25],
              "placement": { "endOffset": 0, "maxSpacing": 1000, "fixed": [0] },
              "slide": { "length": 350, "sideClearance": 12.7, "axisFromBoxBottom": 22.5 }, "holes": [] },
            { "id": "slide_ball_300", "name": "Corredera 300", "kind": "slide", "compatibleThickness": [12, 25],
              "placement": { "endOffset": 0, "maxSpacing": 1000, "fixed": [0] },
              "slide": { "length": 300, "sideClearance": 12.7, "axisFromBoxBottom": 22.5 }, "holes": [] }
        ] },"#,
    );
    let plan = rewood_core::compile_json(&json);
    let d = plan
        .diagnostics
        .items
        .iter()
        .find(|d| d.code == "SPEC-307")
        .unwrap();
    let fix = d.fix.as_ref().unwrap();
    assert_eq!(fix.field, "slide.hardware");
    assert_eq!(fix.value, serde_json::json!(["slide_ball_350"]));
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    rewood_core::spec::apply_fix(&mut v, fix).unwrap();
    let after = rewood_core::compile_json(&v.to_string());
    assert!(!after.manufacturing_blocked, "{:#?}", after.diagnostics);
}
