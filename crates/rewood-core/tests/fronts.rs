//! Rails, doors spanning bays, inset doors and inner drawers.

use rewood_core::plan::PlanStatus;

const WARDROBE: &str = r#"{
  "schemaVersion": "1.0", "id": "w", "name": "w",
  "parameters": { "width": 1200, "height": 2100, "depth": 560, "drawer_top": 500 },
  "material": "melamine_18", "edgeMaterial": "abs_1mm",
  "components": [
    { "type": "carcass", "id": "c", "bays": 2, "joint": { "hardware": ["dowel_8x30"] }, "back": { "material": "hdf_3" } },
    { "type": "rail", "id": "rail", "bay": 1 },
    { "type": "shelves", "id": "sh", "bay": 2, "count": 4, "joint": { "hardware": ["dowel_8x30"] } },
    { "type": "drawers", "id": "dr", "bay": 1, "count": 2, "zone": { "from": 0, "to": "drawer_top" }, "mount": "inset", "setback": 20,
      "joint": { "hardware": ["dowel_8x30"] }, "slide": { "hardware": ["slide_ball_450"] } },
    { "type": "doors", "id": "do", "bay": 1, "count": 1, "mount": "inset" }
  ]
}"#;

fn part<'a>(
    plan: &'a rewood_core::plan::ManufacturingPlan,
    role: &str,
) -> &'a rewood_core::model::Part {
    plan.parts
        .iter()
        .find(|p| p.role == role)
        .unwrap_or_else(|| panic!("{role}"))
}

#[test]
fn a_rail_is_two_supports_and_a_bar_by_the_metre() {
    let plan = rewood_core::compile_json(WARDROBE);
    assert!(plan.diagnostics.items.is_empty(), "{:#?}", plan.diagnostics);
    let supports: Vec<_> = plan
        .joints
        .iter()
        .filter(|j| j.component == "rail")
        .collect();
    assert_eq!(supports.len(), 2);
    // Left side and the divider, at 60 below the top's underside, mid depth.
    assert_eq!(supports[0].face_part, part(&plan, "side_left").id);
    assert_eq!(supports[1].face_part, part(&plan, "divider_1").id);
    let z = supports[0].fasteners[0].position.2;
    assert_eq!(z, 2100.0 - 18.0 - 60.0);
    // Two screws on the side's inner face, 10 above and below the rail.
    let side = part(&plan, "side_left");
    let screws: Vec<f64> = side
        .operations
        .iter()
        .filter(|op| {
            op.source
                .as_ref()
                .is_some_and(|s| s.hardware == "rail_support_oval")
        })
        .filter_map(|op| match op.geometry {
            rewood_core::model::OpGeometry::Drill { u, .. } => Some(u),
            _ => None,
        })
        .collect();
    assert_eq!(screws.len(), 2);
    assert_eq!((screws[1] - screws[0]).abs(), 20.0);
    let bar = plan
        .bom
        .hardware
        .iter()
        .find(|h| h.hardware == "rail_oval_30")
        .unwrap();
    assert_eq!(bar.quantity, 1);
    assert_eq!(bar.items[0].quantity, 0.573);
    assert_eq!(plan.derived["rail.hang"], 2004.0);
}

#[test]
fn a_low_rail_or_shelves_under_it_are_reported() {
    let low = WARDROBE.replace(
        r#""bay": 1 },"#,
        r#""bay": 1, "zone": { "from": 0, "to": 900 } },"#,
    );
    let plan = rewood_core::compile_json(&low);
    assert!(
        plan.diagnostics
            .items
            .iter()
            .any(|d| d.code == "DESIGN-114"),
        "{:#?}",
        plan.diagnostics
    );
    // Shelves in the hanging height of the same bay.
    let shelved = WARDROBE.replace(r#""id": "sh", "bay": 2,"#, r#""id": "sh", "bay": 1,"#);
    let plan = rewood_core::compile_json(&shelved);
    assert!(
        plan.diagnostics
            .items
            .iter()
            .any(|d| d.code == "SPEC-210" && d.message.contains("barral")),
        "{:#?}",
        plan.diagnostics
    );
}

#[test]
fn inset_doors_sit_inside_the_opening_with_an_inset_hinge() {
    let plan = rewood_core::compile_json(WARDROBE);
    let door = part(&plan, "door_1");
    // Flush with the carcass front, between the panels, inside the bay.
    assert_eq!(door.aabb.max.1, 560.0);
    assert_eq!(door.aabb.min.1, 542.0);
    assert_eq!(door.aabb.min.2, 20.0);
    assert_eq!(door.aabb.max.2, 2080.0);
    assert_eq!(door.aabb.min.0, 20.0);
    let hinge = plan.joints.iter().find(|j| j.kind == "hinge").unwrap();
    assert_eq!(hinge.hardware, ["hinge_35_inset"]);
    // Inner drawers set back 20: their fronts end where the door begins.
    let front = part(&plan, "drawer_1_front");
    assert_eq!(front.aabb.max.1, 540.0);
    // The door hangs on the left side: swung open, its 18 mm stand in
    // front of the opening from x = 20 to 38. The drawers sit on two
    // strips of 18 mm board on that side (one alone does not clear 38 + 3),
    // box and front.
    assert_eq!(front.aabb.min.0, 20.0 + 36.0);
    assert_eq!(front.aabb.max.0, 600.0 - 9.0 - 2.0);
    let side = part(&plan, "drawer_1_side_left");
    assert_eq!(side.aabb.min.0, 18.0 + 36.0 + 12.7);
    let spacers = plan
        .bom
        .hardware
        .iter()
        .find(|h| h.hardware == "slide_spacer_36")
        .unwrap();
    assert_eq!(spacers.quantity, 2);
    assert_eq!(plan.derived["dr.spacer_left"], 36.0);
    assert!(!plan.derived.contains_key("dr.spacer_right"));
    assert!(!plan.diagnostics.items.iter().any(|d| d.code == "FAB-101"));
}

#[test]
fn inner_drawers_in_the_plane_of_an_inset_door_get_a_setback_fix() {
    let flush = WARDROBE.replace(r#", "setback": 20"#, "");
    let plan = rewood_core::compile_json(&flush);
    let d = plan
        .diagnostics
        .items
        .iter()
        .find(|d| d.code == "SPEC-214")
        .unwrap();
    let fix = d.fix.as_ref().unwrap();
    assert_eq!(fix.field, "setback");
    assert_eq!(fix.value, serde_json::json!(20.0));
    let mut v: serde_json::Value = serde_json::from_str(&flush).unwrap();
    rewood_core::spec::apply_fix(&mut v, fix).unwrap();
    let after = rewood_core::compile_json(&v.to_string());
    assert!(
        after.diagnostics.items.is_empty(),
        "{:#?}",
        after.diagnostics
    );
}

#[test]
fn the_hinge_names_a_family_and_the_door_picks_the_arm() {
    // A hand-picked overlay hinge on an inset door: the inset variant of
    // the same family takes its place, keeping the placement rule.
    let explicit = WARDROBE.replace(
        r#""count": 1, "mount": "inset" }"#,
        r#""count": 1, "mount": "inset", "hinge": { "hardware": ["hinge_35_overlay"], "placement": { "endOffset": 100, "maxSpacing": 700 } } }"#,
    );
    let plan = rewood_core::compile_json(&explicit);
    assert!(plan.joints.iter().any(|j| j.hardware == ["hinge_35_inset"]));
    // And an inset hinge on an overlay door becomes the overlay one.
    let inset_on_overlay = WARDROBE.replace(
        r#""count": 1, "mount": "inset" }"#,
        r#""count": 1, "hinge": { "hardware": ["hinge_35_inset"] } }"#,
    );
    let plan = rewood_core::compile_json(&inset_on_overlay);
    assert!(plan
        .joints
        .iter()
        .any(|j| j.hardware == ["hinge_35_overlay"]));
    assert!(!plan.diagnostics.items.iter().any(|d| d.code == "SPEC-314"));
    // A family without the needed arm is refused: the 165° hinge has no
    // inset variant.
    let wide_on_inset = WARDROBE.replace(
        r#""count": 1, "mount": "inset" }"#,
        r#""count": 1, "mount": "inset", "hinge": { "hardware": ["hinge_35_overlay_165"] } }"#,
    );
    let plan = rewood_core::compile_json(&wide_on_inset);
    assert!(
        plan.diagnostics.items.iter().any(|d| d.code == "SPEC-314"),
        "{:#?}",
        plan.diagnostics
    );
    // `softClose` swaps the damped variant in, on doors and drawers.
    let soft = WARDROBE
        .replace(
            r#""count": 1, "mount": "inset" }"#,
            r#""count": 1, "mount": "inset", "softClose": true }"#,
        )
        .replace(
            r#""slide": { "hardware": ["slide_ball_450"] }"#,
            r#""slide": { "hardware": ["slide_ball_450"] }, "softClose": true"#,
        );
    let plan = rewood_core::compile_json(&soft);
    assert!(
        plan.joints
            .iter()
            .any(|j| j.hardware == ["hinge_35_inset_soft"]),
        "{:#?}",
        plan.diagnostics
    );
    assert!(
        plan.joints
            .iter()
            .any(|j| j.hardware == ["slide_ball_soft_450"]),
        "{:#?}",
        plan.diagnostics
    );
}

#[test]
fn a_door_set_can_span_two_bays_when_overlaid() {
    let wide = WARDROBE.replace(
        r#""bay": 1, "count": 1, "mount": "inset" }"#,
        r#""bay": 1, "span": 2, "count": 2 }"#,
    );
    let plan = rewood_core::compile_json(&wide);
    assert_eq!(plan.status, PlanStatus::Ok, "{:#?}", plan.diagnostics);
    let d1 = part(&plan, "door_1");
    let d2 = part(&plan, "door_2");
    // Each door covers half the carcass front, meeting over the divider.
    assert_eq!(d1.aabb.min.0, 2.0);
    assert_eq!(d2.aabb.max.0, 1198.0);
    assert_eq!(d1.aabb.max.0, 599.0);
    let hinges: Vec<&str> = plan
        .joints
        .iter()
        .filter(|j| j.kind == "hinge")
        .map(|j| j.face_part.as_str())
        .collect();
    assert_eq!(
        hinges,
        [
            part(&plan, "side_left").id.as_str(),
            part(&plan, "side_right").id.as_str()
        ]
    );
    // Both bays count as covered.
    assert!(!plan.diagnostics.items.iter().any(|d| d.code == "SPEC-212"));

    let inset = WARDROBE.replace(
        r#""bay": 1, "count": 1, "mount": "inset" }"#,
        r#""bay": 1, "span": 2, "count": 2, "mount": "inset" }"#,
    );
    let plan = rewood_core::compile_json(&inset);
    assert!(plan.diagnostics.items.iter().any(|d| d.code == "SPEC-317"));
    let too_far = WARDROBE.replace(
        r#""bay": 1, "count": 1, "mount": "inset" }"#,
        r#""bay": 2, "span": 2, "count": 2 }"#,
    );
    let plan = rewood_core::compile_json(&too_far);
    assert!(plan.diagnostics.items.iter().any(|d| d.code == "SPEC-204"));
}

/// Two single-door cabinets side by side, and a drawer stack between.
fn run(middle: &str, right_door: &str) -> String {
    format!(
        r#"{{
  "schemaVersion": "1.0", "id": "r", "name": "r",
  "parameters": {{ "height": 720, "depth": 560 }},
  "material": "melamine_18", "edgeMaterial": "abs_1mm",
  "components": [
    {{ "type": "carcass", "id": "a", "width": 500, "joint": {{ "hardware": ["dowel_8x30"] }}, "back": {{ "material": "hdf_3" }} }},
    {{ "type": "carcass", "id": "b", "width": 500, "origin": {{ "x": 500 }}, "joint": {{ "hardware": ["dowel_8x30"] }}, "back": {{ "material": "hdf_3" }} }},
    {{ "type": "carcass", "id": "c", "width": 500, "origin": {{ "x": 1000 }}, "joint": {{ "hardware": ["dowel_8x30"] }}, "back": {{ "material": "hdf_3" }} }},
    {{ "type": "doors", "id": "da", "carcass": "a", "count": 1 }},
    {middle},
    {{ "type": "doors", "id": "dc", "carcass": "c", "count": 1{right_door} }}
  ]
}}"#
    )
}

const DRAWERS_B: &str = r#"{ "type": "drawers", "id": "drb", "carcass": "b", "count": 3, "slide": { "hardware": ["slide_ball_450"] }, "joint": { "hardware": ["dowel_8x30"] } }"#;

fn hinge_side(plan: &rewood_core::plan::ManufacturingPlan, component: &str) -> &'static str {
    let j = plan
        .joints
        .iter()
        .find(|j| j.kind == "hinge" && j.component == component)
        .unwrap();
    let door = plan.parts.iter().find(|p| p.id == j.edge_part).unwrap();
    let panel = plan.parts.iter().find(|p| p.id == j.face_part).unwrap();
    if panel.aabb.min.0 > door.aabb.min.0 + 1.0 {
        "right"
    } else {
        "left"
    }
}

#[test]
fn single_doors_hinge_away_from_the_middle_and_a_crossing_is_reported() {
    // Auto: the left door on its left side, the right one on its right,
    // both opening away from the drawers in the middle.
    let plan = rewood_core::compile_json(&run(DRAWERS_B, ""));
    assert_eq!(plan.status, PlanStatus::Ok, "{:#?}", plan.diagnostics);
    assert_eq!(hinge_side(&plan, "da"), "left");
    assert_eq!(hinge_side(&plan, "dc"), "right");
    // Forced towards the drawers: it hits them open, and the fix is the
    // other side.
    let plan = rewood_core::compile_json(&run(DRAWERS_B, r#", "hingeSide": "left""#));
    assert_eq!(hinge_side(&plan, "dc"), "left");
    let hit: Vec<_> = plan
        .diagnostics
        .items
        .iter()
        .filter(|d| d.code == "DESIGN-118")
        .collect();
    assert_eq!(hit.len(), 1, "{hit:#?}");
    let fix = hit[0].fix.as_ref().unwrap();
    assert_eq!(
        (fix.field.as_str(), &fix.value),
        ("hingeSide", &serde_json::json!("right"))
    );
}

#[test]
fn fronts_meeting_inside_a_carcass_keep_one_gap_and_front_height_occupies_what_it_covers() {
    let json = r#"{
  "schemaVersion": "1.0", "id": "g", "name": "g",
  "parameters": { "width": 600, "height": 900, "depth": 560 },
  "material": "melamine_18", "edgeMaterial": "abs_1mm",
  "components": [
    { "type": "carcass", "id": "c", "joint": { "hardware": ["dowel_8x30"] }, "back": { "material": "hdf_3" } },
    { "type": "drawers", "id": "d", "count": 2, "zone": { "from": 0, "to": 400 }, "slide": { "hardware": ["slide_ball_450"] }, "joint": { "hardware": ["dowel_8x30"] } },
    { "type": "doors", "id": "o", "count": 1, "zone": { "from": 400, "to": 900 } }
  ]
}"#;
    let plan = rewood_core::compile_json(json);
    assert_eq!(plan.status, PlanStatus::Ok, "{:#?}", plan.diagnostics);
    let top_drawer = part(&plan, "drawer_2_front");
    let door = part(&plan, "door_1");
    assert!((door.aabb.min.2 - top_drawer.aabb.max.2 - 2.0).abs() < 1e-9);
    // With `frontHeight` the drawers cover less than their zone: the rest
    // is open, and said.
    let short = json.replace(
        r#""count": 2, "zone""#,
        r#""count": 2, "frontHeight": 150, "zone""#,
    );
    let plan = rewood_core::compile_json(&short);
    assert!(
        plan.diagnostics.items.iter().any(|d| d.code == "SPEC-212"),
        "{:#?}",
        plan.diagnostics
    );
}

#[test]
fn a_turned_carcass_is_the_same_furniture_turned() {
    let spec = |rotation: u32| {
        format!(
            r#"{{
  "schemaVersion": "1.0", "id": "t", "name": "t",
  "parameters": {{ "width": 600, "height": 720, "depth": 560 }},
  "material": "melamine_18", "edgeMaterial": "abs_1mm",
  "components": [
    {{ "type": "carcass", "id": "c", "bays": 2, "origin": {{ "x": 100, "y": 900, "rotation": {rotation} }}, "joint": {{ "hardware": ["dowel_8x30"] }}, "back": {{ "material": "hdf_3" }}, "legs": {{ "plinth": {{}} }} }},
    {{ "type": "drawers", "id": "d", "bay": 1, "count": 2, "slide": {{ "hardware": ["slide_ball_450"] }}, "joint": {{ "hardware": ["dowel_8x30"] }}, "handle": {{ "hardware": ["handle_bar_128"] }} }},
    {{ "type": "doors", "id": "o", "bay": 2, "count": 1, "catch": {{}} }}
  ]
}}"#
        )
    };
    let flat = rewood_core::compile_json(&spec(0));
    assert_eq!(flat.status, PlanStatus::Ok, "{:#?}", flat.diagnostics);
    let pivot = rewood_core::geometry::Vec3(100.0, 900.0, 0.0);
    for (deg, q) in [(90, 1), (180, 2), (270, 3)] {
        let turned = rewood_core::compile_json(&spec(deg));
        assert_eq!(
            turned.status,
            PlanStatus::Ok,
            "{deg}: {:#?}",
            turned.diagnostics
        );
        assert_eq!(turned.parts.len(), flat.parts.len());
        for (a, b) in flat.parts.iter().zip(&turned.parts) {
            assert_eq!(a.role, b.role);
            assert_eq!(a.operations, b.operations, "{deg}: {}", a.role);
            assert_eq!(a.aabb.turned_about(pivot, q), b.aabb, "{deg}: {}", a.role);
        }
        for (a, b) in flat.joints.iter().zip(&turned.joints) {
            assert_eq!(a.fasteners.len(), b.fasteners.len());
            for (fa, fb) in a.fasteners.iter().zip(&b.fasteners) {
                assert_eq!(fa.position.turned_about(pivot, q), fb.position);
            }
        }
        assert_eq!(turned.derived["c.rotation"], f64::from(deg));
    }
    let odd = rewood_core::compile_json(&spec(45));
    assert!(odd.diagnostics.items.iter().any(|d| d.code == "SPEC-332"));
}

#[test]
fn a_fixed_front_is_dowelled_to_the_edges_it_covers() {
    let plan = rewood_core::compile_json(
        r#"{
  "schemaVersion": "1.0", "id": "f", "name": "f",
  "parameters": { "width": 1000, "height": 720, "depth": 560 },
  "material": "melamine_18", "edgeMaterial": "abs_1mm",
  "components": [
    { "type": "carcass", "id": "c", "bays": 2, "bayWidths": [600, "auto"], "joint": { "hardware": ["dowel_8x30"] }, "back": { "material": "hdf_3" } },
    { "type": "doors", "id": "blind", "bay": 1, "count": 1, "fixed": true },
    { "type": "doors", "id": "door", "bay": 2, "count": 1 }
  ]
}"#,
    );
    assert_eq!(plan.status, PlanStatus::Ok, "{:#?}", plan.diagnostics);
    let front = part(&plan, "fixed_front_1");
    assert_eq!(front.name, "Frente fijo 1");
    let joints: Vec<_> = plan
        .joints
        .iter()
        .filter(|j| j.component == "blind")
        .collect();
    // The left side, the top and the bottom; not the divider, covered only
    // to its middle. No hinge.
    let roles: Vec<&str> = joints
        .iter()
        .map(|j| {
            let other = if j.face_part == front.id {
                &j.edge_part
            } else {
                &j.face_part
            };
            plan.parts
                .iter()
                .find(|p| &p.id == other)
                .unwrap()
                .role
                .as_str()
        })
        .collect();
    assert_eq!(roles, ["side_left", "top", "bottom"]);
    assert!(joints
        .iter()
        .all(|j| j.kind == "butt" && j.hardware == ["dowel_8x30"]));
    let handled = rewood_core::compile_json(&serde_json::json!({
        "schemaVersion": "1.0", "id": "f", "name": "f",
        "parameters": { "width": 600, "height": 720, "depth": 560 },
        "material": "melamine_18",
        "components": [
            { "type": "carcass", "id": "c", "joint": { "hardware": ["dowel_8x30"] } },
            { "type": "doors", "id": "blind", "count": 1, "fixed": true, "handle": { "hardware": ["handle_bar_128"] } }
        ]
    }).to_string());
    assert!(handled
        .diagnostics
        .items
        .iter()
        .any(|d| d.code == "SPEC-334"));
}

#[test]
fn sliding_doors_share_the_opening_on_two_lanes_and_keep_the_interior_behind() {
    let spec = r#"{
  "schemaVersion": "1.0", "id": "s", "name": "s",
  "parameters": { "width": 1800, "height": 2100, "depth": 600 },
  "material": "melamine_18", "edgeMaterial": "abs_1mm",
  "components": [
    { "type": "carcass", "id": "c", "bays": 2, "dividerSetback": 60, "joint": { "hardware": ["dowel_8x30"] }, "back": { "material": "hdf_3" } },
    { "type": "shelves", "id": "sh", "bay": 2, "count": 3, "setback": 65, "joint": { "hardware": ["dowel_8x30"] } },
    { "type": "sliding_doors", "id": "d", "count": 2 }
  ]
}"#;
    let plan = rewood_core::compile_json(spec);
    assert_eq!(plan.status, PlanStatus::Ok, "{:#?}", plan.diagnostics);
    let a = part(&plan, "sliding_door_1");
    let b = part(&plan, "sliding_door_2");
    // The 2 m kit: each door half the 1764 opening less 7; the kit's
    // profiles cover the 14 between them.
    assert_eq!(a.dims.width, 875.0);
    assert_eq!(b.aabb.min.0 - a.aabb.max.0, 14.0);
    // 46 mm shorter than the opening, as the kit asks.
    assert_eq!(a.dims.length, 2100.0 - 36.0 - 46.0);
    // The first in the back lane, the second one lane (25) in front.
    assert_eq!(b.aabb.max.1 - a.aabb.max.1, 25.0);
    assert!(b.aabb.max.1 < 600.0);
    // Two rollers and two guides a door, the track by the metre.
    let fittings = plan
        .joints
        .iter()
        .filter(|j| j.kind == "fixture" && j.component == "d")
        .count();
    assert_eq!(fittings, 8);
    let track = plan
        .bom
        .hardware
        .iter()
        .find(|h| h.hardware == "sliding_kit_2m")
        .unwrap();
    assert_eq!(track.items[0].quantity, 1.0);

    // Dividers up to the front: the track does not fit, and the fix says
    // how far to take them back.
    let flush = spec.replace(r#""dividerSetback": 60, "#, "");
    let plan = rewood_core::compile_json(&flush);
    let d = plan
        .diagnostics
        .items
        .iter()
        .find(|d| d.code == "SPEC-335")
        .unwrap();
    assert_eq!(d.fix.as_ref().unwrap().field, "dividerSetback");
    // Shelves at their usual 20 reach into the doors' plane.
    let shallow = spec.replace(r#""setback": 65, "#, "");
    let plan = rewood_core::compile_json(&shallow);
    let d = plan
        .diagnostics
        .items
        .iter()
        .find(|d| d.code == "SPEC-215")
        .unwrap();
    assert_eq!(d.fix.as_ref().unwrap().value, serde_json::json!(54.0));
}
