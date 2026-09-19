//! rewood-core: a deterministic furniture manufacturing compiler.
//!
//! ```text
//! F(FurnitureSpec, MaterialLibrary, HardwareLibrary, ManufacturingRules, Profile)
//!   -> ManufacturingPlan
//! ```
//!
//! The same input always produces exactly the same output. Nothing in here
//! touches a clock, a random source, a file or a network; the CLI and the
//! WASM wrapper do the I/O.

// Generators return `Result<(), Diagnostic>`; a Diagnostic is ~150 bytes and
// the Err path is the rare one. Boxing it would cost more than it saves.
#![allow(clippy::result_large_err)]

pub mod cam;
pub mod components;
pub mod diagnostics;
pub mod export;
pub mod expr;
pub mod geometry;
pub mod joints;
pub mod library;
pub mod model;
pub mod nesting;
pub mod params;
pub mod plan;
pub mod rules;
pub mod simulation;
pub mod spec;
pub mod units;

use diagnostics::{Diagnostic, Diagnostics, Severity};
use expr::{Chain, Expr, Value};
use geometry::Face;
use library::Libraries;
use model::{OpGeometry, Operation};
use params::{ParamError, ParamGraph};
use plan::{FurnitureRef, ManufacturingPlan, Versions};
use spec::FurnitureSpec;

pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Compile a spec with the default libraries plus whatever the spec overrides.
pub fn compile(spec: &FurnitureSpec) -> ManufacturingPlan {
    match Libraries::default().with_overrides(&spec.libraries) {
        Ok(libs) => compile_with(spec, &libs),
        Err(e) => {
            let libs = Libraries::default();
            let mut diags = Diagnostics::default();
            diags.push(
                Diagnostic::new(
                    "LIB-105",
                    Severity::Fatal,
                    format!("sobreescritura de biblioteca inválida: {e}"),
                )
                .suggestion("Un id existente se parchea (sólo las claves que cambian); uno nuevo tiene que venir completo."),
            );
            empty_plan(
                FurnitureRef {
                    id: spec.id.clone(),
                    name: spec.name.clone(),
                    version: spec.version.clone(),
                },
                &libs,
                diags,
            )
        }
    }
}

/// Parse and compile JSON in one go; a malformed spec is reported as a
/// FATAL diagnostic inside an otherwise empty plan, never as a panic.
pub fn compile_json(json: &str) -> ManufacturingPlan {
    match FurnitureSpec::from_json(json) {
        Ok(spec) => compile(&spec),
        Err(e) => {
            let mut diags = Diagnostics::default();
            diags.push(Diagnostic::new(
                "SPEC-000",
                Severity::Fatal,
                format!("la especificación no se pudo leer: {e}"),
            ));
            empty_plan(
                FurnitureRef {
                    id: String::new(),
                    name: String::new(),
                    version: String::new(),
                },
                &Libraries::default(),
                diags,
            )
        }
    }
}

pub fn compile_with(spec: &FurnitureSpec, libs: &Libraries) -> ManufacturingPlan {
    let furniture = FurnitureRef {
        id: spec.id.clone(),
        name: spec.name.clone(),
        version: spec.version.clone(),
    };
    let mut diags = Diagnostics::default();

    if spec.schema_version != spec::SCHEMA_VERSION {
        diags.push(Diagnostic::new(
            "SPEC-001",
            Severity::Fatal,
            format!(
                "schemaVersion '{}' no soportada; este motor lee '{}'",
                spec.schema_version,
                spec::SCHEMA_VERSION
            ),
        ));
        return empty_plan(furniture, libs, diags);
    }
    if libs.materials.material(&spec.material).is_none() {
        diags.push(
            Diagnostic::new(
                "LIB-101",
                Severity::Fatal,
                format!("material por defecto desconocido '{}'", spec.material),
            )
            .suggestion("Declaralo en libraries.materials o usá uno de la biblioteca."),
        );
    }
    if let Some(edge) = &spec.edge_material {
        if libs.materials.edge(edge).is_none() {
            diags.push(Diagnostic::new(
                "LIB-104",
                Severity::Fatal,
                format!("material de canto desconocido '{edge}'"),
            ));
        }
    }
    if diags.has_fatal() {
        return empty_plan(furniture, libs, diags);
    }

    // 1. Parameters.
    let params = match ParamGraph::build(&spec.parameters) {
        Ok(p) => p,
        Err(e) => {
            let code = match e {
                ParamError::Cycle(_) => "PARAM-001",
                _ => "PARAM-002",
            };
            diags.push(Diagnostic::new(code, Severity::Fatal, e.to_string()));
            return empty_plan(furniture, libs, diags);
        }
    };

    // 2. Components -> parts + joint requests.
    let mut ctx = components::BuildCtx::new(spec, &params, libs);
    components::expand(&mut ctx);
    let components::BuildCtx {
        mut parts,
        joint_requests,
        derived,
        diagnostics: component_diags,
        ..
    } = ctx;
    diags.extend(component_diags);

    // 3. Constraints over parameters and derived values.
    let scope = Chain(&derived, &params);
    for c in &spec.constraints {
        let outcome = Expr::parse(&c.expr).and_then(|e| e.eval(&scope));
        match outcome {
            Ok(Value::Bool(true)) => {}
            Ok(Value::Bool(false)) => diags.push(
                Diagnostic::new(
                    "CON-001",
                    c.severity,
                    c.message.clone().unwrap_or_else(|| {
                        format!("no se cumple la restricción '{}': {}", c.id, c.expr)
                    }),
                )
                .entity(c.id.clone())
                .location(c.expr.clone()),
            ),
            Ok(Value::Number(_)) => diags.push(
                Diagnostic::new(
                    "SPEC-401",
                    Severity::Fatal,
                    format!(
                        "la restricción '{}' no es una comparación: {}",
                        c.id, c.expr
                    ),
                )
                .entity(c.id.clone()),
            ),
            Err(e) => diags.push(
                Diagnostic::new(
                    "SPEC-402",
                    Severity::Fatal,
                    format!("la restricción '{}' no se pudo evaluar: {e}", c.id),
                )
                .entity(c.id.clone()),
            ),
        }
    }

    // 4. Joints -> fasteners -> holes.
    let joints = joints::resolve(&mut parts, &joint_requests, libs, &mut diags);

    // 5. Edge banding as operations, after the holes so ids stay stable.
    for part in &mut parts {
        let edges: Vec<(Face, String)> = part.edges.iter().map(|(f, m)| (*f, m.clone())).collect();
        for (face, material) in edges {
            let thickness = libs
                .materials
                .edge(&material)
                .map(|e| e.thickness)
                .unwrap_or(0.0);
            let (length, _) = part.dims.face_extent(face);
            let op = Operation {
                id: part.next_op_id(),
                face,
                geometry: OpGeometry::EdgeBand {
                    material,
                    thickness,
                    length,
                },
                source: None,
            };
            part.operations.push(op);
        }
    }

    // 6. Manufacturing rules, then CAM feasibility: every operation has
    // to map to a tool the profile has. Then the NC each program renders
    // to is simulated (§28) and checked against the program.
    diags.extend(rules::run_all(&rules::RuleInput {
        parts: &parts,
        joints: &joints,
        libs,
    }));
    let machining = {
        use cam::PostProcessor;
        let post = cam::GenericIso::default();
        let mut programs = Vec::new();
        for part in &parts {
            for program in cam::programs(part, &libs.profile, &mut diags) {
                let sim = simulation::simulate(
                    &program,
                    &post.render(&program),
                    &libs.profile,
                    &mut diags,
                );
                programs.push(plan::ProgramSummary {
                    part: program.part.clone(),
                    setup: program.setup.to_string(),
                    operations: program.operations.len(),
                    tool_changes: sim.tool_changes,
                    cut_mm: sim.cut_mm,
                    rapid_mm: sim.rapid_mm,
                    seconds: sim.seconds,
                });
            }
        }
        plan::Machining {
            post_processor: post.id().into(),
            total_seconds: units::round3(programs.iter().map(|p| p.seconds).sum()),
            programs,
        }
    };

    // 7. Nesting, cut list and BOM. The sheet count in the BOM is the
    // nesting's; parts that fit no sheet were already reported by FAB-301.
    let nesting = nesting::nest(&parts, libs, &libs.profile.nesting);
    let part_list = plan::part_list(&parts, libs);
    let bom = plan::bom(&parts, &joints, &nesting, machining.total_seconds, libs);
    let purchasing = plan::purchasing(&bom, libs);

    diags.sort();
    let (status, blocked) = ManufacturingPlan::status_from(&diags);
    ManufacturingPlan {
        schema_version: spec::SCHEMA_VERSION.into(),
        furniture,
        versions: versions(libs),
        profile: libs.profile.clone(),
        status,
        manufacturing_blocked: blocked,
        parameters: params.values(),
        derived,
        parts,
        joints,
        part_list,
        bom,
        nesting,
        machining,
        purchasing,
        diagnostics: diags,
    }
}

fn versions(libs: &Libraries) -> Versions {
    Versions {
        schema: spec::SCHEMA_VERSION.into(),
        engine: ENGINE_VERSION.into(),
        materials: libs.materials.version.clone(),
        hardware: libs.hardware.version.clone(),
        profile: format!("{}@{}", libs.profile.id, libs.profile.version),
        suppliers: libs.suppliers.version.clone(),
    }
}

fn empty_plan(
    furniture: FurnitureRef,
    libs: &Libraries,
    mut diags: Diagnostics,
) -> ManufacturingPlan {
    diags.sort();
    let (status, blocked) = ManufacturingPlan::status_from(&diags);
    ManufacturingPlan {
        schema_version: spec::SCHEMA_VERSION.into(),
        furniture,
        versions: versions(libs),
        profile: libs.profile.clone(),
        status,
        manufacturing_blocked: blocked,
        parameters: Default::default(),
        derived: Default::default(),
        parts: Vec::new(),
        joints: Vec::new(),
        part_list: Vec::new(),
        bom: plan::Bom {
            sheets: Vec::new(),
            hardware: Vec::new(),
            consumables: Vec::new(),
            total_weight_kg: 0.0,
            materials_cost: 0.0,
            machining_cost: 0.0,
            total_cost: 0.0,
            currency: String::new(),
            unpriced: Vec::new(),
        },
        nesting: Vec::new(),
        machining: plan::Machining::default(),
        purchasing: Vec::new(),
        diagnostics: diags,
    }
}
