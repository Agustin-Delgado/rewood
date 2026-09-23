//! CAM: from a part's manufacturing operations to machine-independent
//! toolpaths, and from those to NC code through a postprocessor.
//!
//! Setup convention: the part lies on the machine with one large face up,
//! its (u, v) axes as machine X and Y and Z = 0 on the top surface. Setup
//! `A` is `front` up and carries every front-face operation, the grooves
//! on the front and the horizontal (edge) drilling. Setup `B`, only when
//! needed, is the part flipped about its X axis so `back` faces up: a
//! back-face (u, v) lands at machine (u, width − v).
//!
//! `profile.workflow` says what the machine gets. `banded_panels`: the part
//! cut and banded, at its finished size; nothing to contour. `nested_router`:
//! the raw panel at its cut size, every face point shifted by the band on
//! the left and bottom edges, the outline cut last in setup A, and the edge
//! drilling in a setup `E` of its own, once the band is on.
//!
//! Tool choice is data (`profile.tools`): drills by exact diameter, an end
//! mill no wider than the groove, the widest compression bit for the
//! contour. What the profile lacks becomes a diagnostic, never a guess.
//!
//! A part longer than the table's X but not its Y is loaded turned 90°
//! (`rotated`): machine (x, y) = (v, length − u). The rule FAB-302 accepts
//! either orientation, so the program has to as well.

use serde::{Deserialize, Serialize};

use crate::diagnostics::{Diagnostic, Diagnostics, Severity};
use crate::geometry::Face;
use crate::library::profile::{ManufacturingProfile, ToolDef, ToolKind, Workflow};
use crate::model::{OpGeometry, Part};
use crate::units::{round3, EPS};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeSide {
    Left,
    Right,
    Bottom,
    Top,
}

/// A machine-independent operation. All coordinates are machine XY on the
/// current setup, depths positive into the material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolOp {
    /// Vertical drilling, pecked when deeper than the tool's pass depth.
    Drill {
        x: f64,
        y: f64,
        depth: f64,
        through: bool,
    },
    /// A straight slot cut in depth passes; the tool runs along the centre
    /// line, offset sideways when narrower than the slot.
    Slot {
        from: [f64; 2],
        to: [f64; 2],
        width: f64,
        depth: f64,
    },
    /// Outline of the blank (the cut size on a router), tool outside,
    /// climb, in passes.
    Contour { length: f64, width: f64, depth: f64 },
    /// A through opening: the tool follows the outline from inside, in
    /// passes, and the slug drops.
    Cutout {
        centre: [f64; 2],
        width: f64,
        height: f64,
        radius: f64,
        depth: f64,
    },
    /// Horizontal drilling into an edge, from outside the part.
    HorizontalDrill {
        side: EdgeSide,
        /// Position along the edge and height above the machine table.
        along: f64,
        height: f64,
        depth: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolOperation {
    pub tool: ToolDef,
    pub source: String,
    pub op: ToolOp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Program {
    pub part: String,
    /// `A` (front up) or `B` (back up).
    pub setup: char,
    pub face_up: Face,
    /// Blank size as it lies on the machine (swapped when `rotated`).
    pub length: f64,
    pub width: f64,
    pub thickness: f64,
    /// The part is loaded turned 90° because it only fits the table that
    /// way: machine (x, y) = (v, part length − u).
    #[serde(default)]
    pub rotated: bool,
    pub operations: Vec<ToolOperation>,
}

impl Program {
    /// Turn every operation 90° for a blank loaded across the table.
    fn rotate(&mut self) {
        let len = self.length;
        let map = |p: [f64; 2]| [p[1], len - p[0]];
        for o in &mut self.operations {
            o.op = match o.op.clone() {
                ToolOp::Drill {
                    x,
                    y,
                    depth,
                    through,
                } => {
                    let [x, y] = map([x, y]);
                    ToolOp::Drill {
                        x,
                        y,
                        depth,
                        through,
                    }
                }
                ToolOp::Slot {
                    from,
                    to,
                    width,
                    depth,
                } => ToolOp::Slot {
                    from: map(from),
                    to: map(to),
                    width,
                    depth,
                },
                ToolOp::Contour {
                    length,
                    width,
                    depth,
                } => ToolOp::Contour {
                    length: width,
                    width: length,
                    depth,
                },
                ToolOp::Cutout {
                    centre,
                    width,
                    height,
                    radius,
                    depth,
                } => ToolOp::Cutout {
                    centre: map(centre),
                    width: height,
                    height: width,
                    radius,
                    depth,
                },
                ToolOp::HorizontalDrill {
                    side,
                    along,
                    height,
                    depth,
                } => {
                    // Left (u = 0) ends up at y = length: the top edge, its
                    // `along` (part v) now running along x unchanged. Bottom
                    // (v = 0) ends up at x = 0, its `along` (part u) now
                    // running down y.
                    let (side, along) = match side {
                        EdgeSide::Left => (EdgeSide::Top, along),
                        EdgeSide::Right => (EdgeSide::Bottom, along),
                        EdgeSide::Bottom => (EdgeSide::Left, len - along),
                        EdgeSide::Top => (EdgeSide::Right, len - along),
                    };
                    ToolOp::HorizontalDrill {
                        side,
                        along,
                        height,
                        depth,
                    }
                }
            };
        }
        std::mem::swap(&mut self.length, &mut self.width);
        self.rotated = true;
    }
}

impl Program {
    pub fn name(&self) -> String {
        format!("{}_{}", self.part, self.setup)
    }
}

fn pick_drill(profile: &ManufacturingProfile, diameter: f64) -> Option<ToolDef> {
    profile
        .tools
        .iter()
        .find(|t| {
            t.kind == ToolKind::Drill
                && (t.diameter - diameter).abs() <= profile.tolerances.hole_diameter
        })
        .cloned()
}

fn pick_end_mill(profile: &ManufacturingProfile, max_diameter: f64) -> Option<ToolDef> {
    profile
        .tools
        .iter()
        .filter(|t| t.kind == ToolKind::EndMill && t.diameter <= max_diameter + EPS)
        .max_by(|a, b| a.diameter.partial_cmp(&b.diameter).unwrap())
        .cloned()
}

/// The widest router bit that turns the corners of a cutout (its radius
/// no more than theirs) and fits well inside it.
fn pick_cutout_tool(
    profile: &ManufacturingProfile,
    width: f64,
    height: f64,
    radius: f64,
) -> Option<ToolDef> {
    profile
        .tools
        .iter()
        .filter(|t| matches!(t.kind, ToolKind::CompressionBit | ToolKind::EndMill))
        .filter(|t| t.diameter / 2.0 <= radius + EPS && t.diameter < width.min(height) - 2.0)
        .max_by(|a, b| a.diameter.partial_cmp(&b.diameter).unwrap())
        .cloned()
}

fn pick_contour_tool(profile: &ManufacturingProfile) -> Option<ToolDef> {
    profile
        .tools
        .iter()
        .filter(|t| matches!(t.kind, ToolKind::CompressionBit | ToolKind::EndMill))
        .max_by(|a, b| {
            // Compression bits first, then the widest.
            let rank = |t: &ToolDef| (matches!(t.kind, ToolKind::CompressionBit) as u8, t.diameter);
            rank(a).partial_cmp(&rank(b)).unwrap()
        })
        .cloned()
}

/// Programs for one part: setup A always, setup B when the back carries
/// operations, setup E (nesting router only) for the edge drilling that
/// waits for the band. Missing tools and capabilities become diagnostics
/// on the part; the operation is then left out of the program.
///
/// What a program starts from follows `profile.workflow`: the finished,
/// banded part (panel saw, then bander, then CNC: no outline to cut), or
/// the raw cut panel on a nesting router (holes shifted by the band on the
/// left and bottom edges, the outline cut at the cut size).
pub fn programs(
    part: &Part,
    profile: &ManufacturingProfile,
    diags: &mut Diagnostics,
) -> Vec<Program> {
    let (len, wid, t) = (part.dims.length, part.dims.width, part.dims.thickness);
    let nested = profile.workflow == Workflow::NestedRouter;
    // Band thickness on each edge: what the raw panel lacks there.
    let band = |face: Face| {
        part.operations
            .iter()
            .filter(|o| o.face == face)
            .filter_map(|o| match o.geometry {
                OpGeometry::EdgeBand { thickness, .. } => Some(thickness),
                _ => None,
            })
            .fold(0.0, f64::max)
    };
    let (bl, bb) = if nested {
        (band(Face::Left), band(Face::Bottom))
    } else {
        (0.0, 0.0)
    };
    let (blank_len, blank_wid) = if nested {
        (len - bl - band(Face::Right), wid - bb - band(Face::Top))
    } else {
        (len, wid)
    };
    let program = |setup: char, face_up: Face, length: f64, width: f64| Program {
        part: part.id.clone(),
        setup,
        face_up,
        length,
        width,
        thickness: t,
        rotated: false,
        operations: Vec::new(),
    };
    let mut a = program('A', Face::Front, blank_len, blank_wid);
    let mut b = program('B', Face::Back, blank_len, blank_wid);
    // After banding, on the finished part.
    let mut e = program('E', Face::Front, len, wid);
    // A face point, finished (u, v), on the blank's machine (x, y).
    let front_xy = |u: f64, v: f64| [u - bl, v - bb];
    let back_xy = |u: f64, v: f64| [u - bl, blank_wid - (v - bb)];
    let missing = |diags: &mut Diagnostics, op_id: &str, what: String| {
        diags.push(
            Diagnostic::new(
                "CAM-201",
                Severity::Error,
                format!("{} / {op_id}: {what}", part.id),
            )
            .entity(part.id.clone())
            .location(op_id.to_string())
            .suggestion("Agregá la herramienta a 'tools' del perfil o cambiá el herraje."),
        );
    };

    for op in &part.operations {
        let source = op.id.clone();
        match &op.geometry {
            OpGeometry::Drill {
                u,
                v,
                diameter,
                depth,
                through,
                ..
            } => {
                // A missing drill is already FAB-206; just leave the hole out.
                let Some(tool) = pick_drill(profile, *diameter) else {
                    continue;
                };
                match op.face {
                    Face::Front | Face::Back => {
                        let depth = if *through {
                            t + 0.5
                        } else {
                            depth.unwrap_or(0.0)
                        };
                        let (prog, [x, y]) = if op.face == Face::Front {
                            (&mut a, front_xy(*u, *v))
                        } else {
                            (&mut b, back_xy(*u, *v))
                        };
                        prog.operations.push(ToolOperation {
                            tool,
                            source,
                            op: ToolOp::Drill {
                                x,
                                y,
                                depth,
                                through: *through,
                            },
                        });
                    }
                    edge => {
                        if !profile.horizontal_drilling {
                            diags.push(
                                Diagnostic::new(
                                    "CAM-202",
                                    Severity::Error,
                                    format!(
                                        "{} / {}: perforación en canto y el perfil '{}' no tiene taladro horizontal",
                                        part.id, op.id, profile.id
                                    ),
                                )
                                .entity(part.id.clone())
                                .location(op.id.clone())
                                .suggestion("Usá un perfil con horizontalDrilling o un herraje que no perfore el canto."),
                            );
                            continue;
                        }
                        let side = match edge {
                            Face::Left => EdgeSide::Left,
                            Face::Right => EdgeSide::Right,
                            Face::Bottom => EdgeSide::Bottom,
                            _ => EdgeSide::Top,
                        };
                        // The face's v (across the thickness) is measured
                        // from the back, i.e. from the table. On a router
                        // the edge is drilled once banded (setup E).
                        let prog = if nested { &mut e } else { &mut a };
                        prog.operations.push(ToolOperation {
                            tool,
                            source,
                            op: ToolOp::HorizontalDrill {
                                side,
                                along: *u,
                                height: *v,
                                depth: depth.unwrap_or(0.0),
                            },
                        });
                    }
                }
            }
            OpGeometry::Groove {
                from,
                to,
                width,
                depth,
            } => {
                let Some(tool) = pick_end_mill(profile, *width) else {
                    missing(
                        diags,
                        &op.id,
                        format!("no hay fresa de ≤ Ø{} para la ranura", round3(*width)),
                    );
                    continue;
                };
                let (prog, flip) = if op.face == Face::Front {
                    (&mut a, false)
                } else {
                    (&mut b, true)
                };
                let map = |p: [f64; 2]| {
                    if flip {
                        back_xy(p[0], p[1])
                    } else {
                        front_xy(p[0], p[1])
                    }
                };
                prog.operations.push(ToolOperation {
                    tool,
                    source,
                    op: ToolOp::Slot {
                        from: map(*from),
                        to: map(*to),
                        width: *width,
                        depth: *depth,
                    },
                });
            }
            OpGeometry::EdgeBand { .. } => {}
            OpGeometry::Cutout {
                u,
                v,
                width,
                height,
                radius,
            } => {
                let Some(tool) = pick_cutout_tool(profile, *width, *height, *radius) else {
                    missing(
                        diags,
                        &op.id,
                        format!(
                            "no hay fresa de radio ≤ {} que entre en el recorte de {}×{}",
                            round3(*radius),
                            round3(*width),
                            round3(*height)
                        ),
                    );
                    continue;
                };
                // Through: from whichever face it is drawn on (the same
                // opening seen from the other side).
                let (prog, centre) = if op.face == Face::Back {
                    (&mut b, back_xy(*u, *v))
                } else {
                    (&mut a, front_xy(*u, *v))
                };
                prog.operations.push(ToolOperation {
                    tool,
                    source,
                    op: ToolOp::Cutout {
                        centre,
                        width: *width,
                        height: *height,
                        radius: *radius,
                        depth: t + 0.5,
                    },
                });
            }
        }
    }

    // On a router the contour comes last in setup A, after every hole is
    // drilled while the blank is still whole, at the cut size (the band
    // makes the rest). A part that comes cut and banded has no outline to
    // cut.
    match (nested, pick_contour_tool(profile)) {
        (false, _) => {}
        (true, Some(tool)) => a.operations.push(ToolOperation {
            tool,
            source: "contour".into(),
            op: ToolOp::Contour {
                length: blank_len,
                width: blank_wid,
                depth: t + 0.5,
            },
        }),
        (true, None) => diags.push(
            Diagnostic::new(
                "CAM-201",
                Severity::Error,
                format!("{}: no hay fresa para el contorno", part.id),
            )
            .entity(part.id.clone()),
        ),
    }

    // Group by tool so the changer works once per tool; the contour keeps
    // its place at the end. Stable sort: same tool, source order.
    for prog in [&mut a, &mut b, &mut e] {
        let contour = prog
            .operations
            .iter()
            .position(|o| matches!(o.op, ToolOp::Contour { .. }))
            .map(|i| prog.operations.remove(i));
        prog.operations.sort_by_key(|o| o.tool.number);
        if let Some(c) = contour {
            prog.operations.push(c);
        }
    }

    // Across the table when that is the only way it fits (FAB-302 already
    // reported the parts that fit neither way).
    let [max_x, max_y] = profile.max_part_size;
    if (len > max_x + EPS || wid > max_y + EPS) && wid <= max_x + EPS && len <= max_y + EPS {
        a.rotate();
        b.rotate();
        e.rotate();
    }

    let mut out = vec![a];
    if !b.operations.is_empty() {
        out.push(b);
    }
    if !e.operations.is_empty() {
        out.push(e);
    }
    out
}

/// A postprocessor turns a machine-independent program into NC text.
pub trait PostProcessor {
    fn id(&self) -> &'static str;
    fn extension(&self) -> &'static str;
    fn render(&self, program: &Program) -> String;
}

/// Plain ISO G-code (G17/G21/G90, G81 drilling, G01 milling). Horizontal
/// drilling is emitted as a commented block: generic ISO has no aggregate
/// commands, a machine-specific post overrides `horizontal_drill`.
pub struct GenericIso {
    pub safe_z: f64,
    pub clearance_z: f64,
}

impl Default for GenericIso {
    fn default() -> Self {
        GenericIso {
            safe_z: 30.0,
            clearance_z: 3.0,
        }
    }
}

fn fmt(v: f64) -> String {
    let r = round3(v);
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        format!("{r:.3}").trim_end_matches('0').to_string()
    }
}

/// Text inside an NC comment: one line, no parentheses or semicolons. A
/// tool id comes from the spec, and a line break in it would otherwise
/// start a move of its own.
fn comment(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '(' | ')' | ';' | '%') && !c.is_control())
        .collect()
}

impl GenericIso {
    fn tool_change(&self, out: &mut String, tool: &ToolDef, current: &mut Option<String>) {
        if current.as_deref() == Some(tool.id.as_str()) {
            return;
        }
        out.push_str(&format!("G00 Z{}\n", fmt(self.safe_z)));
        out.push_str(&format!(
            "(TOOL {} {:?} D{})\n",
            comment(&tool.id),
            tool.kind,
            fmt(tool.diameter)
        ));
        out.push_str(&format!("T{} M06\n", tool.number));
        out.push_str(&format!("S{} M03\n", tool.rpm));
        *current = Some(tool.id.clone());
    }

    fn drill(&self, out: &mut String, tool: &ToolDef, x: f64, y: f64, depth: f64) {
        // Peck when deeper than one pass.
        if depth > tool.max_depth_per_pass + EPS {
            out.push_str(&format!(
                "G83 X{} Y{} Z{} R{} Q{} F{}\n",
                fmt(x),
                fmt(y),
                fmt(-depth),
                fmt(self.clearance_z),
                fmt(tool.max_depth_per_pass),
                fmt(tool.feed_z)
            ));
        } else {
            out.push_str(&format!(
                "G81 X{} Y{} Z{} R{} F{}\n",
                fmt(x),
                fmt(y),
                fmt(-depth),
                fmt(self.clearance_z),
                fmt(tool.feed_z)
            ));
        }
        out.push_str("G80\n");
    }

    fn slot(
        &self,
        out: &mut String,
        tool: &ToolDef,
        from: [f64; 2],
        to: [f64; 2],
        width: f64,
        depth: f64,
    ) {
        // Side offsets so the slot reaches its full width; the centre line
        // is the last pass when the tool is narrower.
        let dx = to[0] - from[0];
        let dy = to[1] - from[1];
        let len = (dx * dx + dy * dy).sqrt().max(EPS);
        let (nx, ny) = (-dy / len, dx / len);
        let extra = width - tool.diameter;
        let offsets: Vec<f64> = if extra <= EPS {
            vec![0.0]
        } else {
            let n = (extra / tool.diameter).ceil() as usize + 1;
            (0..n)
                .map(|i| -extra / 2.0 + extra * i as f64 / (n - 1) as f64)
                .collect()
        };
        let passes = (depth / tool.max_depth_per_pass).ceil().max(1.0) as usize;
        for off in offsets {
            let a = [from[0] + nx * off, from[1] + ny * off];
            let b = [to[0] + nx * off, to[1] + ny * off];
            out.push_str(&format!("G00 Z{}\n", fmt(self.clearance_z)));
            out.push_str(&format!("G00 X{} Y{}\n", fmt(a[0]), fmt(a[1])));
            for k in 1..=passes {
                let z = -(depth * k as f64 / passes as f64);
                // Back and forth: each pass ends where the next one starts.
                let q = if k % 2 == 1 { b } else { a };
                out.push_str(&format!("G01 Z{} F{}\n", fmt(z), fmt(tool.feed_z)));
                out.push_str(&format!(
                    "G01 X{} Y{} F{}\n",
                    fmt(q[0]),
                    fmt(q[1]),
                    fmt(tool.feed_xy)
                ));
            }
            out.push_str(&format!("G00 Z{}\n", fmt(self.clearance_z)));
        }
    }

    fn contour(&self, out: &mut String, tool: &ToolDef, length: f64, width: f64, depth: f64) {
        let r = tool.diameter / 2.0;
        // Climb milling around the outside: counter-clockwise with the
        // tool outside the rectangle.
        let corners = [
            [-r, -r],
            [length + r, -r],
            [length + r, width + r],
            [-r, width + r],
        ];
        let passes = (depth / tool.max_depth_per_pass).ceil().max(1.0) as usize;
        out.push_str(&format!("G00 Z{}\n", fmt(self.clearance_z)));
        out.push_str(&format!(
            "G00 X{} Y{}\n",
            fmt(corners[0][0]),
            fmt(corners[0][1])
        ));
        for k in 1..=passes {
            let z = -(depth * k as f64 / passes as f64);
            out.push_str(&format!("G01 Z{} F{}\n", fmt(z), fmt(tool.feed_z)));
            for c in corners.iter().skip(1).chain(std::iter::once(&corners[0])) {
                out.push_str(&format!(
                    "G01 X{} Y{} F{}\n",
                    fmt(c[0]),
                    fmt(c[1]),
                    fmt(tool.feed_xy)
                ));
            }
        }
        out.push_str(&format!("G00 Z{}\n", fmt(self.safe_z)));
    }

    #[allow(clippy::too_many_arguments)]
    fn cutout(
        &self,
        out: &mut String,
        tool: &ToolDef,
        centre: [f64; 2],
        size: [f64; 2],
        radius: f64,
        depth: f64,
    ) {
        // The outline from inside, one tool radius in, in passes.
        let path = crate::model::cutout_path(
            centre[0],
            centre[1],
            size[0],
            size[1],
            radius,
            tool.diameter / 2.0,
        );
        let passes = (depth / tool.max_depth_per_pass).ceil().max(1.0) as usize;
        out.push_str(&format!("G00 Z{}\n", fmt(self.clearance_z)));
        out.push_str(&format!("G00 X{} Y{}\n", fmt(path[0][0]), fmt(path[0][1])));
        for k in 1..=passes {
            let z = -(depth * k as f64 / passes as f64);
            out.push_str(&format!("G01 Z{} F{}\n", fmt(z), fmt(tool.feed_z)));
            for p in path.iter().skip(1) {
                out.push_str(&format!(
                    "G01 X{} Y{} F{}\n",
                    fmt(p[0]),
                    fmt(p[1]),
                    fmt(tool.feed_xy)
                ));
            }
        }
        out.push_str(&format!("G00 Z{}\n", fmt(self.safe_z)));
    }

    fn horizontal_drill(
        &self,
        out: &mut String,
        tool: &ToolDef,
        side: EdgeSide,
        along: f64,
        height: f64,
        depth: f64,
    ) {
        out.push_str(&format!(
            "(HDRILL {:?} D{} ALONG {} HEIGHT {} DEPTH {} F{})\n",
            side,
            fmt(tool.diameter),
            fmt(along),
            fmt(height),
            fmt(depth),
            fmt(tool.feed_z)
        ));
    }
}

impl PostProcessor for GenericIso {
    fn id(&self) -> &'static str {
        "generic_iso"
    }

    fn extension(&self) -> &'static str {
        "nc"
    }

    fn render(&self, program: &Program) -> String {
        let mut out = String::new();
        out.push_str("%\n");
        out.push_str(&format!(
            "(REWOOD {} SETUP {} FACE_UP {:?} BLANK {}x{}x{} ORIGIN LOWER-LEFT Z0 TOP{})\n",
            comment(&program.part),
            program.setup,
            program.face_up,
            fmt(program.length),
            fmt(program.width),
            fmt(program.thickness),
            if program.rotated { " ROTATED 90" } else { "" }
        ));
        out.push_str("G21 G90 G17 G40 G49\nG54\n");
        let mut current: Option<String> = None;
        for o in &program.operations {
            out.push_str(&format!("({})\n", comment(&o.source)));
            // The horizontal aggregate carries its own drills: loading one
            // in the spindle (and starting it) for an HDRILL block is a
            // tool change for nothing.
            if !matches!(o.op, ToolOp::HorizontalDrill { .. }) {
                self.tool_change(&mut out, &o.tool, &mut current);
            }
            match o.op {
                ToolOp::Drill { x, y, depth, .. } => self.drill(&mut out, &o.tool, x, y, depth),
                ToolOp::Slot {
                    from,
                    to,
                    width,
                    depth,
                } => self.slot(&mut out, &o.tool, from, to, width, depth),
                ToolOp::Contour {
                    length,
                    width,
                    depth,
                } => self.contour(&mut out, &o.tool, length, width, depth),
                ToolOp::Cutout {
                    centre,
                    width,
                    height,
                    radius,
                    depth,
                } => self.cutout(&mut out, &o.tool, centre, [width, height], radius, depth),
                ToolOp::HorizontalDrill {
                    side,
                    along,
                    height,
                    depth,
                } => self.horizontal_drill(&mut out, &o.tool, side, along, height, depth),
            }
        }
        out.push_str(&format!("G00 Z{}\nM05\nM30\n%\n", fmt(self.safe_z)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Libraries;

    #[test]
    fn top_panel_gets_two_setups_and_pecked_cam_holes() {
        let spec = include_str!("../../../fixtures/basic_cabinet/input.json");
        let plan = crate::compile_json(spec);
        let libs = Libraries::default();
        let top = plan.parts.iter().find(|p| p.role == "top").unwrap();
        let mut diags = Diagnostics::default();
        let progs = programs(top, &libs.profile, &mut diags);
        assert!(diags.items.is_empty(), "{:#?}", diags);
        // Everything on the top is on its front (inside) face or its edges:
        // one setup.
        assert_eq!(progs.len(), 1);
        let a = &progs[0];
        let drills = a
            .operations
            .iter()
            .filter(|o| matches!(o.op, ToolOp::Drill { .. }))
            .count();
        let hdrills = a
            .operations
            .iter()
            .filter(|o| matches!(o.op, ToolOp::HorizontalDrill { .. }))
            .count();
        let slots = a
            .operations
            .iter()
            .filter(|o| matches!(o.op, ToolOp::Slot { .. }))
            .count();
        assert_eq!((drills, hdrills, slots), (4, 8, 1));
        // Cut and banded before it gets here: no outline to cut.
        assert!(!a
            .operations
            .iter()
            .any(|o| matches!(o.op, ToolOp::Contour { .. })));

        let nc = GenericIso::default().render(a);
        assert!(nc.starts_with("%\n(REWOOD P003 SETUP A"));
        assert!(nc.contains("G83")); // Ø15 cam 12.5 deep: pecked with a 10 mm pass
        assert!(nc.contains("(HDRILL Left D8"));
        assert!(nc.ends_with("M30\n%\n"));
        assert_eq!(nc, GenericIso::default().render(a));
    }

    #[test]
    fn a_nesting_router_works_the_raw_panel_and_drills_edges_after_banding() {
        let spec = include_str!("../../../fixtures/basic_cabinet/input.json");
        let plan = crate::compile_json(spec);
        let mut profile = Libraries::default().profile;
        profile.workflow = Workflow::NestedRouter;
        // The top: banded on its front edge (its local bottom).
        let top = plan.parts.iter().find(|p| p.role == "top").unwrap();
        let band = 1.0;
        let mut diags = Diagnostics::default();
        let progs = programs(top, &profile, &mut diags);
        assert!(diags.items.is_empty(), "{:#?}", diags);
        let a = &progs[0];
        // The blank is the cut size, not the finished one.
        assert_eq!((a.length, a.width), (top.cut.length, top.cut.width));
        // The outline is cut last, at the cut size.
        assert!(matches!(
            a.operations.last().unwrap().op,
            ToolOp::Contour { length, width, .. }
                if length == top.cut.length && width == top.cut.width
        ));
        // A hole keeps its distance to the unbanded edges and moves by
        // the band towards the banded one.
        let hole = top
            .operations
            .iter()
            .find_map(|o| match (o.face, &o.geometry) {
                (Face::Front, OpGeometry::Drill { u, v, .. }) => Some((o.id.clone(), *u, *v)),
                _ => None,
            })
            .unwrap();
        let drilled = a.operations.iter().find(|o| o.source == hole.0).unwrap();
        let ToolOp::Drill { x, y, .. } = drilled.op else {
            panic!()
        };
        assert_eq!((x, y), (hole.1, hole.2 - band));
        // Edge drilling waits for the band: its own setup, on the finished part.
        let e = progs.iter().find(|p| p.setup == 'E').unwrap();
        assert_eq!((e.length, e.width), (top.dims.length, top.dims.width));
        assert!(e
            .operations
            .iter()
            .all(|o| matches!(o.op, ToolOp::HorizontalDrill { .. })));
        assert!(!a
            .operations
            .iter()
            .any(|o| matches!(o.op, ToolOp::HorizontalDrill { .. })));
    }

    #[test]
    fn drawer_side_flips_for_its_back_groove() {
        let spec = include_str!("../../../fixtures/drawer_unit/input.json");
        let plan = crate::compile_json(spec);
        let libs = Libraries::default();
        let side = plan
            .parts
            .iter()
            .find(|p| p.role == "drawer_1_side_left")
            .unwrap();
        let mut diags = Diagnostics::default();
        let progs = programs(side, &libs.profile, &mut diags);
        assert_eq!(progs.len(), 2);
        let b = &progs[1];
        assert_eq!(b.setup, 'B');
        let slot = b
            .operations
            .iter()
            .find(|o| matches!(o.op, ToolOp::Slot { .. }))
            .unwrap();
        if let ToolOp::Slot { from, .. } = slot.op {
            // Groove at v = 11.6 on the back lands at width − 11.6 once flipped.
            assert!((from[1] - (side.dims.width - 11.6)).abs() < 1e-6);
        }
    }

    #[test]
    fn a_part_too_long_for_x_is_loaded_across_the_table() {
        let spec = include_str!("../../../fixtures/wardrobe_1800/input.json");
        let plan = crate::compile_json(spec);
        let libs = Libraries::default();
        // The back: 1779 × 2079 on a 2800 × 2070 table only fits turned.
        let back = plan.parts.iter().find(|p| p.role == "back").unwrap();
        assert!(back.dims.width > libs.profile.max_part_size[1]);
        let mut diags = Diagnostics::default();
        let progs = programs(back, &libs.profile, &mut diags);
        assert!(progs[0].rotated);
        assert_eq!(progs[0].length, back.dims.width);
        assert_eq!(progs[0].width, back.dims.length);
        let nc = GenericIso::default().render(&progs[0]);
        assert!(nc.contains("ROTATED 90"));
        // A side panel fits as is.
        let side = plan.parts.iter().find(|p| p.role == "side_left").unwrap();
        assert!(!programs(side, &libs.profile, &mut diags)[0].rotated);
    }

    #[test]
    fn a_profile_without_horizontal_drilling_reports_edge_holes() {
        let spec = include_str!("../../../fixtures/basic_cabinet/input.json");
        let plan = crate::compile_json(spec);
        let mut profile = Libraries::default().profile;
        profile.horizontal_drilling = false;
        let top = plan.parts.iter().find(|p| p.role == "top").unwrap();
        let mut diags = Diagnostics::default();
        programs(top, &profile, &mut diags);
        assert_eq!(diags.count(Severity::Error), 8);
        assert!(diags.items.iter().all(|d| d.code == "CAM-202"));
    }
}

#[cfg(test)]
mod gcode_tests {
    use super::*;
    use crate::library::Libraries;

    /// Walk every program of the wardrobe and check the tool never leaves
    /// the blank plus one tool radius, never cuts deeper than the blank,
    /// and only feeds (G01) below the surface.
    #[test]
    fn gcode_stays_inside_the_envelope() {
        let spec = include_str!("../../../fixtures/wardrobe_1800/input.json");
        let plan = crate::compile_json(spec);
        let libs = Libraries::default();
        let post = GenericIso::default();
        let mut programs = 0;
        for part in &plan.parts {
            let mut diags = Diagnostics::default();
            for program in super::programs(part, &libs.profile, &mut diags) {
                programs += 1;
                let nc = post.render(&program);
                let (mut x, mut y, mut z) = (0.0_f64, 0.0_f64, 30.0_f64);
                let r = 6.0 + EPS;
                for line in nc.lines().filter(|l| l.starts_with('G')) {
                    let mut feed = false;
                    for word in line.split_whitespace() {
                        let (k, v) = word.split_at(1);
                        let val = v.parse::<f64>().ok();
                        match (k, val) {
                            ("X", Some(v)) => x = v,
                            ("Y", Some(v)) => y = v,
                            ("Z", Some(v)) => z = v,
                            ("G", Some(1.0)) => feed = true,
                            _ => {}
                        }
                    }
                    assert!(
                        x >= -r && x <= program.length + r,
                        "{}: X {x} out of blank",
                        program.name()
                    );
                    assert!(
                        y >= -r && y <= program.width + r,
                        "{}: Y {y} out of blank",
                        program.name()
                    );
                    assert!(
                        z >= -(program.thickness + 0.5) - EPS,
                        "{}: Z {z} too deep",
                        program.name()
                    );
                    if feed && line.contains('X') {
                        assert!(
                            z <= 0.0,
                            "{}: feeding above the surface: {line}",
                            program.name()
                        );
                    }
                }
            }
        }
        assert!(programs >= plan.parts.len());
    }
}
