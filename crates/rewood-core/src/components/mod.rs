//! Component generators: a component is design intent ("a carcass 1000 wide
//! with a grooved back"); the generator expands it into parts placed in
//! furniture space and joint requests between them. It never places a hole:
//! holes come from resolving joints against the hardware library.

mod carcass;
mod doors;
mod drawers;
mod layout;
mod rail;
mod shelves;
mod worktop;

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
    /// A line of identical holes on one face of one part (a System 32 row
    /// for shelf pins): from `from` to `to` on `face`, one fastener per
    /// hole at the hardware's pitch.
    Row {
        from: Vec3,
        to: Vec3,
        face: crate::geometry::Face,
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
            JointKind::Row { .. } => "row",
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
    /// Fastener positions along the joint snap to `reference + k * pitch`
    /// in world coordinates on the joint's axis: hinge cups land on the
    /// System 32 grid so their plate holes are the row's holes.
    pub snap: Option<(f64, f64)>,
}

/// Inner box of a carcass, needed by shelves and doors.
///
/// System 32: shelf-pin rows run at [`ROW_INSET`] from the front edge (and
/// from the back panel), their holes at [`PIN_PITCH`] starting
/// [`GRID_ORIGIN`] above the carcass floor; hinge cups sit half a pitch
/// off that grid so the mounting plate's two holes, 32 apart, fall on it.
pub const PIN_PITCH: f64 = 32.0;
pub const ROW_INSET: f64 = 37.0;
pub const GRID_ORIGIN: f64 = 37.0;
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
    /// Wall-hung carcass: nothing on the sides' inner faces may reach
    /// above this height at the back, the hangers sit there.
    pub hanger_clear_z: Option<f64>,
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
    /// How many consecutive carcass bays this (virtual) bay covers; 1 for
    /// a real bay, more for a door set spanning several.
    pub spans: usize,
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
    /// Inner drawers: inside the opening, a door closes over them.
    InnerDrawers,
    Shelves,
    /// The hanging height under a rail.
    Rail,
}

/// Hardware bought by the metre or per component rather than per
/// fastener (a rail bar): goes straight to the BOM.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtraBom {
    pub component: String,
    pub hardware: String,
    pub quantity: usize,
    pub metres: f64,
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
    /// Y range of the front panel (fronts only): where it sits in depth.
    pub front_y: Option<(f64, f64)>,
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
    pub extra_bom: Vec<ExtraBom>,
}

/// World axes whose facing edges get banded, from the component's choice
/// and the generator's default. `Front` is the edge facing +Y.
fn mount_es(mount: &str) -> &'static str {
    match mount {
        "inset" => "embutida",
        "half_overlay" => "de media superposición",
        _ => "superpuesta",
    }
}

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
            extra_bom: Vec::new(),
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
        self.occupy_front(component, kind, carcass, bays, zone, None);
    }

    /// Like [`occupy`](Self::occupy), for a front that sits at a known depth.
    pub fn occupy_front(
        &mut self,
        component: &str,
        kind: OccupancyKind,
        carcass: &CarcassInfo,
        bays: &[Bay],
        zone: Zone,
        front_y: Option<(f64, f64)>,
    ) {
        for bay in bays {
            self.occupancy.push(Occupancy {
                component: component.to_string(),
                kind,
                carcass: carcass.id.clone(),
                bay: bay.index,
                zone,
                front_y,
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
            snap: None,
        });
    }

    /// Like [`request`], with the fasteners snapped to a world grid along
    /// the joint axis.
    #[allow(clippy::too_many_arguments)]
    pub fn request_snapped(
        &mut self,
        kind: JointKind,
        component: &str,
        part_a: &str,
        part_b: &str,
        joint: &JointSpec,
        reference: f64,
        pitch: f64,
    ) {
        self.request(kind, component, part_a, part_b, joint);
        if let Some(r) = self.joint_requests.last_mut() {
            r.snap = Some((reference, pitch));
        }
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

    /// Group consecutive bays into one virtual bay `span` wide, starting
    /// at each listed bay: the opening a wide door set covers. The inner
    /// range runs from the first bay's left panel to the last bay's right
    /// panel; the dividers in between stay behind the doors.
    pub fn span_bays(
        &self,
        component: &str,
        carcass: &CarcassInfo,
        bays: Vec<Bay>,
        span: Option<&ParamInput>,
    ) -> Result<Vec<Bay>, Diagnostic> {
        let Some(span) = span else {
            return Ok(bays);
        };
        let n = self.eval(component, "span", span)?;
        if n < 1.0 || n.fract() != 0.0 {
            return Err(Diagnostic::new(
                "SPEC-303",
                Severity::Fatal,
                format!("'{component}.span' tiene que ser un entero ≥ 1, es {n}"),
            )
            .entity(component));
        }
        let n = n as usize;
        let mut out = Vec::new();
        let mut next_free = 0;
        for first in bays {
            if first.index < next_free {
                // Already covered by the previous span (all-bays mode).
                continue;
            }
            let last_index = first.index + n - 1;
            let Some(last) = carcass.bays.iter().find(|b| b.index == last_index) else {
                return Err(Diagnostic::new(
                    "SPEC-204",
                    Severity::Fatal,
                    format!(
                        "'{component}' cubre las bahías {}–{last_index} y la carcasa tiene {}",
                        first.index,
                        carcass.bays.len()
                    ),
                )
                .entity(component));
            };
            next_free = last_index + 1;
            out.push(Bay {
                index: first.index,
                x0: first.x0,
                x1: last.x1,
                cover_x0: first.cover_x0,
                cover_x1: last.cover_x1,
                left_part: first.left_part.clone(),
                right_part: last.right_part.clone(),
                spans: n,
            });
        }
        Ok(out)
    }

    /// The hinge hardware for a door mount. The default overlay hinge on
    /// an inset door is swapped for the library's inset hinge; anything
    /// else that does not match is an error, because the arm geometry
    /// decides where the plate goes.
    /// The hinge a door actually needs: the arm for how it sits on its
    /// panel (`overlay`, `half_overlay` on a divider shared with another
    /// door, `inset`) and, if the doors say so, with or without a damper.
    /// The spec's hinge names the family (Ø35 cup, its opening angle);
    /// the variant is the library's item of that angle with the wanted
    /// mount and damper. `SPEC-314` when the library has none.
    pub fn hinge_variant(
        &self,
        component: &str,
        hinge: &JointSpec,
        mount: &str,
        soft_close: Option<bool>,
    ) -> Result<JointSpec, Diagnostic> {
        let mut out = hinge.clone();
        for h in &mut out.hardware {
            let Some(def) = self.libs.hardware.get(h) else {
                continue;
            };
            let Some(spec) = &def.hinge else {
                continue;
            };
            let soft = soft_close.unwrap_or(spec.soft_close);
            if spec.mount == mount && spec.soft_close == soft {
                continue;
            }
            let alt = self.libs.hardware.iter().find(|d| {
                d.kind == "hinge"
                    && d.hinge.as_ref().is_some_and(|s| {
                        s.mount == mount
                            && s.soft_close == soft
                            && (s.opening - spec.opening).abs() < crate::units::EPS
                    })
            });
            match alt {
                Some(alt) => *h = alt.id.clone(),
                None => {
                    return Err(Diagnostic::new(
                        "SPEC-314",
                        Severity::Fatal,
                        format!(
                            "'{component}': no hay una bisagra {}{} de {}° como '{}' en la biblioteca",
                            mount_es(mount),
                            if soft { " con cierre suave" } else { "" },
                            spec.opening,
                            def.name
                        ),
                    )
                    .entity(component)
                    .suggestion(
                        "Elegí otra familia de bisagra, o agregá la variante en libraries.hardware.",
                    ));
                }
            }
        }
        Ok(out)
    }

    /// The slide a drawer actually needs: the spec's slide names length
    /// and style, `softClose` picks the damped variant of the same length
    /// and style (or the plain one). `SPEC-321` when the library has none.
    pub fn slide_variant(
        &self,
        component: &str,
        slide: &JointSpec,
        soft_close: Option<bool>,
    ) -> Result<JointSpec, Diagnostic> {
        let Some(soft) = soft_close else {
            return Ok(slide.clone());
        };
        let mut out = slide.clone();
        for h in &mut out.hardware {
            let Some(def) = self.libs.hardware.get(h) else {
                continue;
            };
            let Some(spec) = &def.slide else {
                continue;
            };
            if spec.soft_close == soft {
                continue;
            }
            let alt = self.libs.hardware.iter().find(|d| {
                d.kind == "slide"
                    && d.slide.as_ref().is_some_and(|s| {
                        s.soft_close == soft
                            && s.style == spec.style
                            && (s.length - spec.length).abs() < crate::units::EPS
                            && (s.side_clearance - spec.side_clearance).abs() < crate::units::EPS
                    })
            });
            match alt {
                Some(alt) => *h = alt.id.clone(),
                None => {
                    return Err(Diagnostic::new(
                        "SPEC-321",
                        Severity::Fatal,
                        format!(
                            "'{component}': no hay una corredera como '{}' {} cierre suave en la biblioteca",
                            def.name,
                            if soft { "con" } else { "sin" }
                        ),
                    )
                    .entity(component)
                    .suggestion(
                        "Las de rodillo no vienen con cierre suave: elegí una telescópica a bolillas, o sacá 'softClose'.",
                    ));
                }
            }
        }
        Ok(out)
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

    /// Two components of the same kind name their parts alike ("Puerta 1"
    /// from each door set): a cut list with two different "Puerta 1" rows
    /// tells the workshop nothing. Suffix the component id wherever a
    /// name is shared across components.
    fn disambiguate_names(&mut self) {
        let mut owners: BTreeMap<String, std::collections::BTreeSet<String>> = BTreeMap::new();
        for part in &self.parts {
            owners
                .entry(part.name.clone())
                .or_default()
                .insert(part.component.clone());
        }
        for part in &mut self.parts {
            if owners[&part.name].len() > 1 {
                part.name = format!("{} ({})", part.name, part.component);
            }
        }
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
                    JointKind::Row { from, to, .. } => {
                        *from = *from + o;
                        *to = *to + o;
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
            ComponentSpec::Rail { .. } => rail::build(ctx, component),
            ComponentSpec::Worktop { .. } => worktop::build(ctx, component),
        };
        if let Err(d) = result {
            ctx.diagnostics.push(d);
        }
    }
    layout::check(ctx);
    ctx.disambiguate_names();
    ctx.apply_origins();
}

impl Scope for BuildCtx<'_> {
    fn lookup(&self, name: &str) -> Option<Value> {
        self.scope().lookup(name)
    }
}
