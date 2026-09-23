//! Machining simulation (§28, levels 2 and 3): the NC text a postprocessor
//! produced is interpreted like a controller would, move by move, and what
//! it would cut is checked against the machine-independent program it was
//! rendered from and against the machine's limits.
//!
//! The point is to verify the *post*, not the plan: the plan's own rules
//! (FAB-2xx) already checked holes against faces and thickness. Here the
//! questions are whether the G-code rapids into material, feeds without a
//! spindle, leaves the travel, goes through the table, and whether every
//! operation ends up cut exactly once at its position and depth. A machine
//! post that drops a hole or doubles a depth fails here, on every plan,
//! before any panel is loaded.
//!
//! The interpreter covers the ISO subset the posts use: G00/G01, G81/G83
//! drilling cycles with R and Q, G80, T/M06, S/M03, M05, M30, and the
//! `(HDRILL …)` comment blocks that stand in for a horizontal aggregate.
//! Anything else is reported, not guessed.

use serde::{Deserialize, Serialize};

use crate::cam::{EdgeSide, Program, ToolOp};
use crate::diagnostics::{Diagnostic, Diagnostics, Severity};
use crate::library::profile::{ManufacturingProfile, ToolDef};
use crate::units::{round3, EPS};

/// Machine envelope and kinematics used by the simulation. Part of the
/// profile; every field has a default so an older profile still loads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineLimits {
    /// Highest Z the head can go to, above the blank's top surface.
    pub travel_z: f64,
    /// How far below the blank's underside a cut may go (into the spoil
    /// board) before it counts as hitting the table.
    pub max_cut_below_blank: f64,
    /// mm/min for G00.
    pub rapid_feed: f64,
    pub tool_change_seconds: f64,
    /// Machine cost per hour, in the profile's currency; 0 = unknown.
    #[serde(default, skip_serializing_if = "crate::library::material::is_zero")]
    pub hourly_rate: f64,
    /// Clamps, stops and pods the tool must never enter: level 3
    /// "fijaciones". Machine coordinates of the setup (blank origin).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixtures: Vec<Fixture>,
}

impl Default for MachineLimits {
    fn default() -> Self {
        MachineLimits {
            travel_z: 150.0,
            max_cut_below_blank: 1.0,
            rapid_feed: 20000.0,
            tool_change_seconds: 8.0,
            hourly_rate: 0.0,
            fixtures: Vec::new(),
        }
    }
}

/// An obstacle on the table: a rectangle in XY and the Z its top reaches,
/// relative to the blank's top surface (a clamp bar +40, a vacuum pod
/// under the blank −18). The tool collides when its tip is at or below
/// `top` while its footprint overlaps the rectangle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fixture {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub length: f64,
    pub width: f64,
    pub top: f64,
}

impl Fixture {
    /// Does a move from `a` to `b` (tool radius `r`) cross this fixture
    /// below its top? Both ends' Z count: the lower one decides.
    fn hit(&self, a: [f64; 3], b: [f64; 3], r: f64) -> bool {
        if a[2].min(b[2]) > self.top + EPS {
            return false;
        }
        // Liang–Barsky against the rectangle grown by the tool radius.
        let (x0, y0, x1, y1) = (
            self.x - r,
            self.y - r,
            self.x + self.length + r,
            self.y + self.width + r,
        );
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
        for (p, q) in [
            (-dx, a[0] - x0),
            (dx, x1 - a[0]),
            (-dy, a[1] - y0),
            (dy, y1 - a[1]),
        ] {
            if p.abs() < EPS {
                if q < 0.0 {
                    return false;
                }
            } else {
                let t = q / p;
                if p < 0.0 {
                    t0 = t0.max(t);
                } else {
                    t1 = t1.min(t);
                }
            }
        }
        t0 <= t1
    }
}

/// Material the interpreted NC would remove.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Cut {
    /// A drilling cycle: tool tip reaches `-depth` at (x, y).
    Hole {
        tool: String,
        x: f64,
        y: f64,
        depth: f64,
    },
    /// A feed move at or below the surface, from `from` to `to`.
    Segment {
        tool: String,
        from: [f64; 3],
        to: [f64; 3],
    },
    /// Horizontal drilling reported by the post's aggregate block.
    Horizontal {
        tool_diameter: f64,
        side: EdgeSide,
        along: f64,
        height: f64,
        depth: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Simulation {
    pub program: String,
    pub lines: usize,
    pub tool_changes: usize,
    pub rapid_mm: f64,
    pub cut_mm: f64,
    /// Feed, rapid and tool-change time from the profile's feeds.
    pub seconds: f64,
    pub cuts: Vec<Cut>,
    /// Operations of the program matched by a cut, in program order.
    pub operations_verified: usize,
}

struct State<'a> {
    x: f64,
    y: f64,
    z: f64,
    tool: Option<&'a ToolDef>,
    spindle: bool,
    feed: f64,
    /// Modal motion: 0 rapid, 1 feed, 81/83 drilling cycle, none.
    motion: Option<u32>,
    cycle_r: f64,
    cycle_z: f64,
    cycle_q: f64,
    rapid_mm: f64,
    cut_mm: f64,
    seconds: f64,
    tool_changes: usize,
    cuts: Vec<Cut>,
}

fn parse_hdrill(comment: &str) -> Option<Cut> {
    // (HDRILL Left D8 ALONG 50 HEIGHT 9 DEPTH 34 F800)
    let words: Vec<&str> = comment.split_whitespace().collect();
    if words.first() != Some(&"HDRILL") || words.len() < 9 {
        return None;
    }
    let side = match words[1] {
        "Left" => EdgeSide::Left,
        "Right" => EdgeSide::Right,
        "Bottom" => EdgeSide::Bottom,
        "Top" => EdgeSide::Top,
        _ => return None,
    };
    let num = |s: &str| s.parse::<f64>().ok();
    Some(Cut::Horizontal {
        tool_diameter: num(words[2].trim_start_matches('D'))?,
        side,
        along: num(words[4])?,
        height: num(words[6])?,
        depth: num(words[8])?,
    })
}

/// Interpret one program's NC and check it. Findings go to `diags` with
/// the part as entity and the program name as location.
pub fn simulate(
    program: &Program,
    nc: &str,
    profile: &ManufacturingProfile,
    diags: &mut Diagnostics,
) -> Simulation {
    let machine = &profile.machine;
    let name = program.name();
    let floor = -(program.thickness + machine.max_cut_below_blank);
    let mut st = State {
        x: 0.0,
        y: 0.0,
        z: machine.travel_z,
        tool: None,
        spindle: false,
        feed: 0.0,
        motion: None,
        cycle_r: 0.0,
        cycle_z: 0.0,
        cycle_q: 0.0,
        rapid_mm: 0.0,
        cut_mm: 0.0,
        seconds: 0.0,
        tool_changes: 0,
        cuts: Vec::new(),
    };
    let mut lines = 0;
    let mut findings: Vec<(&'static str, String)> = Vec::new();
    let mut ended = false;

    for (n, raw) in nc.lines().enumerate() {
        let line_no = n + 1;
        let line = raw.trim();
        if line.is_empty() || line == "%" {
            continue;
        }
        lines += 1;
        if let Some(inner) = line.strip_prefix('(') {
            let inner = inner.trim_end_matches(')');
            if let Some(cut) = parse_hdrill(inner) {
                if !profile.horizontal_drilling {
                    findings.push((
                        "CAM-303",
                        format!("línea {line_no}: taladro horizontal en un perfil sin agregado"),
                    ));
                }
                if let Cut::Horizontal { depth, .. } = &cut {
                    let tool_feed = st.tool.map(|t| t.feed_z).unwrap_or(800.0);
                    st.seconds += 2.0 * depth / tool_feed * 60.0;
                    st.cut_mm += depth;
                }
                st.cuts.push(cut);
            }
            continue;
        }
        if ended {
            findings.push(("CAM-303", format!("línea {line_no}: código después de M30")));
            break;
        }

        // Words of the block.
        let (mut gx, mut gy, mut gz) = (None, None, None);
        let (mut r, mut q, mut f, mut s, mut t) = (None, None, None, None, None);
        let mut gcodes: Vec<u32> = Vec::new();
        let mut mcodes: Vec<u32> = Vec::new();
        let mut bad = false;
        for word in line.split_whitespace() {
            let (k, v) = word.split_at(1);
            let val = v.parse::<f64>();
            match (k, val) {
                ("G", Ok(v)) => gcodes.push(v as u32),
                ("M", Ok(v)) => mcodes.push(v as u32),
                ("X", Ok(v)) => gx = Some(v),
                ("Y", Ok(v)) => gy = Some(v),
                ("Z", Ok(v)) => gz = Some(v),
                ("R", Ok(v)) => r = Some(v),
                ("Q", Ok(v)) => q = Some(v),
                ("F", Ok(v)) => f = Some(v),
                ("S", Ok(v)) => s = Some(v),
                ("T", Ok(v)) => t = Some(v as u32),
                _ => bad = true,
            }
        }
        if bad {
            findings.push((
                "CAM-303",
                format!("línea {line_no}: palabra desconocida en '{line}'"),
            ));
            continue;
        }
        if let Some(f) = f {
            st.feed = f;
        }
        for g in &gcodes {
            match g {
                0 | 1 | 81 | 83 => st.motion = Some(*g),
                80 => st.motion = None,
                17 | 21 | 40 | 49 | 54 | 90 => {}
                other => findings.push((
                    "CAM-303",
                    format!("línea {line_no}: G{other} no está en el subconjunto simulado"),
                )),
            }
        }
        for m in &mcodes {
            match m {
                6 => {
                    let Some(number) = t else {
                        findings.push((
                            "CAM-303",
                            format!("línea {line_no}: M06 sin número de herramienta"),
                        ));
                        continue;
                    };
                    match profile.tools.iter().find(|tool| tool.number == number) {
                        Some(tool) => {
                            if st.z < 0.0 {
                                findings.push((
                                    "CAM-301",
                                    format!("línea {line_no}: cambio de herramienta con el husillo dentro del material (Z {})", round3(st.z)),
                                ));
                            }
                            st.tool = Some(tool);
                            st.tool_changes += 1;
                            st.seconds += machine.tool_change_seconds;
                        }
                        None => findings.push((
                            "CAM-303",
                            format!(
                                "línea {line_no}: T{number} no existe en el perfil '{}'",
                                profile.id
                            ),
                        )),
                    }
                }
                3 => st.spindle = true,
                5 => st.spindle = false,
                30 => ended = true,
                other => findings.push((
                    "CAM-303",
                    format!("línea {line_no}: M{other} no está en el subconjunto simulado"),
                )),
            }
        }
        let _ = s;

        let has_xyz = gx.is_some() || gy.is_some() || gz.is_some();
        match st.motion {
            Some(81) | Some(83) if has_xyz => {
                // A drilling cycle block: rapid to (x, y) at the retract
                // plane, feed to Z, retract.
                let Some(tool) = st.tool else {
                    findings.push((
                        "CAM-302",
                        format!("línea {line_no}: ciclo de taladrado sin herramienta"),
                    ));
                    continue;
                };
                if !st.spindle {
                    findings.push((
                        "CAM-302",
                        format!("línea {line_no}: ciclo de taladrado con el husillo parado"),
                    ));
                }
                if let Some(v) = r {
                    st.cycle_r = v;
                }
                if let Some(v) = gz {
                    st.cycle_z = v;
                }
                if let Some(v) = q {
                    st.cycle_q = v;
                }
                let (x, y) = (gx.unwrap_or(st.x), gy.unwrap_or(st.y));
                for f in &machine.fixtures {
                    let r = tool.diameter / 2.0;
                    let travel = st.z.max(st.cycle_r);
                    if f.hit([st.x, st.y, travel], [x, y, travel], r)
                        || f.hit([x, y, st.cycle_z], [x, y, st.cycle_z], r)
                    {
                        findings.push((
                            "CAM-308",
                            format!("línea {line_no}: el taladro en ({}, {}) cae sobre la fijación '{}'", round3(x), round3(y), f.name),
                        ));
                    }
                }
                if st.cycle_r <= 0.0 {
                    findings.push((
                        "CAM-301",
                        format!(
                            "línea {line_no}: plano de retroceso R {} dentro del material",
                            round3(st.cycle_r)
                        ),
                    ));
                }
                // Rapid across at the current Z (the controller moves to R
                // first only if lower), then down to R.
                let travel_z = st.z.max(st.cycle_r);
                if travel_z <= 0.0 {
                    findings.push((
                        "CAM-301",
                        format!(
                            "línea {line_no}: rápido en XY a Z {} dentro del material",
                            round3(travel_z)
                        ),
                    ));
                }
                let dx = x - st.x;
                let dy = y - st.y;
                let d = (dx * dx + dy * dy).sqrt() + (st.z - st.cycle_r).abs();
                st.rapid_mm += d;
                st.seconds += d / machine.rapid_feed * 60.0;
                let depth = -st.cycle_z;
                if st.cycle_z < floor - EPS {
                    findings.push((
                        "CAM-305",
                        format!(
                            "línea {line_no}: taladro a Z {} atraviesa la mesa (piso {})",
                            round3(st.cycle_z),
                            round3(floor)
                        ),
                    ));
                }
                // One bite per pass: G81 goes the whole depth at once, G83
                // by Q; either may not take more than the tool allows.
                let bite = if st.motion == Some(83) && st.cycle_q > EPS {
                    st.cycle_q
                } else {
                    depth
                };
                if bite > tool.max_depth_per_pass + 0.01 {
                    findings.push((
                        "CAM-309",
                        format!(
                            "línea {line_no}: {} baja {} mm de una vez y admite {}",
                            tool.id,
                            round3(bite),
                            round3(tool.max_depth_per_pass)
                        ),
                    ));
                }
                let plunge = st.cycle_r - st.cycle_z;
                let feed = if st.feed > 0.0 { st.feed } else { tool.feed_z };
                let pecks = if st.motion == Some(83) && st.cycle_q > EPS {
                    (depth / st.cycle_q).ceil().max(1.0)
                } else {
                    1.0
                };
                // Each peck retracts to R and comes back: the extra travel
                // is a rapid.
                st.seconds +=
                    plunge / feed * 60.0 + (pecks - 1.0) * 2.0 * plunge / machine.rapid_feed * 60.0;
                st.cut_mm += depth.max(0.0);
                st.cuts.push(Cut::Hole {
                    tool: tool.id.clone(),
                    x,
                    y,
                    depth,
                });
                st.x = x;
                st.y = y;
                st.z = st.cycle_r;
            }
            Some(0) | Some(1) if has_xyz => {
                let rapid = st.motion == Some(0);
                let (x, y, z) = (gx.unwrap_or(st.x), gy.unwrap_or(st.y), gz.unwrap_or(st.z));
                let (dx, dy, dz) = (x - st.x, y - st.y, z - st.z);
                let d = (dx * dx + dy * dy + dz * dz).sqrt();
                if z > machine.travel_z + EPS {
                    findings.push((
                        "CAM-304",
                        format!(
                            "línea {line_no}: Z {} supera la carrera ({})",
                            round3(z),
                            round3(machine.travel_z)
                        ),
                    ));
                }
                let r = st.tool.map(|t| t.diameter / 2.0).unwrap_or(0.0) + 1.0;
                for f in &machine.fixtures {
                    if f.hit([st.x, st.y, st.z], [x, y, z], r - 1.0) {
                        findings.push((
                            "CAM-308",
                            format!(
                                "línea {line_no}: la herramienta entra en la fijación '{}' ({}, {}) → ({}, {}) a Z {}",
                                f.name,
                                round3(st.x),
                                round3(st.y),
                                round3(x),
                                round3(y),
                                round3(z.min(st.z))
                            ),
                        ));
                    }
                }
                if x < -r - EPS
                    || y < -r - EPS
                    || x > profile.max_part_size[0] + r + EPS
                    || y > profile.max_part_size[1] + r + EPS
                {
                    findings.push((
                        "CAM-304",
                        format!(
                            "línea {line_no}: ({}, {}) fuera de la mesa de {}×{}",
                            round3(x),
                            round3(y),
                            profile.max_part_size[0],
                            profile.max_part_size[1]
                        ),
                    ));
                }
                if rapid {
                    // A rapid may descend to the clearance plane but never
                    // into the material, and never move sideways below it.
                    if z < 0.0 || (st.z <= 0.0 && (dx.abs() > EPS || dy.abs() > EPS)) {
                        findings.push(("CAM-301", format!("línea {line_no}: rápido dentro del material: G00 hacia ({}, {}, {})", round3(x), round3(y), round3(z))));
                    }
                    st.rapid_mm += d;
                    st.seconds += d / machine.rapid_feed * 60.0;
                } else {
                    let Some(tool) = st.tool else {
                        findings.push((
                            "CAM-302",
                            format!("línea {line_no}: avance sin herramienta"),
                        ));
                        continue;
                    };
                    if !st.spindle {
                        findings.push((
                            "CAM-302",
                            format!("línea {line_no}: avance con el husillo parado"),
                        ));
                    }
                    if z < floor - EPS {
                        findings.push((
                            "CAM-305",
                            format!(
                                "línea {line_no}: Z {} atraviesa la mesa (piso {})",
                                round3(z),
                                round3(floor)
                            ),
                        ));
                    }
                    // A descent into the material is a new pass: no deeper
                    // than the tool takes in one.
                    let descent = st.z.min(0.0) - z;
                    if descent > tool.max_depth_per_pass + 0.01 {
                        findings.push((
                            "CAM-309",
                            format!(
                                "línea {line_no}: {} baja {} mm en una pasada y admite {}",
                                tool.id,
                                round3(descent),
                                round3(tool.max_depth_per_pass)
                            ),
                        ));
                    }
                    let feed = if st.feed > 0.0 {
                        st.feed
                    } else {
                        findings.push(("CAM-302", format!("línea {line_no}: G01 sin avance F")));
                        tool.feed_xy
                    };
                    st.seconds += d / feed * 60.0;
                    if z <= EPS || st.z <= EPS {
                        st.cut_mm += d;
                        st.cuts.push(Cut::Segment {
                            tool: tool.id.clone(),
                            from: [st.x, st.y, st.z],
                            to: [x, y, z],
                        });
                    }
                }
                st.x = x;
                st.y = y;
                st.z = z;
            }
            _ if has_xyz => findings.push((
                "CAM-303",
                format!("línea {line_no}: coordenadas sin modo de movimiento"),
            )),
            _ => {}
        }
    }
    if !ended {
        findings.push(("CAM-303", "el programa no termina en M30".into()));
    }
    if st.z < 0.0 {
        findings.push((
            "CAM-301",
            format!(
                "el programa termina con la herramienta a Z {}",
                round3(st.z)
            ),
        ));
    }

    // Level 2, material removed: every operation of the program has to be
    // cut once, at its position and depth, and nothing else may be cut.
    let verified = verify_operations(program, &st.cuts, profile, &mut findings);

    // One finding per code for the per-move checks: a part that leaves the
    // table does so on every move, and one line says it.
    let mut seen: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    let mut collapsed: Vec<(&str, String)> = Vec::new();
    for (code, msg) in findings {
        if matches!(code, "CAM-301" | "CAM-304" | "CAM-305" | "CAM-308") {
            match seen.get(code) {
                Some(&i) => {
                    let entry = &mut collapsed[i].1;
                    let n = entry
                        .rsplit(" (y ")
                        .next()
                        .and_then(|t| t.strip_suffix(" más)"))
                        .and_then(|t| t.parse::<usize>().ok())
                        .unwrap_or(0);
                    if n > 0 {
                        *entry = entry[..entry.rfind(" (y ").unwrap()].to_string();
                    }
                    entry.push_str(&format!(" (y {} más)", n + 1));
                    continue;
                }
                None => {
                    seen.insert(code, collapsed.len());
                }
            }
        }
        collapsed.push((code, msg));
    }
    for (code, msg) in collapsed {
        // A rapid into the material or a move off the table crashes the
        // machine: that program must not reach the workshop.
        let severity = if matches!(code, "CAM-301" | "CAM-304") {
            Severity::Fatal
        } else {
            Severity::Error
        };
        diags.push(
            Diagnostic::new(code, severity, format!("{name}: {msg}"))
                .entity(program.part.clone())
                .location(name.clone()),
        );
    }
    Simulation {
        program: name,
        lines,
        tool_changes: st.tool_changes,
        rapid_mm: round3(st.rapid_mm),
        cut_mm: round3(st.cut_mm),
        seconds: round3(st.seconds),
        cuts: st
            .cuts
            .into_iter()
            .map(|c| match c {
                Cut::Hole { tool, x, y, depth } => Cut::Hole {
                    tool,
                    x: round3(x),
                    y: round3(y),
                    depth: round3(depth),
                },
                Cut::Segment { tool, from, to } => Cut::Segment {
                    tool,
                    from: from.map(round3),
                    to: to.map(round3),
                },
                other => other,
            })
            .collect(),
        operations_verified: verified,
    }
}

fn verify_operations(
    program: &Program,
    cuts: &[Cut],
    profile: &ManufacturingProfile,
    findings: &mut Vec<(&'static str, String)>,
) -> usize {
    let pos_tol = profile.tolerances.hole_position.max(0.01);
    let mut used = vec![false; cuts.len()];
    let mut verified = 0;
    for o in &program.operations {
        let ok = match &o.op {
            ToolOp::Drill { x, y, depth, .. } => {
                let found = cuts.iter().enumerate().find(|(i, c)| {
                    !used[*i]
                        && matches!(c, Cut::Hole { tool, x: cx, y: cy, depth: cd }
                            if *tool == o.tool.id
                                && (cx - x).abs() <= pos_tol
                                && (cy - y).abs() <= pos_tol
                                && (cd - depth).abs() <= 0.01)
                });
                match found {
                    Some((i, _)) => {
                        used[i] = true;
                        true
                    }
                    None => {
                        findings.push((
                            "CAM-306",
                            format!(
                                "{}: la perforación Ø{} en ({}, {}) a {} mm no aparece en el NC",
                                o.source,
                                round3(o.tool.diameter),
                                round3(*x),
                                round3(*y),
                                round3(*depth)
                            ),
                        ));
                        false
                    }
                }
            }
            ToolOp::Slot {
                from,
                to,
                width,
                depth,
            } => {
                // The deepest pass has to run the full length of the slot,
                // within half a width of its centre line.
                let half = width / 2.0 + EPS;
                let full = cuts.iter().enumerate().filter(|(i, c)| {
                    !used[*i]
                        && matches!(c, Cut::Segment { tool, from: a, to: b }
                            if *tool == o.tool.id
                                && (a[2] + depth).abs() <= 0.01
                                && (b[2] + depth).abs() <= 0.01
                                && covers(from, to, [a[0], a[1]], [b[0], b[1]], half))
                });
                let idx: Vec<usize> = full.map(|(i, _)| i).collect();
                // The full-depth passes, side by side, have to take the
                // whole width, and none may cut past it.
                let r = o.tool.diameter / 2.0;
                let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
                let len = dx.hypot(dy).max(EPS);
                let (nx, ny) = (-dy / len, dx / len);
                let mut bands: Vec<(f64, f64)> = idx
                    .iter()
                    .filter_map(|i| match &cuts[*i] {
                        Cut::Segment { from: a, .. } => {
                            let off = (a[0] - from[0]) * nx + (a[1] - from[1]) * ny;
                            Some((off - r, off + r))
                        }
                        _ => None,
                    })
                    .collect();
                bands.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut reach = -width / 2.0;
                let mut gap_left = false;
                for (lo, hi) in &bands {
                    if *lo > reach + 0.01 {
                        gap_left = true;
                    }
                    reach = reach.max(*hi);
                }
                let narrow = !idx.is_empty() && (gap_left || reach < width / 2.0 - 0.01);
                let wide = bands
                    .iter()
                    .any(|(lo, hi)| *lo < -width / 2.0 - 0.01 || *hi > width / 2.0 + 0.01);
                if narrow || wide {
                    findings.push((
                        "CAM-306",
                        format!(
                            "{}: la ranura de {} mm queda {} en el NC",
                            o.source,
                            round3(*width),
                            if wide { "más ancha" } else { "más angosta" }
                        ),
                    ));
                }
                if idx.is_empty() {
                    findings.push((
                        "CAM-306",
                        format!(
                            "{}: la ranura {:?}→{:?} a {} mm no se corta entera en el NC",
                            o.source,
                            from.map(round3),
                            to.map(round3),
                            round3(*depth)
                        ),
                    ));
                    false
                } else {
                    for i in idx {
                        used[i] = true;
                    }
                    true
                }
            }
            ToolOp::Contour {
                length,
                width,
                depth,
            } => {
                // Four sides at the final depth, tool outside the rectangle.
                let r = o.tool.diameter / 2.0;
                let sides = [
                    ([-r, -r], [length + r, -r]),
                    ([length + r, -r], [length + r, width + r]),
                    ([length + r, width + r], [-r, width + r]),
                    ([-r, width + r], [-r, -r]),
                ];
                let mut all = true;
                for (p, q) in sides {
                    let found = cuts.iter().enumerate().find(|(i, c)| {
                        !used[*i]
                            && matches!(c, Cut::Segment { tool, from: a, to: b }
                                if *tool == o.tool.id
                                    && (a[2] + depth).abs() <= 0.01
                                    && (b[2] + depth).abs() <= 0.01
                                    && covers(&p, &q, [a[0], a[1]], [b[0], b[1]], 0.01))
                    });
                    match found {
                        Some((i, _)) => used[i] = true,
                        None => all = false,
                    }
                }
                if !all {
                    findings.push((
                        "CAM-306",
                        format!(
                            "contorno {}×{} a {} mm: el NC no recorre los cuatro lados a la profundidad final",
                            round3(*length),
                            round3(*width),
                            round3(*depth)
                        ),
                    ));
                }
                all
            }
            ToolOp::Cutout {
                centre,
                width,
                height,
                radius,
                depth,
            } => {
                // Every edge of the outline, one tool radius in, at the
                // final depth.
                let path = crate::model::cutout_path(
                    centre[0],
                    centre[1],
                    *width,
                    *height,
                    *radius,
                    o.tool.diameter / 2.0,
                );
                let mut all = true;
                for w in path.windows(2) {
                    let found = cuts.iter().enumerate().find(|(i, c)| {
                        !used[*i]
                            && matches!(c, Cut::Segment { tool, from: a, to: b }
                                if *tool == o.tool.id
                                    && (a[2] + depth).abs() <= 0.01
                                    && (b[2] + depth).abs() <= 0.01
                                    && covers(&w[0], &w[1], [a[0], a[1]], [b[0], b[1]], 0.01))
                    });
                    match found {
                        Some((i, _)) => used[i] = true,
                        None => all = false,
                    }
                }
                if !all {
                    findings.push((
                        "CAM-306",
                        format!(
                            "{}: el recorte de {}×{} en ({}, {}) no se corta entero en el NC",
                            o.source,
                            round3(*width),
                            round3(*height),
                            round3(centre[0]),
                            round3(centre[1])
                        ),
                    ));
                }
                all
            }
            ToolOp::HorizontalDrill {
                side,
                along,
                height,
                depth,
            } => {
                let found = cuts.iter().enumerate().find(|(i, c)| {
                    !used[*i]
                        && matches!(c, Cut::Horizontal { tool_diameter, side: s, along: a, height: h, depth: d }
                            if (tool_diameter - o.tool.diameter).abs() <= 0.01
                                && s == side
                                && (a - along).abs() <= pos_tol
                                && (h - height).abs() <= pos_tol
                                && (d - depth).abs() <= 0.01)
                });
                match found {
                    Some((i, _)) => {
                        used[i] = true;
                        true
                    }
                    None => {
                        findings.push((
                            "CAM-306",
                            format!(
                                "{}: el taladro horizontal {:?} en {} / {} a {} mm no aparece en el NC",
                                o.source,
                                side,
                                round3(*along),
                                round3(*height),
                                round3(*depth)
                            ),
                        ));
                        false
                    }
                }
            }
        };
        if ok {
            verified += 1;
        }
    }
    // Cuts nobody asked for: holes, edge drilling, and milling below the
    // surface outside every slot's and contour's corridor (a slot's
    // shallower passes and plunges stay inside its own).
    let in_corridor = |tool: &str, a: &[f64; 3], b: &[f64; 3]| {
        program.operations.iter().any(|o| {
            if o.tool.id != tool {
                return false;
            }
            let r = o.tool.diameter / 2.0;
            match &o.op {
                ToolOp::Slot {
                    from, to, width, ..
                } => {
                    let lateral = (width / 2.0 - r).max(0.0) + 0.01;
                    covers_part(from, to, [a[0], a[1]], [b[0], b[1]], lateral)
                }
                ToolOp::Contour { length, width, .. } => {
                    let (l, w) = (*length, *width);
                    let sides = [
                        ([-r, -r], [l + r, -r]),
                        ([l + r, -r], [l + r, w + r]),
                        ([l + r, w + r], [-r, w + r]),
                        ([-r, w + r], [-r, -r]),
                    ];
                    sides
                        .iter()
                        .any(|(p, q)| covers_part(p, q, [a[0], a[1]], [b[0], b[1]], 0.01))
                }
                ToolOp::Cutout {
                    centre,
                    width,
                    height,
                    radius,
                    ..
                } => crate::model::cutout_path(centre[0], centre[1], *width, *height, *radius, r)
                    .windows(2)
                    .any(|w| covers_part(&w[0], &w[1], [a[0], a[1]], [b[0], b[1]], 0.01)),
                _ => false,
            }
        })
    };
    for (i, c) in cuts.iter().enumerate() {
        if used[i] {
            continue;
        }
        match c {
            Cut::Hole { tool, x, y, depth } => findings.push((
                "CAM-307",
                format!(
                    "el NC perfora con {tool} en ({}, {}) a {} mm y ninguna operación lo pide",
                    round3(*x),
                    round3(*y),
                    round3(*depth)
                ),
            )),
            Cut::Horizontal {
                side,
                along,
                height,
                ..
            } => findings.push((
                "CAM-307",
                format!(
                    "el NC taladra el canto {:?} en {} / {} y ninguna operación lo pide",
                    side,
                    round3(*along),
                    round3(*height)
                ),
            )),
            Cut::Segment { tool, from, to } if !in_corridor(tool, from, to) => findings.push((
                "CAM-307",
                format!(
                    "el NC fresa de {:?} a {:?} y ninguna operación lo pide",
                    from.map(round3),
                    to.map(round3)
                ),
            )),
            Cut::Segment { .. } => {}
        }
    }
    verified
}

/// Does the segment a→b lie on p→q's line (within `lateral`) and between
/// its ends? It need not cover it: a plunge or a partial pass is fine.
fn covers_part(p: &[f64; 2], q: &[f64; 2], a: [f64; 2], b: [f64; 2], lateral: f64) -> bool {
    let dx = q[0] - p[0];
    let dy = q[1] - p[1];
    let len = (dx * dx + dy * dy).sqrt();
    if len <= EPS {
        return (a[0] - p[0]).hypot(a[1] - p[1]) <= lateral
            && (b[0] - p[0]).hypot(b[1] - p[1]) <= lateral;
    }
    let (ux, uy) = (dx / len, dy / len);
    let inside = |r: [f64; 2]| {
        let along = (r[0] - p[0]) * ux + (r[1] - p[1]) * uy;
        let across = (r[0] - p[0]) * -uy + (r[1] - p[1]) * ux;
        along >= -lateral - 0.01 && along <= len + lateral + 0.01 && across.abs() <= lateral
    };
    inside(a) && inside(b)
}

/// Does the segment a→b run along p→q (either direction) and cover it,
/// staying within `lateral` of its line?
fn covers(p: &[f64; 2], q: &[f64; 2], a: [f64; 2], b: [f64; 2], lateral: f64) -> bool {
    let dx = q[0] - p[0];
    let dy = q[1] - p[1];
    let len = (dx * dx + dy * dy).sqrt();
    if len <= EPS {
        return (a[0] - p[0]).hypot(a[1] - p[1]) <= lateral;
    }
    let (ux, uy) = (dx / len, dy / len);
    let along = |r: [f64; 2]| (r[0] - p[0]) * ux + (r[1] - p[1]) * uy;
    let across = |r: [f64; 2]| ((r[0] - p[0]) * -uy + (r[1] - p[1]) * ux).abs();
    if across(a) > lateral || across(b) > lateral {
        return false;
    }
    // The NC carries three decimals: the same tolerance applies along.
    let (s0, s1) = (along(a), along(b));
    s0.min(s1) <= lateral && s0.max(s1) >= len - lateral
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cam::{GenericIso, PostProcessor};
    use crate::library::Libraries;

    fn programs_of(fixture: &str) -> (Vec<Program>, ManufacturingProfile) {
        let plan = crate::compile_json(fixture);
        let libs = Libraries::default();
        let mut diags = Diagnostics::default();
        let progs = plan
            .parts
            .iter()
            .flat_map(|p| crate::cam::programs(p, &libs.profile, &mut diags))
            .collect();
        (progs, libs.profile)
    }

    #[test]
    fn a_cutout_is_cut_along_its_whole_outline_and_nowhere_else() {
        let (progs, profile) = programs_of(include_str!("../../../fixtures/vanity/input.json"));
        let post = GenericIso::default();
        let p = progs
            .iter()
            .find(|p| {
                p.operations
                    .iter()
                    .any(|o| matches!(o.op, ToolOp::Cutout { .. }))
            })
            .unwrap();
        let codes = |nc: &str| {
            let mut diags = Diagnostics::default();
            simulate(p, nc, &profile, &mut diags);
            diags
                .items
                .iter()
                .map(|d| d.code.clone())
                .collect::<Vec<_>>()
        };
        let nc = post.render(p);
        assert!(codes(&nc).is_empty(), "{:?}", codes(&nc));
        // The last full-depth move of the outline left out: the slug hangs
        // on by that corner.
        let lines: Vec<&str> = nc.lines().collect();
        let last = lines.iter().rposition(|l| l.starts_with("G01 X")).unwrap();
        let cut: String = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != last)
            .map(|(_, l)| format!("{l}\n"))
            .collect();
        assert!(codes(&cut).contains(&"CAM-306".to_string()));
    }

    #[test]
    fn the_generic_post_survives_its_own_simulation() {
        let (progs, profile) =
            programs_of(include_str!("../../../fixtures/wardrobe_1800/input.json"));
        let post = GenericIso::default();
        let mut total = 0.0;
        for p in &progs {
            let mut diags = Diagnostics::default();
            let sim = simulate(p, &post.render(p), &profile, &mut diags);
            assert!(diags.items.is_empty(), "{}: {:#?}", p.name(), diags.items);
            assert_eq!(sim.operations_verified, p.operations.len(), "{}", p.name());
            assert!(sim.seconds > 0.0);
            total += sim.seconds;
        }
        // Same input, same time: the estimate is part of the plan.
        let again: f64 = progs
            .iter()
            .map(|p| simulate(p, &post.render(p), &profile, &mut Diagnostics::default()).seconds)
            .sum();
        assert_eq!(total, again);
    }

    #[test]
    fn a_nesting_router_program_survives_its_own_simulation_too() {
        // Every fixture, on the raw panel: shifted holes, the outline at the
        // cut size, edge drilling in its own setup after banding.
        let dir = format!("{}/../../fixtures", env!("CARGO_MANIFEST_DIR"));
        let mut profile = Libraries::default().profile;
        profile.workflow = crate::library::profile::Workflow::NestedRouter;
        let post = GenericIso::default();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.ends_with("invalid_cabinet") {
                continue;
            }
            let plan =
                crate::compile_json(&std::fs::read_to_string(path.join("input.json")).unwrap());
            for part in &plan.parts {
                let mut diags = Diagnostics::default();
                for p in crate::cam::programs(part, &profile, &mut diags) {
                    let sim = simulate(&p, &post.render(&p), &profile, &mut diags);
                    assert_eq!(
                        sim.operations_verified,
                        p.operations.len(),
                        "{path:?} {}",
                        p.name()
                    );
                }
                assert!(diags.items.is_empty(), "{path:?}: {:#?}", diags.items);
            }
        }
    }

    #[test]
    fn a_post_that_bites_too_deep_skimps_a_slot_or_mills_elsewhere_is_caught() {
        let (progs, profile) =
            programs_of(include_str!("../../../fixtures/basic_cabinet/input.json"));
        let post = GenericIso::default();
        // A side: its back groove is 3.2 wide, cut with a Ø3 end mill in
        // passes of at most its pass depth.
        let p = progs
            .iter()
            .find(|p| {
                p.operations
                    .iter()
                    .any(|o| matches!(o.op, ToolOp::Slot { .. }))
            })
            .unwrap();
        let codes = |nc: &str| {
            let mut diags = Diagnostics::default();
            simulate(p, nc, &profile, &mut diags);
            diags
                .items
                .iter()
                .map(|d| d.code.clone())
                .collect::<Vec<_>>()
        };
        assert!(codes(&post.render(p)).is_empty());

        // The whole groove depth in one pass.
        let mut greedy = p.clone();
        for o in &mut greedy.operations {
            o.tool.max_depth_per_pass = 100.0;
        }
        assert!(codes(&post.render(&greedy)).contains(&"CAM-309".to_string()));

        // One pass down the middle: the groove comes out 3 mm, not 3.2.
        let mut narrow = p.clone();
        for o in &mut narrow.operations {
            if let ToolOp::Slot { width, .. } = &mut o.op {
                *width = o.tool.diameter;
            }
        }
        let got = codes(&post.render(&narrow));
        assert!(got.contains(&"CAM-306".to_string()), "{got:?}");

        // A feed move across the panel with the tool still down.
        let nc = post.render(p);
        let at = nc.find("G01 Z-").unwrap();
        let end = at + nc[at..].find('\n').unwrap() + 1;
        let stray = format!("{}G01 X200 Y200 F1000\n{}", &nc[..end], &nc[end..]);
        assert!(codes(&stray).contains(&"CAM-307".to_string()));
    }

    #[test]
    fn a_post_that_drops_or_moves_a_hole_is_caught() {
        let (progs, profile) =
            programs_of(include_str!("../../../fixtures/basic_cabinet/input.json"));
        let post = GenericIso::default();
        let top = progs
            .iter()
            .find(|p| p.part == "P003" && p.setup == 'A')
            .unwrap();
        let nc = post.render(top);
        let g81 = nc
            .lines()
            .find(|l| l.starts_with("G81") || l.starts_with("G83"))
            .unwrap();

        // Dropped hole: the operation is not machined.
        let dropped = nc.replacen(g81, "", 1);
        let mut diags = Diagnostics::default();
        simulate(top, &dropped, &profile, &mut diags);
        assert!(
            diags.items.iter().any(|d| d.code == "CAM-306"),
            "{:#?}",
            diags.items
        );

        // Moved hole: one operation missing, one cut nobody asked for.
        let moved = nc.replacen(g81, &g81.replacen("X", "X1", 1), 1);
        let mut diags = Diagnostics::default();
        simulate(top, &moved, &profile, &mut diags);
        let codes: Vec<&str> = diags.items.iter().map(|d| d.code.as_str()).collect();
        assert!(
            codes.contains(&"CAM-306") && codes.contains(&"CAM-307"),
            "{codes:?}"
        );
    }

    #[test]
    fn rapids_into_material_and_table_hits_are_caught() {
        let (progs, profile) =
            programs_of(include_str!("../../../fixtures/basic_cabinet/input.json"));
        let post = GenericIso::default();
        let p = &progs[0];
        let nc = post.render(p);

        let plunge = nc.replacen("G00 Z3\n", "G00 Z-5\n", 1);
        let mut diags = Diagnostics::default();
        simulate(p, &plunge, &profile, &mut diags);
        assert!(diags.items.iter().any(|d| d.code == "CAM-301"));

        let zline = nc.lines().find(|l| l.starts_with("G01 Z-")).unwrap();
        let through_table = nc.replacen(zline, "G01 Z-40 F800", 1);
        let mut diags = Diagnostics::default();
        simulate(p, &through_table, &profile, &mut diags);
        assert!(
            diags.items.iter().any(|d| d.code == "CAM-305"),
            "{:#?}",
            diags.items
        );

        let no_spindle = nc.replacen("M03", "M05", 1);
        let mut diags = Diagnostics::default();
        simulate(p, &no_spindle, &profile, &mut diags);
        assert!(diags.items.iter().any(|d| d.code == "CAM-302"));

        let tline = nc.lines().find(|l| l.starts_with('T')).unwrap();
        let unknown_tool = nc.replacen(tline, "T99 M06", 1);
        let mut diags = Diagnostics::default();
        simulate(p, &unknown_tool, &profile, &mut diags);
        assert!(diags.items.iter().any(|d| d.code == "CAM-303"));
    }

    #[test]
    fn a_clamp_in_the_contour_path_is_a_collision_and_a_pod_under_a_hole_too() {
        // A router cutting the outline: the case where a clamp is in its way.
        let plan = crate::compile_json(include_str!("../../../fixtures/basic_cabinet/input.json"));
        let mut profile = Libraries::default().profile;
        profile.workflow = crate::library::profile::Workflow::NestedRouter;
        let mut diags = Diagnostics::default();
        let progs: Vec<Program> = plan
            .parts
            .iter()
            .flat_map(|p| crate::cam::programs(p, &profile, &mut diags))
            .collect();
        let post = GenericIso::default();
        let p = &progs[0];
        let nc = post.render(p);
        // Nothing declared: clean.
        let mut diags = Diagnostics::default();
        simulate(p, &nc, &profile, &mut diags);
        assert!(diags.items.is_empty());
        // A clamp bar along the blank's front edge, 40 mm tall: the contour
        // (tool outside the blank, at −6 mm) runs straight through it.
        profile.machine.fixtures.push(Fixture {
            name: "prensa frontal".into(),
            x: 0.0,
            y: -30.0,
            length: p.length,
            width: 20.0,
            top: 40.0,
        });
        let mut diags = Diagnostics::default();
        simulate(p, &nc, &profile, &mut diags);
        assert!(
            diags
                .items
                .iter()
                .any(|d| d.code == "CAM-308" && d.message.contains("prensa frontal")),
            "{:#?}",
            diags.items
        );
        // A vacuum pod under the first hole: only a through cut reaches it.
        profile.machine.fixtures.clear();
        let (hx, hy, through) = p
            .operations
            .iter()
            .find_map(|o| match o.op {
                ToolOp::Drill { x, y, through, .. } => Some((x, y, through)),
                _ => None,
            })
            .unwrap();
        profile.machine.fixtures.push(Fixture {
            name: "ventosa".into(),
            x: hx - 40.0,
            y: hy - 40.0,
            length: 80.0,
            width: 80.0,
            top: -p.thickness,
        });
        let mut diags = Diagnostics::default();
        simulate(p, &nc, &profile, &mut diags);
        let hits = diags.items.iter().filter(|d| d.code == "CAM-308").count();
        if through {
            assert!(hits > 0);
        } else {
            // A blind hole stays above the pod; the contour (through) may
            // still cross it, which is a real collision too.
            let _ = hits;
        }
    }

    #[test]
    fn covers_handles_direction_and_partial_runs() {
        let p = [0.0, 0.0];
        let q = [100.0, 0.0];
        assert!(covers(&p, &q, [0.0, 0.0], [100.0, 0.0], 0.1));
        assert!(covers(&p, &q, [100.0, 0.0], [0.0, 0.0], 0.1));
        assert!(covers(&p, &q, [-3.0, 0.05], [103.0, 0.0], 0.1));
        assert!(!covers(&p, &q, [0.0, 0.0], [90.0, 0.0], 0.1));
        // Rounded to the NC's three decimals: still a match.
        assert!(covers(
            &[0.0, 0.0],
            &[237.3333, 0.0],
            [0.0, 0.0],
            [237.333, 0.0],
            0.01
        ));
        assert!(!covers(&p, &q, [0.0, 1.0], [100.0, 1.0], 0.1));
    }
}
