//! Templates: a spec with options a person picks (`options`), components
//! that come and go with them (`when`), stacks repeated over a range of
//! bays (`lastBay`), and the desk pieces that only exist without a
//! pedestal (`panel`, `modesty`). Plus what the engine sorts out on its
//! own when the choices meet on one panel (holes staggered along their
//! joint, hinges and rows on one System 32 grid).

use rewood_core::diagnostics::Severity;
use rewood_core::expr::Value;
use rewood_core::model::{OpGeometry, Part};
use rewood_core::options::OptionKind;
use rewood_core::plan::{ManufacturingPlan, PlanStatus};

fn spec(params: &str, options: &str, components: &str) -> String {
    format!(
        r#"{{
  "schemaVersion": "1.0", "id": "t", "name": "t",
  "parameters": {{ {params} }},
  "options": [ {options} ],
  "material": "melamine_18", "edgeMaterial": "abs_1mm",
  "components": [ {components} ]
}}"#
    )
}

const DIMS: &str = r#""width": 900, "height": 720, "depth": 450"#;
const CARCASS: &str = r#"{ "type": "carcass", "id": "c", "joint": { "hardware": ["minifix_15", "dowel_8x30"] }, "back": { "material": "hdf_3" } }"#;

fn codes(plan: &ManufacturingPlan, code: &str) -> Vec<(Severity, String)> {
    plan.diagnostics
        .items
        .iter()
        .filter(|d| d.code == code)
        .map(|d| (d.severity, d.message.clone()))
        .collect()
}

fn of<'a>(plan: &'a ManufacturingPlan, component: &str) -> Vec<&'a Part> {
    plan.parts
        .iter()
        .filter(|p| p.component == component)
        .collect()
}

#[test]
fn when_leaves_a_component_out_and_what_hangs_from_it() {
    let json = spec(
        r#""width": 900, "height": 720, "depth": 450, "pedestal": false"#,
        "",
        &format!(
            r#"{CARCASS},
            {{ "type": "carcass", "id": "p", "when": "pedestal", "width": 400, "origin": {{ "x": 1000 }}, "joint": {{ "hardware": ["dowel_8x30"] }}, "back": {{ "material": "hdf_3" }} }},
            {{ "type": "shelves", "id": "s", "carcass": "p", "count": 2, "joint": {{ "hardware": ["dowel_8x30"] }} }},
            {{ "type": "shelves", "id": "s2", "carcass": "c", "when": "width > 600", "count": 1, "joint": {{ "hardware": ["dowel_8x30"] }} }}"#
        ),
    );
    let plan = rewood_core::compile_json(&json);
    assert_eq!(plan.inactive, vec!["p".to_string(), "s".to_string()]);
    assert!(of(&plan, "p").is_empty() && of(&plan, "s").is_empty());
    assert_eq!(of(&plan, "s2").len(), 1);
    assert!(!plan.manufacturing_blocked, "{:#?}", plan.diagnostics);

    let on = json.replace(r#""pedestal": false"#, r#""pedestal": true"#);
    let plan = rewood_core::compile_json(&on);
    assert!(plan.inactive.is_empty());
    assert_eq!(of(&plan, "s").len(), 2);
}

#[test]
fn a_when_that_is_not_a_condition_is_fatal() {
    let json = spec(
        DIMS,
        "",
        &format!(
            r#"{CARCASS}, {{ "type": "shelves", "id": "s", "when": "width", "count": 1, "joint": {{ "hardware": ["dowel_8x30"] }} }}"#
        ),
    );
    let plan = rewood_core::compile_json(&json);
    let found = codes(&plan, "SPEC-103");
    assert_eq!(found.len(), 1, "{:#?}", plan.diagnostics);
    assert_eq!(found[0].0, Severity::Fatal);
    assert!(
        found[0].1.contains("se esperaba una condición"),
        "{}",
        found[0].1
    );
}

#[test]
fn a_constraint_with_a_false_when_does_not_apply() {
    let json = spec(r#""width": 900, "height": 720, "depth": 450, "pedestal": false"#, "", CARCASS).replace(
        r#""components""#,
        r#""constraints": [ { "id": "k", "expr": "width > 5000", "when": "pedestal" } ], "components""#,
    );
    let plan = rewood_core::compile_json(&json);
    assert!(codes(&plan, "CON-001").is_empty());
    let plan =
        rewood_core::compile_json(&json.replace(r#""pedestal": false"#, r#""pedestal": true"#));
    assert_eq!(codes(&plan, "CON-001").len(), 1);
}

#[test]
fn last_bay_repeats_a_drawer_stack_over_a_range_of_bays() {
    let json = spec(
        r#""width": 1800, "height": 800, "depth": 500, "cols": 2"#,
        "",
        r#"{ "type": "carcass", "id": "c", "bays": 3, "joint": { "hardware": ["minifix_15", "dowel_8x30"] }, "back": { "material": "hdf_3" } },
        { "type": "drawers", "id": "d", "bay": 2, "lastBay": "1 + cols", "count": 2,
          "joint": { "hardware": ["dowel_8x30"], "placement": { "endOffset": 40, "maxSpacing": 150 } },
          "slide": { "hardware": ["slide_ball_450"] }, "handle": { "hardware": ["handle_bar_128"] } },
        { "type": "shelves", "id": "s", "bay": 1, "count": 2, "joint": { "hardware": ["dowel_8x30"] } }"#,
    );
    let plan = rewood_core::compile_json(&json);
    assert!(!plan.manufacturing_blocked, "{:#?}", plan.diagnostics);
    let fronts: Vec<&str> = of(&plan, "d")
        .into_iter()
        .filter(|p| p.name.starts_with("Frente cajón"))
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(
        fronts,
        vec![
            "Frente cajón 1 bahía 2",
            "Frente cajón 2 bahía 2",
            "Frente cajón 1 bahía 3",
            "Frente cajón 2 bahía 3"
        ]
    );
    // Two stacks on either side of one divider: their slide screws face
    // each other across it and have been moved clear.
    assert!(
        codes(&plan, "FAB-205").is_empty(),
        "{:#?}",
        codes(&plan, "FAB-205")
    );

    let empty =
        rewood_core::compile_json(&json.replace(r#""lastBay": "1 + cols""#, r#""lastBay": 1"#));
    assert!(codes(&empty, "SPEC-204")[0]
        .1
        .contains("el rango está vacío"));
    let out =
        rewood_core::compile_json(&json.replace(r#""lastBay": "1 + cols""#, r#""lastBay": 4"#));
    assert!(codes(&out, "SPEC-204")[0].1.contains("la bahía 4"));
}

#[test]
fn shelves_on_both_sides_of_a_divider_do_not_drill_the_same_spot() {
    let json = spec(
        r#""width": 1200, "height": 1800, "depth": 400"#,
        "",
        r#"{ "type": "carcass", "id": "c", "bays": 2, "joint": { "hardware": ["minifix_15", "dowel_8x30"] }, "back": { "material": "hdf_3" } },
        { "type": "shelves", "id": "s", "count": 3, "joint": { "hardware": ["dowel_8x30"] } }"#,
    );
    let plan = rewood_core::compile_json(&json);
    assert!(
        codes(&plan, "FAB-205").is_empty(),
        "{:#?}",
        codes(&plan, "FAB-205")
    );
    assert_eq!(plan.status, PlanStatus::Ok, "{:#?}", plan.diagnostics);
    // The divider's dowel holes: every one on one face is off every one
    // on the other by at least a hole's width.
    let divider = plan
        .parts
        .iter()
        .find(|p| p.role.starts_with("divider"))
        .unwrap();
    let holes = |front: bool| -> Vec<(f64, f64)> {
        divider
            .operations
            .iter()
            .filter(|o| (o.face == rewood_core::geometry::Face::Front) == front)
            .filter_map(|o| match o.geometry {
                OpGeometry::Drill { u, v, .. } if o.face.is_large() => Some((u, v)),
                _ => None,
            })
            .collect()
    };
    let (a, b) = (holes(true), holes(false));
    assert!(!a.is_empty() && !b.is_empty());
    for (u, v) in &a {
        for (x, y) in &b {
            let d = ((u - x).powi(2) + (v - y).powi(2)).sqrt();
            assert!(d >= 8.0, "({u}, {v}) y ({x}, {y}) a {d} mm");
        }
    }
    // Moving holes is as deterministic as the rest.
    let deterministic = rewood_core::compile_json(&json).to_json_pretty();
    assert_eq!(deterministic, plan.to_json_pretty());
}

#[test]
fn a_raised_carcass_keeps_hinges_on_its_shelf_pin_rows() {
    // A pedestal on a 100 mm plinth: rows and hinge cups share the grid
    // measured from the carcass floor, wherever the carcass stands.
    for z in [0, 100, 116, 250] {
        let json = spec(
            DIMS,
            "",
            &format!(
                r#"{{ "type": "carcass", "id": "c", "width": 450, "origin": {{ "z": {z} }}, "joint": {{ "hardware": ["minifix_15", "dowel_8x30"] }}, "back": {{ "material": "hdf_3" }} }},
                {{ "type": "shelves", "id": "s", "count": 2, "support": "pins" }},
                {{ "type": "doors", "id": "d", "count": 1, "handle": {{ "hardware": ["handle_bar_128"] }} }}"#
            ),
        );
        let plan = rewood_core::compile_json(&json);
        assert!(
            codes(&plan, "FAB-205").is_empty(),
            "z = {z}: {:#?}",
            codes(&plan, "FAB-205")
        );
        // No plate hole of its own: they are the row's.
        let plates = plan
            .parts
            .iter()
            .flat_map(|p| &p.operations)
            .filter(|o| {
                o.source
                    .as_ref()
                    .is_some_and(|s| s.label.starts_with("plate"))
            })
            .count();
        assert_eq!(plates, 0, "z = {z}");
    }
}

#[test]
fn a_desk_on_two_end_panels_with_a_modesty_panel() {
    let json = spec(
        r#""width": 1200, "height": 722, "depth": 580, "modesty": true"#,
        "",
        r#"{ "type": "panel", "id": "l", "x": 0 },
        { "type": "panel", "id": "r", "x": "width", "facing": "left" },
        { "type": "worktop", "id": "top", "overhang": { "front": 20 } },
        { "type": "modesty", "id": "m", "when": "modesty", "height": 300 }"#,
    );
    let plan = rewood_core::compile_json(&json);
    assert_eq!(plan.status, PlanStatus::Ok, "{:#?}", plan.diagnostics);
    let l = of(&plan, "l")[0];
    let r = of(&plan, "r")[0];
    let top = of(&plan, "top")[0];
    let m = of(&plan, "m")[0];
    assert_eq!((l.aabb.min.0, l.aabb.max.0), (0.0, 18.0));
    assert_eq!((r.aabb.min.0, r.aabb.max.0), (1182.0, 1200.0));
    assert_eq!((top.aabb.min.2, top.aabb.max.2), (722.0, 740.0));
    assert_eq!((top.aabb.min.1, top.aabb.max.1), (0.0, 600.0));
    // Between the panels, 20 from the back, right under the top.
    assert_eq!((m.aabb.min.0, m.aabb.max.0), (18.0, 1182.0));
    assert_eq!((m.aabb.min.1, m.aabb.max.1), (20.0, 38.0));
    assert_eq!((m.aabb.min.2, m.aabb.max.2), (422.0, 722.0));
    // Panels into the top, the modesty panel into both panels.
    let joined = |a: &str, b: &str| {
        plan.joints.iter().any(|j| {
            (j.edge_part == a && j.face_part == b) || (j.edge_part == b && j.face_part == a)
        })
    };
    assert!(joined(&l.id, &top.id) && joined(&r.id, &top.id));
    assert!(joined(&m.id, &l.id) && joined(&m.id, &r.id));

    // Without the modesty panel it sways: a warning, still made.
    let loose =
        rewood_core::compile_json(&json.replace(r#""modesty": true"#, r#""modesty": false"#));
    let found = codes(&loose, "DESIGN-117");
    assert_eq!(found.len(), 1, "{:#?}", loose.diagnostics);
    assert_eq!(found[0].0, Severity::Warning);
    assert!(!loose.manufacturing_blocked);
}

#[test]
fn options_resolve_their_bounds_and_flag_values_outside_them() {
    let json = spec(
        r#""width": 900, "height": 720, "depth": 450, "drawers": 3, "side": 1, "open": true, "twice": "2 * width""#,
        r#"{ "param": "drawers", "label": "Cajones", "min": 1, "max": "floor(height / 150)", "step": 1 },
           { "param": "side", "label": "Lado", "choices": [ { "value": 0, "label": "izquierda" }, { "value": 1, "label": "derecha" } ] },
           { "param": "open", "label": "Abierto", "when": "drawers > 5" }"#,
        CARCASS,
    );
    let plan = rewood_core::compile_json(&json);
    assert!(
        plan.diagnostics
            .items
            .iter()
            .all(|d| !d.code.starts_with("SPEC-5")),
        "{:#?}",
        plan.diagnostics
    );
    let o = &plan.options;
    assert_eq!(o.len(), 3);
    assert_eq!(
        (o[0].kind, o[0].min, o[0].max),
        (OptionKind::Number, Some(1.0), Some(4.0))
    );
    assert_eq!(
        (o[1].kind, o[1].value),
        (OptionKind::Choice, Value::Number(1.0))
    );
    assert_eq!((o[2].kind, o[2].active), (OptionKind::Toggle, false));

    // Out of range: an error that knows how to fix itself.
    let plan = rewood_core::compile_json(&json.replace(r#""drawers": 3"#, r#""drawers": 9"#));
    let bad = plan
        .diagnostics
        .items
        .iter()
        .find(|d| d.code == "SPEC-503")
        .unwrap();
    assert_eq!(bad.severity, Severity::Error);
    let fix = bad.fix.as_ref().unwrap();
    assert_eq!(
        (fix.field.as_str(), &fix.value),
        ("parameters.drawers", &serde_json::json!(4.0))
    );
    // A choice that is not offered.
    let plan = rewood_core::compile_json(&json.replace(r#""side": 1"#, r#""side": 7"#));
    assert!(codes(&plan, "SPEC-503")[0].1.contains("izquierda, derecha"));
    // An option over a formula, or over nothing.
    let plan =
        rewood_core::compile_json(&json.replace(r#""param": "side""#, r#""param": "twice""#));
    assert_eq!(codes(&plan, "SPEC-502").len(), 1);
    let plan = rewood_core::compile_json(&json.replace(r#""param": "side""#, r#""param": "nope""#));
    assert_eq!(codes(&plan, "SPEC-501").len(), 1);
}
