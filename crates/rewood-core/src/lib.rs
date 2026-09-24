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
pub mod options;
pub mod params;
pub mod plan;
pub mod rules;
pub mod simulation;
pub mod spec;
mod stagger;
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

/// What the components and their joints produce, before any rule looks.
struct Built {
    parts: Vec<model::Part>,
    joints: Vec<model::Joint>,
    derived: std::collections::BTreeMap<String, f64>,
    diags: Diagnostics,
    extra_bom: Vec<components::ExtraBom>,
    inactive: Vec<String>,
}

fn build(spec: &FurnitureSpec, params: &ParamGraph, libs: &Libraries, hints: Hints) -> Built {
    let mut ctx = components::BuildCtx::new(spec, params, libs);
    ctx.pin_avoid = hints.pin_avoid;
    ctx.spacers = hints.spacers;
    components::expand(&mut ctx);
    // Carcasses turned about their origin, with what is built on them.
    let turns: std::collections::BTreeMap<String, (geometry::Vec3, u8)> = ctx
        .carcass_of
        .iter()
        .filter_map(|(component, carcass)| {
            let c = ctx.carcasses.get(carcass)?;
            (c.turns != 0).then(|| (component.clone(), (c.origin, c.turns)))
        })
        .collect();
    let components::BuildCtx {
        mut parts,
        mut joint_requests,
        derived,
        diagnostics: mut diags,
        extra_bom,
        disabled: inactive,
        ..
    } = ctx;
    // A spec that asks for thousands of parts (a runaway count) is a
    // mistake, not furniture: stop before joints, nesting and CAM spend
    // minutes on it.
    if parts.len() > MAX_PARTS {
        diags.push(Diagnostic::new(
            "SPEC-002",
            Severity::Fatal,
            format!(
                "la spec genera {} piezas; el máximo es {MAX_PARTS}: revisá las cantidades",
                parts.len()
            ),
        ));
        parts.clear();
        joint_requests.clear();
    }
    // A joint between something that turns and something that does not (a
    // worktop over a turned carcass) is resolved once both are where they
    // end up; everything else in the frame it was generated in.
    let component_of: std::collections::BTreeMap<String, String> = parts
        .iter()
        .map(|p| (p.id.clone(), p.component.clone()))
        .collect();
    let turned = |part: &str| {
        component_of
            .get(part)
            .is_some_and(|c| turns.contains_key(c))
    };
    let (late, early): (Vec<_>, Vec<_>) = joint_requests.into_iter().partition(|r| {
        !turns.contains_key(&r.component) && (turned(&r.part_a) || turned(&r.part_b))
    });
    let mut joints = joints::resolve(&mut parts, &early, libs, &mut diags);
    // Fasteners of butt joints that run into another hole move along
    // their joint line before anything checks them.
    stagger::stagger(&mut parts, &mut joints, libs);
    // Everything is generated and joined facing +Y; a turned carcass turns
    // now, parts and joints alike, before any rule looks at the whole.
    components::turn(&mut parts, &mut joints, &turns);
    let first = joints.len() + 1;
    joints.extend(joints::resolve_from(
        &mut parts, &late, libs, &mut diags, first,
    ));
    Built {
        parts,
        joints,
        derived,
        diags,
        extra_bom,
        inactive,
    }
}

/// What a first build tells the second: shelves to move off a hole,
/// inner drawers to move off an open door.
#[derive(Default)]
struct Hints {
    pin_avoid: std::collections::BTreeMap<String, Vec<f64>>,
    spacers: std::collections::BTreeMap<String, f64>,
}

/// More parts than any piece of furniture has.
const MAX_PARTS: usize = 2000;

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

/// Give every part of a material sold in several designs its colour:
/// fronts (doors, drawer fronts) take `frontDecor`, the rest `decor`, and
/// a colour the material is not sold in falls back to its default.
fn assign_decors(parts: &mut [model::Part], spec: &FurnitureSpec, libs: &Libraries) {
    for part in parts {
        let Some(material) = libs.materials.material(&part.material) else {
            continue;
        };
        let wanted = if is_front_role(&part.role) {
            spec.front_decor.as_ref().or(spec.decor.as_ref())
        } else {
            spec.decor.as_ref()
        };
        let sold = |id: &String| {
            libs.materials
                .decor(id)
                .is_some_and(|d| d.materials.contains(&part.material))
        };
        part.decor = wanted
            .filter(|id| sold(id))
            .or(material.default_decor.as_ref().filter(|id| sold(id)))
            .cloned();
    }
}

/// What the person sees from outside and may want in another colour: a
/// door, a flap, a drawer front, a fixed front. Not a drawer box's front.
pub fn is_front_role(role: &str) -> bool {
    role.contains("door") || (role.contains("front") && !role.contains("box_front"))
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
    for (field, decor) in [("decor", &spec.decor), ("frontDecor", &spec.front_decor)] {
        if let Some(id) = decor {
            if libs.materials.decor(id).is_none() {
                diags.push(
                    Diagnostic::new(
                        "LIB-106",
                        Severity::Error,
                        format!("color desconocido '{id}' en {field}: se usa el de la placa por defecto"),
                    )
                    .suggestion("Elegí un color de la biblioteca (libraries.materials, decors)."),
                );
            }
        }
    }
    if diags.has_fatal() {
        return empty_plan(furniture, libs, diags);
    }
    if spec.edge_material.is_none() {
        diags.push(
            Diagnostic::new(
                "DESIGN-109",
                Severity::Info,
                "el mueble no tiene material de canto: todos los bordes de placa quedan a la vista",
            )
            .suggestion(
                "Declará edgeMaterial (por ejemplo pvc_0_45mm, el tapacanto PVC 22 × 0,45).",
            )
            .fix_opt(
                libs.materials
                    .edge("pvc_0_45mm")
                    .or_else(|| libs.materials.edges().next())
                    .map(|e| {
                        (
                            format!("Cantear con {}", e.name),
                            String::new(),
                            "edgeMaterial".to_string(),
                            serde_json::json!(e.id),
                        )
                    }),
            ),
        );
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

    // 2. Components -> parts + joint requests, and 4. joints -> fasteners
    // -> holes. A shelf pin that meets a hole on the panel's other face
    // (a slide screw), or an inner drawer that comes out where its door
    // stands open, sends the build round once more: that shelf told to
    // take another grid line, those drawers to sit on spacers.
    let mut built = build(spec, &params, libs, Hints::default());
    let hints = Hints {
        pin_avoid: stagger::pin_conflicts(&built.parts, &built.joints, libs),
        spacers: rules::design::spacer_needs(&built.parts, &built.joints),
    };
    if !hints.pin_avoid.is_empty() || !hints.spacers.is_empty() {
        built = build(spec, &params, libs, hints);
    }
    let Built {
        mut parts,
        joints,
        derived,
        diags: built_diags,
        extra_bom,
        inactive,
    } = built;
    diags.extend(built_diags);
    assign_decors(&mut parts, spec, libs);

    // 3. Constraints over parameters and derived values.
    let scope = Chain(&derived, &params);
    for c in &spec.constraints {
        let applies = match &c.when {
            None => Ok(true),
            Some(params::ParamInput::Bool(b)) => Ok(*b),
            Some(params::ParamInput::Number(_)) => Err("'when' es un número".to_string()),
            Some(params::ParamInput::Expr(src)) => {
                match Expr::parse(src).and_then(|e| e.eval(&scope)) {
                    Ok(Value::Bool(b)) => Ok(b),
                    Ok(Value::Number(_)) => Err(format!("'when' no es una condición: {src}")),
                    Err(e) => Err(format!("'when': {e}")),
                }
            }
        };
        match applies {
            Ok(true) => {}
            Ok(false) => continue,
            Err(e) => {
                diags.push(
                    Diagnostic::new(
                        "SPEC-402",
                        Severity::Fatal,
                        format!("la restricción '{}' no se pudo evaluar: {e}", c.id),
                    )
                    .entity(c.id.clone()),
                );
                continue;
            }
        }
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

    // Template options, resolved for the UI and checked against the values.
    let (options, option_diags) = options::resolve(spec, &scope);
    for d in option_diags {
        diags.push(d);
    }

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
    let bom = plan::bom(
        &parts,
        &joints,
        &nesting,
        machining.total_seconds,
        libs,
        &extra_bom,
    );
    let purchasing = plan::purchasing(&bom, libs);
    let catalog = plan::Catalog::of(&parts, libs);

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
        options,
        inactive,
        derived,
        parts,
        joints,
        part_list,
        bom,
        nesting,
        machining,
        purchasing,
        catalog,
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
        options: Vec::new(),
        inactive: Vec::new(),
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
        catalog: plan::Catalog::default(),
        diagnostics: diags,
    }
}
