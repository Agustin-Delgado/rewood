//! Component generators: a component is design intent ("a carcass 1000 wide
//! with a grooved back"); the generator expands it into parts placed in
//! furniture space and joint requests between them. It never places a hole:
//! holes come from resolving joints against the hardware library.

mod carcass;
mod doors;
mod drawers;
mod layout;
mod shelves;

use std::collections::BTreeMap;

use crate::diagnostics::{Diagnostic, Diagnostics, Severity};
use crate::expr::{Chain, Scope, Value};
use crate::geometry::{Aabb, Axis, Dims, Face, Placement, Vec3};
use crate::library::hardware::PlacementRule;
use crate::library::Libraries;
use crate::model::{CutSize, Grain, OpGeometry, Operation, Part};
use crate::params::{ParamGraph, ParamInput};
use crate::spec::{ComponentSpec, EdgeBanding, FurnitureSpec, JointSpec};

/// How two parts are joined.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JointKind {
    /// One part's edge against the other's large face; the geometry decides
    /// which is which.
    Butt,
    /// A door hung on a carcass side. `part_a` is the door, `part_b` the
    /// side, and `hinge_edge` the world direction of the door's hinge edge.
    Hinge { hinge_edge: Axis },
    /// A drawer box side running on a carcass side. `part_a` is the box
    /// side, `part_b` the carcass side. The joint line runs from the front
    /// of the box towards the back, at the slide's hole height.
    Slide,
    /// Two large faces pressed together (a drawer front on its box front).
    /// Fasteners are laid out on a grid: along the long side by the
    /// placement rule, in two rows across when the short side allows.
    /// `part_a` is the one drilled through (edge part), `part_b` receives
    /// the screws (face part).
    FaceToFace,
    /// A handle on a panel: no second part. `centre` is the handle's
    /// middle on the panel's `Front` face, `along` the direction it runs.
    Handle { centre: Vec3, along: Axis },
    /// Something screwed onto one face of one part (a leg under the
    /// bottom, a plinth clip): a point on `face`, holes laid out with
    /// `offsetAlong` on `along` and `offsetAcross` on the other in-plane
    /// axis.
    Fixture {
        centre: Vec3,
        face: crate::geometry::Face,
        along: Axis,
    },
}

impl JointKind {
    pub fn label(self) -> &'static str {
        match self {
            JointKind::Butt => "butt",
            JointKind::Hinge { .. } => "hinge",
            JointKind::Slide => "slide",
            JointKind::FaceToFace => "face_to_face",
            JointKind::Handle { .. } => "handle",
            JointKind::Fixture { .. } => "fixture",
        }
    }
}

/// Intent to join two parts with some hardware.
#[derive(Debug, Clone, PartialEq)]
pub struct JointRequest {
    pub kind: JointKind,
    pub component: String,
    pub part_a: String,
    pub part_b: String,
    pub hardware: Vec<String>,
    pub placement: Option<PlacementRule>,
}

/// Inner box of a carcass, needed by shelves and doors.
#[derive(Debug, Clone, PartialEq)]
pub struct CarcassInfo {
    pub id: String,
    /// Translation applied to every part of the carcass and of the
    /// components that refer to it, after generation.
    pub origin: Vec3,
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub thickness: f64,
    pub material: String,
    /// World Y where the usable inner depth starts (after the back panel).
    pub inner_y0: f64,
    pub side_left: String,
    pub side_right: String,
    pub bays: Vec<Bay>,
}

/// One compartment of a carcass, between two vertical panels.
#[derive(Debug, Clone, PartialEq)]
pub struct Bay {
    /// 1-based.
    pub index: usize,
    /// Inner X range, between the two panels' faces.
    pub x0: f64,
    pub x1: f64,
    /// X range a front (door, drawer front) covers: out to the carcass
    /// edge on a side, to the divider's centre on a divider.
    pub cover_x0: f64,
    pub cover_x1: f64,
    pub left_part: String,
    pub right_part: String,
}

/// A vertical range of a bay: [z0, z1] in furniture Z.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zone {
    pub z0: f64,
    pub z1: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OccupancyKind {
    Doors,
    Drawers,
    Shelves,
}

/// What a dependent component took of a carcass bay, for the layout
/// checks that run once everything is expanded.
#[derive(Debug, Clone, PartialEq)]
pub struct Occupancy {
    pub component: String,
    pub kind: OccupancyKind,
    pub carcass: String,
    pub bay: usize,
    pub zone: Zone,
}

pub struct BuildCtx<'a> {
    pub spec: &'a FurnitureSpec,
    pub params: &'a ParamGraph,
    pub libs: &'a Libraries,
    pub parts: Vec<Part>,
    pub joint_requests: Vec<JointRequest>,
    /// Values computed by generators, published as `component.name` so
    /// constraints can reference them.
    pub derived: BTreeMap<String, f64>,
    pub diagnostics: Diagnostics,
    pub carcasses: BTreeMap<String, CarcassInfo>,
    /// Component id → carcass id it belongs to (a carcass maps to itself).
    pub carcass_of: BTreeMap<String, String>,
    pub occupancy: Vec<Occupancy>,
}

/// World axes whose facing edges get banded, from the component's choice
/// and the generator's default. `Front` is the edge facing +Y.
pub fn banded_axes(choice: EdgeBanding, default: &[Axis]) -> Vec<Axis> {
    match choice {
        EdgeBanding::Default => default.to_vec(),
        EdgeBanding::None => Vec::new(),
        EdgeBanding::Front => vec![Axis::PosY],
        EdgeBanding::All => vec![
            Axis::PosX,
            Axis::NegX,
            Axis::PosY,
            Axis::NegY,
            Axis::PosZ,
            Axis::NegZ,
        ],
    }
}

/// What a generator needs to say about a part; everything else is derived.
pub struct PartInit<'a> {
    pub name: String,
    pub component: &'a str,
    pub role: &'a str,
    pub material: &'a str,
    pub length: f64,
    pub width: f64,
    pub grain: Grain,
    pub placement: Placement,
    /// World axes whose facing edges get the default edge band.
    pub banded_edges: &'a [Axis],
}

impl<'a> BuildCtx<'a> {
    pub fn new(spec: &'a FurnitureSpec, params: &'a ParamGraph, libs: &'a Libraries) -> Self {
        BuildCtx {
            spec,
            params,
            libs,
            parts: Vec::new(),
            joint_requests: Vec::new(),
            derived: BTreeMap::new(),
            diagnostics: Diagnostics::default(),
            carcasses: BTreeMap::new(),
            carcass_of: BTreeMap::new(),
            occupancy: Vec::new(),
        }
    }

    /// Record what a component takes of its carcass, one entry per bay.
    pub fn occupy(
        &mut self,
        component: &str,
        kind: OccupancyKind,
        carcass: &CarcassInfo,
        bays: &[Bay],
        zone: Zone,
    ) {
        for bay in bays {
            self.occupancy.push(Occupancy {
                component: component.to_string(),
                kind,
                carcass: carcass.id.clone(),
                bay: bay.index,
                zone,
            });
        }
    }

    /// A finding that does not stop the component: it is generated as
    /// asked, and the operator reads why it is probably wrong.
    pub fn warn(&mut self, d: Diagnostic) {
        self.diagnostics.push(d);
    }

    /// DESIGN-113: a dimension typed as a number where the spec expects a
    /// parameter, so nothing else follows it when it changes. Zero is not
    /// a dimension (`zone.from: 0`) and is left alone.
    pub fn note_literals(&mut self, component: &str, fields: &[(&str, &ParamInput)]) {
        let literals: Vec<String> = fields
            .iter()
            .filter_map(|(name, v)| match v {
                ParamInput::Number(n) if *n != 0.0 => Some(format!("{name} = {n}")),
                _ => None,
            })
            .collect();
        if literals.is_empty() {
            return;
        }
        self.warn(
            Diagnostic::new(
                "DESIGN-113",
                Severity::Info,
                format!(
                    "'{component}' tiene medidas escritas a mano ({}); con un parámetro, el resto del mueble las sigue",
                    literals.join(", ")
                ),
            )
            .entity(component)
            .suggestion("Declaralas en 'parameters' y referencialas por nombre."),
        );
    }

    pub fn scope(&self) -> Chain<'_> {
        Chain(&self.derived, self.params)
    }

    /// Evaluate a number-or-expression field of a component.
    pub fn eval(&self, component: &str, field: &str, v: &ParamInput) -> Result<f64, Diagnostic> {
        let value = match v {
            ParamInput::Number(n) => Value::Number(*n),
            ParamInput::Bool(_) => {
                return Err(Diagnostic::new(
                    "SPEC-102",
                    Severity::Fatal,
                    format!("el campo '{field}' del componente '{component}' es un booleano y se esperaba un número"),
                )
                .entity(component))
            }
            ParamInput::Expr(src) => {
                let expr = crate::expr::Expr::parse(src).map_err(|e| {
                    Diagnostic::new(
                        "SPEC-101",
                        Severity::Fatal,
                        format!("el campo '{field}' del componente '{component}' no se pudo interpretar: {e}"),
                    )
                    .entity(component)
                })?;
                expr.eval(&self.scope()).map_err(|e| {
                    Diagnostic::new(
                        "SPEC-101",
                        Severity::Fatal,
                        format!("el campo '{field}' del componente '{component}' no se pudo evaluar: {e}"),
                    )
                    .entity(component)
                })?
            }
        };
        value.as_number().map_err(|e| {
            Diagnostic::new(
                "SPEC-102",
                Severity::Fatal,
                format!("campo '{field}' de '{component}': {e}"),
            )
            .entity(component)
        })
    }

    pub fn publish(&mut self, component: &str, name: &str, value: f64) {
        self.derived.insert(format!("{component}.{name}"), value);
    }

    pub fn material_or_default<'b>(
        &'b self,
        component: &str,
        id: Option<&'b String>,
    ) -> Result<&'b str, Diagnostic> {
        let id = id.map(String::as_str).unwrap_or(&self.spec.material);
        if self.libs.materials.material(id).is_none() {
            return Err(Diagnostic::new(
                "LIB-101",
                Severity::Fatal,
                format!("material desconocido '{id}'"),
            )
            .entity(component)
            .suggestion("Declaralo en libraries.materials o usá uno de la biblioteca."));
        }
        Ok(id)
    }

    pub fn thickness_of(&self, material: &str) -> f64 {
        self.libs
            .materials
            .material(material)
            .map(|m| m.nominal_thickness)
            .unwrap_or(0.0)
    }

    pub fn add_part(&mut self, init: PartInit<'_>) -> String {
        let material = self
            .libs
            .materials
            .material(init.material)
            .expect("material validated by caller");
        let dims = Dims {
            length: init.length,
            width: init.width,
            thickness: material.nominal_thickness,
        };
        let grain = if material.grain == crate::library::material::GrainKind::None {
            Grain::None
        } else {
            init.grain
        };

        let mut edges = BTreeMap::new();
        let mut cut = CutSize {
            length: init.length,
            width: init.width,
        };
        if let Some(edge_id) = &self.spec.edge_material {
            if let Some(edge) = self.libs.materials.edge(edge_id) {
                for axis in init.banded_edges {
                    let face = init.placement.face_facing(*axis);
                    if face.is_large() {
                        // `All` names every world axis; two of them are the panel's faces.
                        continue;
                    }
                    edges.insert(face, edge_id.clone());
                    match face {
                        Face::Left | Face::Right => cut.length -= edge.thickness,
                        Face::Bottom | Face::Top => cut.width -= edge.thickness,
                        Face::Front | Face::Back => {}
                    }
                }
            }
        }

        let id = format!("P{:03}", self.parts.len() + 1);
        let aabb = Aabb::of_part(&init.placement, dims);
        self.parts.push(Part {
            id: id.clone(),
            name: init.name,
            component: init.component.to_string(),
            role: init.role.to_string(),
            material: init.material.to_string(),
            dims,
            cut,
            grain,
            edges,
            placement: init.placement,
            aabb,
            operations: Vec::new(),
            overlap_exempt: Vec::new(),
        });
        id
    }

    pub fn request_joint(
        &mut self,
        component: &str,
        part_a: &str,
        part_b: &str,
        joint: &JointSpec,
    ) {
        self.request(JointKind::Butt, component, part_a, part_b, joint);
    }

    pub fn request(
        &mut self,
        kind: JointKind,
        component: &str,
        part_a: &str,
        part_b: &str,
        joint: &JointSpec,
    ) {
        self.joint_requests.push(JointRequest {
            kind,
            component: component.to_string(),
            part_a: part_a.to_string(),
            part_b: part_b.to_string(),
            hardware: joint.hardware.clone(),
            placement: joint.placement.clone(),
        });
    }

    /// The bays a dependent component applies to: the one named by `bay`,
    /// or all of them.
    pub fn bays_for(
        &self,
        component: &str,
        carcass: &CarcassInfo,
        bay: Option<&ParamInput>,
    ) -> Result<Vec<Bay>, Diagnostic> {
        match bay {
            None => Ok(carcass.bays.clone()),
            Some(v) => {
                let n = self.eval(component, "bay", v)?;
                carcass
                    .bays
                    .iter()
                    .find(|b| b.index as f64 == n)
                    .cloned()
                    .map(|b| vec![b])
                    .ok_or_else(|| {
                        Diagnostic::new(
                            "SPEC-204",
                            Severity::Fatal,
                            format!(
                                "el componente '{component}' pide la bahía {n} y la carcasa tiene {}",
                                carcass.bays.len()
                            ),
                        )
                        .entity(component)
                    })
            }
        }
    }

    /// The height range a component works in: the declared zone, clamped
    /// to the carcass, or the whole carcass.
    pub fn zone_for(
        &self,
        component: &str,
        carcass: &CarcassInfo,
        zone: Option<&crate::spec::ZoneSpec>,
    ) -> Result<Zone, Diagnostic> {
        let Some(zone) = zone else {
            return Ok(Zone {
                z0: 0.0,
                z1: carcass.height,
            });
        };
        let z0 = self.eval(component, "zone.from", &zone.from)?;
        let z1 = self.eval(component, "zone.to", &zone.to)?;
        if z0 < 0.0 || z1 > carcass.height + crate::units::EPS || z1 <= z0 {
            return Err(Diagnostic::new(
                "SPEC-205",
                Severity::Fatal,
                format!(
                    "la zona {z0}–{z1} de '{component}' no cabe en una carcasa de {} mm de alto",
                    carcass.height
                ),
            )
            .entity(component));
        }
        Ok(Zone { z0, z1 })
    }

    /// A groove on a part's `Front` face along the world segment
    /// `from`–`to`, `width` wide and `depth` deep.
    pub fn groove(&mut self, part_id: &str, from: Vec3, to: Vec3, width: f64, depth: f64) {
        let part = self.part_mut(part_id);
        let (u0, v0) = part.world_point_to_face_uv(Face::Front, from);
        let (u1, v1) = part.world_point_to_face_uv(Face::Front, to);
        let op = Operation {
            id: part.next_op_id(),
            face: Face::Front,
            geometry: OpGeometry::Groove {
                from: [u0, v0],
                to: [u1, v1],
                width,
                depth,
            },
            source: None,
        };
        part.operations.push(op);
    }

    pub fn part_mut(&mut self, id: &str) -> &mut Part {
        self.parts
            .iter_mut()
            .find(|p| p.id == id)
            .expect("part id issued by add_part")
    }

    /// Resolve which carcass a dependent component refers to.
    pub fn carcass_for(
        &mut self,
        component: &str,
        id: Option<&String>,
    ) -> Result<CarcassInfo, Diagnostic> {
        let found = match id {
            Some(id) => self.carcasses.get(id).cloned().ok_or_else(|| {
                Diagnostic::new("SPEC-201", Severity::Fatal, format!("el componente '{component}' refiere a una carcasa '{id}' que no existe"))
                    .entity(component)
            }),
            None => match self.carcasses.len() {
                1 => Ok(self.carcasses.values().next().unwrap().clone()),
                0 => Err(Diagnostic::new(
                    "SPEC-202",
                    Severity::Fatal,
                    format!("el componente '{component}' necesita una carcasa declarada antes"),
                )
                .entity(component)),
                _ => Err(Diagnostic::new(
                    "SPEC-203",
                    Severity::Fatal,
                    format!("hay varias carcasas; el componente '{component}' tiene que decir cuál con 'carcass'"),
                )
                .entity(component)),
            },
        };
        if let Ok(c) = &found {
            self.carcass_of.insert(component.to_string(), c.id.clone());
        }
        found
    }

    /// Move every part (and every joint anchored at a point) of a
    /// component by its carcass's origin. Generators work in carcass
    /// space; this is what puts modules next to each other.
    fn apply_origins(&mut self) {
        let offset_of = |carcass_of: &BTreeMap<String, String>, component: &str| -> Option<Vec3> {
            let cid = carcass_of.get(component)?;
            let o = self.carcasses.get(cid)?.origin;
            (o != Vec3(0.0, 0.0, 0.0)).then_some(o)
        };
        for part in &mut self.parts {
            if let Some(o) = offset_of(&self.carcass_of, &part.component) {
                part.placement.origin = part.placement.origin + o;
                part.aabb = crate::geometry::Aabb::of_part(&part.placement, part.dims);
            }
        }
        for req in &mut self.joint_requests {
            if let Some(o) = offset_of(&self.carcass_of, &req.component) {
                match &mut req.kind {
                    JointKind::Handle { centre, .. } | JointKind::Fixture { centre, .. } => {
                        *centre = *centre + o;
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Expand every component in declaration order. A component that fails
/// leaves a FATAL diagnostic and the rest still run, so one bad component
/// does not hide findings on the others.
pub fn expand(ctx: &mut BuildCtx<'_>) {
    let mut seen = std::collections::BTreeSet::new();
    for component in &ctx.spec.components {
        if !seen.insert(component.id().to_string()) {
            ctx.diagnostics.push(
                Diagnostic::new(
                    "SPEC-100",
                    Severity::Fatal,
                    format!("id de componente repetido '{}'", component.id()),
                )
                .entity(component.id()),
            );
            continue;
        }
        let result = match component {
            ComponentSpec::Carcass { .. } => carcass::build(ctx, component),
            ComponentSpec::Shelves { .. } => shelves::build(ctx, component),
            ComponentSpec::Doors { .. } => doors::build(ctx, component),
            ComponentSpec::Drawers { .. } => drawers::build(ctx, component),
        };
        if let Err(d) = result {
            ctx.diagnostics.push(d);
        }
    }
    layout::check(ctx);
    ctx.apply_origins();
}

impl Scope for BuildCtx<'_> {
    fn lookup(&self, name: &str) -> Option<Value> {
        self.scope().lookup(name)
    }
}
