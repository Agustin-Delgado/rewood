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
    assert_eq!(plan.status, PlanStatus::Ok, "{:#?}", plan.diagnostics);
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
    assert_eq!(front.aabb.min.0, 20.0);
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
    assert_eq!(after.status, PlanStatus::Ok, "{:#?}", after.diagnostics);
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
    let wide = WARDROBE
        .replace(
            r#""bay": 1, "count": 1, "mount": "inset" }"#,
            r#""bay": 1, "span": 2, "count": 2 }"#,
        )
        .replace(r#", "setback": 20"#, "");
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
