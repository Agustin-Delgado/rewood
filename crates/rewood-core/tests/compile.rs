//! End-to-end behaviour of the compiler that a fixture cannot express:
//! determinism across runs, propagation of a parameter change, and the
//! failure modes that must never panic.

use rewood_core::diagnostics::Severity;
use rewood_core::model::OpGeometry;
use rewood_core::plan::PlanStatus;
use rewood_core::spec::FurnitureSpec;

const BASIC: &str = include_str!("../../../fixtures/basic_cabinet/input.json");

fn spec_with_width(width: f64) -> FurnitureSpec {
    let mut spec = FurnitureSpec::from_json(BASIC).unwrap();
    spec.parameters.insert(
        "width".into(),
        rewood_core::params::ParamInput::Number(width),
    );
    spec
}

#[test]
fn same_input_same_output() {
    let a = rewood_core::compile_json(BASIC).to_json_pretty();
    let b = rewood_core::compile_json(BASIC).to_json_pretty();
    assert_eq!(a, b);
}

#[test]
fn basic_cabinet_is_manufacturable() {
    let plan = rewood_core::compile_json(BASIC);
    assert_eq!(plan.status, PlanStatus::Ok, "{:#?}", plan.diagnostics);
    assert!(!plan.manufacturing_blocked);
    assert_eq!(plan.parts.len(), 9);
    let holes: usize = plan
        .parts
        .iter()
        .flat_map(|p| &p.operations)
        .filter(|op| matches!(op.geometry, OpGeometry::Drill { .. }))
        .count();
    // 4 carcass joints × (2 minifix × 3 holes + 2 dowels × 2 holes)
    // + 4 shelf joints × 2 dowels × 2 holes
    // + 2 doors × 2 hinges × (3 holes on the door + 2 on the side).
    assert_eq!(holes, 4 * (6 + 4) + 4 * 4 + 2 * 2 * 5);
    assert_eq!(plan.joints.len(), 10);
    let hinges = plan
        .bom
        .hardware
        .iter()
        .find(|h| h.hardware == "hinge_35_overlay")
        .unwrap();
    assert_eq!(hinges.quantity, 4);
    let minifix = plan
        .bom
        .hardware
        .iter()
        .find(|h| h.hardware == "minifix_15")
        .unwrap();
    assert_eq!(minifix.quantity, 8);
}

#[test]
fn widening_the_cabinet_adds_fasteners_and_grows_parts() {
    let narrow = rewood_core::compile(&spec_with_width(1000.0));
    let wide = rewood_core::compile(&spec_with_width(1800.0));

    let top = |plan: &rewood_core::plan::ManufacturingPlan| {
        plan.parts.iter().find(|p| p.role == "top").unwrap().clone()
    };
    assert_eq!(top(&narrow).dims.length, 964.0);
    assert_eq!(top(&wide).dims.length, 1764.0);

    // Top-to-side joints run along the depth (400) so their fastener count
    // does not change; but the doors and shelves follow the width.
    let doors = |plan: &rewood_core::plan::ManufacturingPlan| plan.derived["doors.door_width"];
    assert_eq!(doors(&narrow), 497.0);
    assert_eq!(doors(&wide), 897.0);

    // And the constraint on shelf span now fails.
    assert!(wide
        .diagnostics
        .items
        .iter()
        .any(|d| d.code == "CON-001" && d.entity.as_deref() == Some("shelf_span")));
    assert_eq!(wide.status, PlanStatus::Errors);
    assert!(!wide.manufacturing_blocked);
}

#[test]
fn a_longer_joint_gets_more_fasteners() {
    // Deeper cabinet: the top-to-side joint line is the depth.
    let mut spec = FurnitureSpec::from_json(BASIC).unwrap();
    spec.parameters.insert(
        "depth".into(),
        rewood_core::params::ParamInput::Number(900.0),
    );
    let plan = rewood_core::compile(&spec);
    let j = &plan.joints[0];
    assert_eq!(j.length, 900.0);
    let minifix = j
        .fasteners
        .iter()
        .filter(|f| f.hardware == "minifix_15")
        .count();
    let dowels = j
        .fasteners
        .iter()
        .filter(|f| f.hardware == "dowel_8x30")
        .count();
    assert_eq!(minifix, 3); // 50, 450, 850
    assert_eq!(dowels, 4); // 120 .. 780 at ≤ 300
}

#[test]
fn parameter_cycle_blocks_without_panicking() {
    let mut spec = FurnitureSpec::from_json(BASIC).unwrap();
    spec.parameters.insert(
        "width".into(),
        rewood_core::params::ParamInput::Expr("height + 1".into()),
    );
    spec.parameters.insert(
        "height".into(),
        rewood_core::params::ParamInput::Expr("width + 1".into()),
    );
    let plan = rewood_core::compile(&spec);
    assert!(plan.manufacturing_blocked);
    assert_eq!(plan.diagnostics.items[0].code, "PARAM-001");
    assert!(plan.parts.is_empty());
}

#[test]
fn malformed_json_is_a_fatal_diagnostic() {
    let plan = rewood_core::compile_json("{ not json");
    assert!(plan.manufacturing_blocked);
    assert_eq!(plan.diagnostics.items[0].code, "SPEC-000");
    assert_eq!(plan.diagnostics.items[0].severity, Severity::Fatal);
}

#[test]
fn unknown_field_is_rejected() {
    let plan = rewood_core::compile_json(
        r#"{"schemaVersion":"1.0","id":"x","name":"x","material":"melamine_18","components":[],"colour":"red"}"#,
    );
    assert_eq!(plan.diagnostics.items[0].code, "SPEC-000");
    assert!(plan.diagnostics.items[0].message.contains("colour"));
}

#[test]
fn unknown_schema_version_is_fatal() {
    let plan = rewood_core::compile_json(
        r#"{"schemaVersion":"9.0","id":"x","name":"x","material":"melamine_18","components":[]}"#,
    );
    assert_eq!(plan.diagnostics.items[0].code, "SPEC-001");
}

#[test]
fn shelves_without_a_carcass_is_fatal() {
    let plan = rewood_core::compile_json(
        r#"{"schemaVersion":"1.0","id":"x","name":"x","material":"melamine_18",
            "components":[{"type":"shelves","id":"s","count":2,"joint":{"hardware":["dowel_8x30"]}}]}"#,
    );
    assert_eq!(plan.diagnostics.items[0].code, "SPEC-202");
    assert!(plan.manufacturing_blocked);
}

#[test]
fn a_single_door_hangs_on_the_left_and_a_tall_one_gets_three_hinges() {
    let mut spec = FurnitureSpec::from_json(BASIC).unwrap();
    spec.parameters.insert(
        "width".into(),
        rewood_core::params::ParamInput::Number(500.0),
    );
    spec.parameters.insert(
        "height".into(),
        rewood_core::params::ParamInput::Number(1400.0),
    );
    let plan = rewood_core::compile(&spec);
    assert_eq!(plan.derived["doors.count"], 1.0);
    let hinge = plan.joints.iter().find(|j| j.kind == "hinge").unwrap();
    assert_eq!(
        hinge.face_part, "P001",
        "single door hangs on the left side"
    );
    assert_eq!(hinge.fasteners.len(), 3);
    let door = plan.parts.iter().find(|p| p.role == "door_1").unwrap();
    let cups: Vec<f64> = door
        .operations
        .iter()
        .filter_map(|op| match &op.geometry {
            OpGeometry::Drill { u, diameter, .. } if *diameter == 35.0 => Some(*u),
            _ => None,
        })
        .collect();
    assert_eq!(cups, vec![100.0, 698.0, 1296.0]);
}

#[test]
fn three_doors_without_dividers_is_fatal_unless_hinges_are_off() {
    let mut spec = FurnitureSpec::from_json(BASIC).unwrap();
    spec.parameters.insert(
        "door_count".into(),
        rewood_core::params::ParamInput::Number(3.0),
    );
    let plan = rewood_core::compile(&spec);
    assert!(plan.diagnostics.items.iter().any(|d| d.code == "SPEC-306"));
    assert!(plan.manufacturing_blocked);

    let json = BASIC.replace(
        r#""count": "door_count""#,
        r#""count": "door_count", "hinge": null"#,
    );
    let mut spec = FurnitureSpec::from_json(&json).unwrap();
    spec.parameters.insert(
        "door_count".into(),
        rewood_core::params::ParamInput::Number(3.0),
    );
    let plan = rewood_core::compile(&spec);
    assert!(!plan.manufacturing_blocked, "{:#?}", plan.diagnostics);
    assert_eq!(
        plan.parts.iter().filter(|p| p.component == "doors").count(),
        3
    );
    assert!(plan.joints.iter().all(|j| j.kind == "butt"));
}

#[test]
fn a_butt_fastener_cannot_be_used_as_a_hinge() {
    let json = BASIC.replace(
        r#""count": "door_count""#,
        r#""count": "door_count", "hinge": { "hardware": ["dowel_8x30"] }"#,
    );
    let plan = rewood_core::compile_json(&json);
    assert!(plan.diagnostics.items.iter().any(|d| d.code == "JOINT-104"));
}

#[test]
fn edge_banding_is_configurable_per_component() {
    let json = BASIC
        .replace(r#""setback": 20,"#, r#""setback": 20, "edges": "all","#)
        .replace(
            r#""count": "door_count""#,
            r#""count": "door_count", "edges": "none""#,
        );
    let plan = rewood_core::compile_json(&json);
    assert!(!plan.manufacturing_blocked, "{:#?}", plan.diagnostics);
    let shelf = plan.parts.iter().find(|p| p.role == "shelf_1").unwrap();
    assert_eq!(shelf.edges.len(), 4);
    // 1 mm band on every edge shrinks the raw cut by 2 mm each way.
    assert_eq!(shelf.cut.length, shelf.dims.length - 2.0);
    assert_eq!(shelf.cut.width, shelf.dims.width - 2.0);
    let door = plan.parts.iter().find(|p| p.role == "door_1").unwrap();
    assert!(door.edges.is_empty());
    assert_eq!(door.cut.length, door.dims.length);
}

const DRAWERS: &str = include_str!("../../../fixtures/drawer_unit/input.json");

#[test]
fn drawer_unit_is_manufacturable_and_consistent() {
    let plan = rewood_core::compile_json(DRAWERS);
    assert!(!plan.manufacturing_blocked, "{:#?}", plan.diagnostics);
    // 5 carcass parts + plinth + 3 × (front, 2 sides, box front, box back, bottom).
    assert_eq!(plan.parts.len(), 5 + 1 + 3 * 6);
    let slides = plan.joints.iter().filter(|j| j.kind == "slide").count();
    assert_eq!(slides, 6);
    let slide_line = plan
        .bom
        .hardware
        .iter()
        .find(|h| h.hardware == "slide_ball_450")
        .unwrap();
    assert_eq!(slide_line.quantity, 6);

    // Box depth follows the slide length; box width leaves the slide clearance.
    assert_eq!(plan.derived["drawers.box_depth"], 450.0);
    assert_eq!(plan.derived["drawers.box_width"], 564.0 - 2.0 * 12.7);

    // Slide holes on the carcass side sit 37/165/293 from its front edge
    // (v = 0 is the front on the left side) at the box bottom + 22.5.
    let side = plan.parts.iter().find(|p| p.role == "side_left").unwrap();
    let first_slide = plan
        .joints
        .iter()
        .find(|j| j.kind == "slide" && j.face_part == side.id)
        .unwrap()
        .id
        .clone();
    let slide_holes: Vec<(f64, f64)> = side
        .operations
        .iter()
        .filter(|op| {
            op.source
                .as_ref()
                .is_some_and(|s| s.hardware == "slide_ball_450" && s.joint == first_slide)
        })
        .filter_map(|op| match op.geometry {
            OpGeometry::Drill { u, v, .. } => Some((u, v)),
            _ => None,
        })
        .collect();
    assert_eq!(
        slide_holes,
        vec![(44.5, 37.0), (44.5, 165.0), (44.5, 293.0)]
    );

    // The three drawers' boxes are interchangeable parts on the cut list.
    let left_sides = plan
        .part_list
        .iter()
        .find(|r| r.name == "Lateral cajón 1 izq.")
        .unwrap();
    assert_eq!(left_sides.quantity, 3);
}

#[test]
fn a_slide_longer_than_the_carcass_is_fatal() {
    let mut spec = FurnitureSpec::from_json(DRAWERS).unwrap();
    spec.parameters.insert(
        "depth".into(),
        rewood_core::params::ParamInput::Number(400.0),
    );
    let plan = rewood_core::compile(&spec);
    assert!(plan.diagnostics.items.iter().any(|d| d.code == "SPEC-307"));
    assert!(plan.manufacturing_blocked);
}

#[test]
fn placement_override_changes_fastener_count() {
    let mut spec = FurnitureSpec::from_json(DRAWERS).unwrap();
    for c in &mut spec.components {
        if let rewood_core::spec::ComponentSpec::Drawers { joint, .. } = c {
            joint.placement = Some(rewood_core::library::hardware::PlacementRule {
                end_offset: 30.0,
                max_spacing: 60.0,
                count_by_length: vec![],
                fixed: vec![],
            });
        }
    }
    let plan = rewood_core::compile(&spec);
    let box_joint = plan
        .joints
        .iter()
        .find(|j| j.component == "drawers" && j.kind == "butt")
        .unwrap();
    // 190.667 long: usable 130.667 / 60 -> 3 gaps -> 4 dowels.
    assert_eq!(box_joint.fasteners.len(), 4);
}

const WARDROBE: &str = include_str!("../../../fixtures/wardrobe_1800/input.json");

/// The technical success criterion of the specification (§55): a
/// 1800×2100×500 wardrobe with 3 bays, 2 doors, 4 shelves and 3 drawers
/// compiles to a complete, valid package without human intervention.
#[test]
fn wardrobe_success_criterion() {
    let plan = rewood_core::compile_json(WARDROBE);
    assert!(!plan.manufacturing_blocked, "{:#?}", plan.diagnostics);
    assert_eq!(plan.diagnostics.count(Severity::Error), 0);
    assert_eq!(plan.derived["carcass.bays"], 3.0);
    assert_eq!(plan.derived["carcass.bay_width"], 576.0);

    let roles = |prefix: &str| {
        plan.parts
            .iter()
            .filter(|p| p.role.starts_with(prefix))
            .count()
    };
    assert_eq!(roles("divider_"), 2);
    assert_eq!(
        plan.parts
            .iter()
            .filter(|p| p.component.starts_with("shelves"))
            .count(),
        4
    );
    assert_eq!(roles("drawer_"), 3 * 6);
    assert_eq!(
        plan.parts
            .iter()
            .filter(|p| p.component.starts_with("door"))
            .count(),
        2
    );

    // Every part has a placement, a cut size and every hole is inside its face.
    for p in &plan.parts {
        assert!(p.cut.length > 0.0 && p.cut.width > 0.0, "{}", p.id);
    }
    // The middle door hangs on divider 1, the left door on the left side.
    let hinges: Vec<(&str, &str)> = plan
        .joints
        .iter()
        .filter(|j| j.kind == "hinge")
        .map(|j| (j.component.as_str(), j.face_part.as_str()))
        .collect();
    let divider1 = &plan
        .parts
        .iter()
        .find(|p| p.role == "divider_1")
        .unwrap()
        .id;
    assert_eq!(
        hinges,
        vec![
            ("door_left", "P001"),
            ("door_middle_top", divider1.as_str())
        ]
    );
    // Drawers in bay 2 run on divider 1 and divider 2.
    let slides: std::collections::BTreeSet<&str> = plan
        .joints
        .iter()
        .filter(|j| j.kind == "slide")
        .map(|j| j.face_part.as_str())
        .collect();
    assert_eq!(slides.len(), 2);
    assert!(slides.contains(divider1.as_str()));
}

#[test]
fn bay_and_zone_out_of_range_are_fatal() {
    let json = WARDROBE.replace(
        r#""bay": 1,
      "count": 2"#,
        r#""bay": 4,
      "count": 2"#,
    );
    assert_ne!(json, WARDROBE);
    let plan = rewood_core::compile_json(&json);
    assert!(plan.diagnostics.items.iter().any(|d| d.code == "SPEC-204"));

    let mut spec = FurnitureSpec::from_json(WARDROBE).unwrap();
    spec.parameters.insert(
        "drawer_zone_top".into(),
        rewood_core::params::ParamInput::Number(2500.0),
    );
    let plan = rewood_core::compile(&spec);
    assert!(
        plan.diagnostics.items.iter().any(|d| d.code == "SPEC-205"),
        "{:#?}",
        plan.diagnostics
    );
}

#[test]
fn drawers_and_shelves_in_the_same_zone_collide_and_are_reported() {
    let mut spec = FurnitureSpec::from_json(WARDROBE).unwrap();
    for c in &mut spec.components {
        if let rewood_core::spec::ComponentSpec::Shelves { id, zone, .. } = c {
            if id == "shelves_middle" {
                *zone = None;
            }
        }
    }
    let plan = rewood_core::compile(&spec);
    assert!(plan.diagnostics.items.iter().any(|d| d.code == "FAB-101"));
    assert_eq!(plan.status, PlanStatus::Errors);
}

#[test]
fn drawer_fronts_are_screwed_to_the_box_and_handles_are_drilled_through() {
    let plan = rewood_core::compile_json(WARDROBE);
    let fixing = plan
        .joints
        .iter()
        .find(|j| j.kind == "face_to_face")
        .unwrap();
    // 514.6 mm wide contact: 3 columns × 2 rows of screws.
    assert_eq!(fixing.fasteners.len(), 6);
    let box_front = plan
        .parts
        .iter()
        .find(|p| p.id == fixing.edge_part)
        .unwrap();
    let through = box_front
        .operations
        .iter()
        .filter(|op| matches!(op.geometry, OpGeometry::Drill { through: true, .. }))
        .count();
    assert_eq!(through, 6);
    let front = plan
        .parts
        .iter()
        .find(|p| p.id == fixing.face_part)
        .unwrap();
    assert_eq!(front.role, "drawer_1_front");

    // The door handle sits 40 mm from the opening edge, centred on the height.
    let handle = plan
        .joints
        .iter()
        .find(|j| j.kind == "handle" && j.component == "door_left")
        .unwrap();
    assert_eq!(
        handle.fasteners[0].position,
        rewood_core::geometry::Vec3(561.0, 500.0, 1050.0)
    );
    let door = plan
        .parts
        .iter()
        .find(|p| p.component == "door_left")
        .unwrap();
    let holes: Vec<(f64, f64)> = door
        .operations
        .iter()
        .filter(|op| {
            op.source
                .as_ref()
                .is_some_and(|s| s.hardware == "handle_bar_128")
        })
        .filter_map(|op| match op.geometry {
            OpGeometry::Drill {
                u,
                v,
                through: true,
                ..
            } => Some((u, v)),
            _ => None,
        })
        .collect();
    assert_eq!(holes, vec![(1112.0, 40.0), (984.0, 40.0)]);
    let handles = plan
        .bom
        .hardware
        .iter()
        .find(|h| h.hardware == "handle_bar_128")
        .unwrap();
    assert_eq!(handles.quantity, 5);
}

#[test]
fn a_handle_needs_handle_hardware() {
    let json = WARDROBE.replace(
        r#""handle": {
        "hardware": [
          "handle_bar_128"
        ]
      }"#,
        r#""handle": { "hardware": ["dowel_8x30"] }"#,
    );
    assert_ne!(json, WARDROBE);
    let plan = rewood_core::compile_json(&json);
    assert!(plan.diagnostics.items.iter().any(|d| d.code == "JOINT-104"));
}

#[test]
fn explicit_bay_widths_with_auto_and_the_sum_check() {
    let mut spec = FurnitureSpec::from_json(WARDROBE).unwrap();
    let set = |spec: &mut FurnitureSpec, widths: Vec<rewood_core::params::ParamInput>| {
        for c in &mut spec.components {
            if let rewood_core::spec::ComponentSpec::Carcass { bay_widths, .. } = c {
                *bay_widths = widths.clone();
            }
        }
    };
    use rewood_core::params::ParamInput::{Expr, Number};
    // 1764 inner − 2 dividers × 18 = 1728 to share.
    set(
        &mut spec,
        vec![Number(500.0), Expr("auto".into()), Number(400.0)],
    );
    let plan = rewood_core::compile(&spec);
    assert!(!plan.manufacturing_blocked, "{:#?}", plan.diagnostics);
    assert_eq!(plan.derived["carcass.bay_1_width"], 500.0);
    assert_eq!(plan.derived["carcass.bay_2_width"], 828.0);
    assert_eq!(plan.derived["carcass.bay_3_width"], 400.0);
    let shelf = plan
        .parts
        .iter()
        .find(|p| p.role == "bay1_shelf_1" || p.component == "shelves_left")
        .unwrap();
    assert_eq!(shelf.dims.length, 500.0);

    // Widths that do not add up, without an auto, are fatal.
    set(&mut spec, vec![Number(500.0), Number(500.0), Number(500.0)]);
    let plan = rewood_core::compile(&spec);
    assert!(plan.diagnostics.items.iter().any(|d| d.code == "SPEC-309"));

    // Exact sum without auto is fine.
    set(&mut spec, vec![Number(500.0), Number(828.0), Number(400.0)]);
    let plan = rewood_core::compile(&spec);
    assert!(!plan.manufacturing_blocked, "{:#?}", plan.diagnostics);
}

const BOOKCASE: &str = include_str!("../../../fixtures/bookcase_fixed/input.json");

#[test]
fn fixed_shelves_take_the_full_depth_and_split_the_zones() {
    let plan = rewood_core::compile_json(BOOKCASE);
    assert_eq!(plan.status, PlanStatus::Ok, "{:#?}", plan.diagnostics);
    let fixed = plan
        .parts
        .iter()
        .find(|p| p.role == "fixed_shelf_1")
        .unwrap();
    let spread = plan.parts.iter().find(|p| p.role == "shelf_1").unwrap();
    assert_eq!(fixed.placement.origin.2, 1000.0);
    // Full inner depth (only the back groove clearance) vs the 20 mm setback.
    assert_eq!(spread.dims.width + 20.0, fixed.dims.width);
    // Pilot holes on the side at the shelf's mid-thickness.
    let side = plan.parts.iter().find(|p| p.role == "side_left").unwrap();
    assert!(side.operations.iter().any(|op| matches!(
        op.geometry,
        OpGeometry::Drill { u, .. } if (u - 1009.0).abs() < 1e-6
    )));
    // No overlap between the fixed shelf and the shelves above and below.
    assert!(!plan.diagnostics.items.iter().any(|d| d.code == "FAB-101"));
}

#[test]
fn fixed_shelf_positions_are_validated() {
    let both = BOOKCASE.replace(
        "\"positions\": [\"divider_z\"],",
        "\"positions\": [\"divider_z\"], \"count\": 1,",
    );
    let plan = rewood_core::compile_json(&both);
    assert!(plan.diagnostics.items.iter().any(|d| d.code == "SPEC-310"));

    let outside = BOOKCASE.replace("\"divider_z\": 1000", "\"divider_z\": 1990");
    let plan = rewood_core::compile_json(&outside);
    assert!(plan.manufacturing_blocked);
    assert!(plan.diagnostics.items.iter().any(|d| d.code == "SPEC-311"));

    let unsorted = BOOKCASE.replace(
        "\"positions\": [\"divider_z\"],",
        "\"positions\": [\"divider_z\", \"divider_z + 10\"],",
    );
    let plan = rewood_core::compile_json(&unsorted);
    assert!(plan.diagnostics.items.iter().any(|d| d.code == "SPEC-311"));
}

const KITCHEN: &str = include_str!("../../../fixtures/kitchen_run/input.json");

#[test]
fn modules_with_an_origin_stand_side_by_side_with_everything_that_hangs_on_them() {
    let plan = rewood_core::compile_json(KITCHEN);
    assert_eq!(plan.status, PlanStatus::Ok, "{:#?}", plan.diagnostics);
    let part = |component: &str, role: &str| {
        plan.parts
            .iter()
            .find(|p| p.component == component && p.role == role)
            .unwrap_or_else(|| panic!("{component}/{role}"))
    };
    // The second module starts at x = 600 and its front follows it.
    assert_eq!(part("m2", "side_left").aabb.min.0, 600.0);
    assert_eq!(part("drawers_m2", "drawer_1_front").aabb.min.0, 602.0);
    assert_eq!(part("door_m3", "door_1").aabb.min.0, 1202.0);
    // Modules touch without overlapping: no FAB-101.
    assert!(!plan.diagnostics.items.iter().any(|d| d.code == "FAB-101"));
    // The handle moved with its front: holes at the front's own centre.
    let front = part("drawers_m2", "drawer_1_front");
    let handle_us: Vec<f64> = front
        .operations
        .iter()
        .filter(|op| {
            op.source
                .as_ref()
                .is_some_and(|s| s.hardware == "handle_bar_128")
        })
        .filter_map(|op| match op.geometry {
            OpGeometry::Drill { u, .. } => Some(u),
            _ => None,
        })
        .collect();
    assert_eq!(handle_us, vec![362.0, 234.0]);
    // Legs and plinth clips, three modules' worth.
    let qty = |id: &str| {
        plan.bom
            .hardware
            .iter()
            .find(|h| h.hardware == id)
            .map(|h| h.quantity)
            .unwrap_or(0)
    };
    assert_eq!(qty("leg_adjustable_100"), 12);
    assert_eq!(qty("plinth_clip"), 6);
    // Leg screws on the underside of every bottom, none anywhere else.
    let leg_holes = plan
        .parts
        .iter()
        .filter(|p| p.role == "bottom")
        .flat_map(|p| &p.operations)
        .filter(|op| {
            op.source
                .as_ref()
                .is_some_and(|s| s.hardware == "leg_adjustable_100")
        })
        .count();
    assert_eq!(leg_holes, 3 * 4 * 4);
    assert!(plan.parts.iter().any(|p| p.aabb.min.2 == -100.0));
}

/// A plan frozen in an order (§32) must load with today's model: every
/// field added since goes with `#[serde(default)]`. This strips the newer
/// keys from a current plan and reads it back.
#[test]
fn older_snapshots_still_load() {
    let mut v: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/basic_cabinet/expected.json"
    ))
    .unwrap();
    let obj = v.as_object_mut().unwrap();
    obj.remove("machining");
    obj.remove("purchasing");
    obj["profile"].as_object_mut().unwrap().remove("machine");
    obj["profile"].as_object_mut().unwrap().remove("currency");
    obj["versions"].as_object_mut().unwrap().remove("suppliers");
    let bom = obj["bom"].as_object_mut().unwrap();
    for k in [
        "materialsCost",
        "machiningCost",
        "totalCost",
        "currency",
        "unpriced",
    ] {
        bom.remove(k);
    }
    for line in bom["sheets"].as_array_mut().unwrap() {
        line.as_object_mut().unwrap().remove("cost");
    }
    for line in bom["hardware"].as_array_mut().unwrap() {
        line.as_object_mut().unwrap().remove("cost");
        for i in line["items"].as_array_mut().unwrap() {
            i.as_object_mut().unwrap().remove("cost");
        }
    }
    for line in bom["consumables"].as_array_mut().unwrap() {
        line.as_object_mut().unwrap().remove("cost");
    }
    for layout in obj["nesting"].as_array_mut().unwrap() {
        layout.as_object_mut().unwrap().remove("cuts");
    }
    let plan: rewood_core::plan::ManufacturingPlan = serde_json::from_value(v).unwrap();
    assert_eq!(plan.parts.len(), 9);
    assert_eq!(plan.bom.total_cost, 0.0);
    assert!(plan.purchasing.is_empty());
}
