//! Layout checks that need every component expanded: what each bay of
//! each carcass ended up holding, and how the carcasses of a run sit
//! next to each other. Generators cannot see this — a drawer stack does
//! not know a door was declared over the same bay two components later.

use super::{BuildCtx, Occupancy, OccupancyKind};
use crate::diagnostics::{Diagnostic, Severity};
use crate::rules::mm;
use crate::units::EPS;

/// Smallest uncovered height of a bay that counts as a hole in the
/// front, not as the gap between two fronts.
const OPEN_MIN: f64 = 40.0;
/// A gap between neighbouring modules up to this is taken for a mistake.
const RUN_GAP_MAX: f64 = 50.0;

pub fn check(ctx: &mut BuildCtx<'_>) {
    let mut out = Vec::new();
    for carcass in ctx.carcasses.values() {
        for bay in &carcass.bays {
            let here: Vec<&Occupancy> = ctx
                .occupancy
                .iter()
                .filter(|o| o.carcass == carcass.id && o.bay == bay.index)
                .collect();
            let bay_label = if carcass.bays.len() > 1 {
                format!("la bahía {} de '{}'", bay.index, carcass.id)
            } else {
                format!("la carcasa '{}'", carcass.id)
            };
            if here.is_empty() {
                out.push(
                    Diagnostic::new(
                        "SPEC-211",
                        Severity::Info,
                        format!("{bay_label} está vacía: sin estantes, puertas ni cajones"),
                    )
                    .entity(carcass.id.clone()),
                );
                continue;
            }
            // Two things claiming the same height of the same bay.
            for (i, a) in here.iter().enumerate() {
                for b in &here[i + 1..] {
                    if !a.kind.clashes_with(b.kind) {
                        continue;
                    }
                    let overlap = a.zone.z1.min(b.zone.z1) - a.zone.z0.max(b.zone.z0);
                    if overlap > EPS {
                        out.push(
                            Diagnostic::new(
                                "SPEC-210",
                                Severity::Error,
                                format!(
                                    "{} de '{}' ({}–{}) y {} de '{}' ({}–{}) se pisan {} mm en {bay_label}",
                                    a.kind.label(),
                                    a.component,
                                    mm(a.zone.z0),
                                    mm(a.zone.z1),
                                    b.kind.label(),
                                    b.component,
                                    mm(b.zone.z0),
                                    mm(b.zone.z1),
                                    mm(overlap)
                                ),
                            )
                            .entity(b.component.clone())
                            .location(a.component.clone())
                            .suggestion(
                                "Dales zonas ('zone') que no se solapen, o ponelos en bahías distintas.",
                            ),
                        );
                    }
                }
            }
            // Inner drawers behind an inset door share its plane unless
            // they are set back: the door would hit the fronts.
            for a in here.iter().filter(|o| o.kind == OccupancyKind::Doors) {
                let Some((dy0, _)) = a.front_y else {
                    continue;
                };
                for b in here
                    .iter()
                    .filter(|o| o.kind == OccupancyKind::InnerDrawers)
                {
                    let Some((_, by1)) = b.front_y else {
                        continue;
                    };
                    let overlap = a.zone.z1.min(b.zone.z1) - a.zone.z0.max(b.zone.z0);
                    if overlap > EPS && by1 > dy0 + EPS {
                        out.push(
                            Diagnostic::new(
                                "SPEC-214",
                                Severity::Error,
                                format!(
                                    "los cajones interiores de '{}' llegan hasta {} mm de profundidad y la puerta embutida de '{}' empieza en {}: se tocan en {bay_label}",
                                    b.component,
                                    mm(by1),
                                    a.component,
                                    mm(dy0)
                                ),
                            )
                            .entity(b.component.clone())
                            .location(a.component.clone())
                            .suggestion("Retranqueá los cajones interiores ('setback') al menos el espesor de la puerta más la luz.")
                            .fix(
                                format!("Retranquear {} mm", mm((by1 - dy0 + 2.0).ceil())),
                                b.component.clone(),
                                "setback",
                                serde_json::json!((by1 - dy0 + 2.0).ceil()),
                            ),
                        );
                    }
                }
            }
            // A bay with fronts that do not reach all of its height.
            let mut fronts: Vec<(f64, f64)> = here
                .iter()
                .filter(|o| o.kind.is_front())
                .map(|o| (o.zone.z0, o.zone.z1))
                .collect();
            if !fronts.is_empty() {
                fronts.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let mut cursor = 0.0;
                let mut open = Vec::new();
                for (z0, z1) in fronts {
                    if z0 - cursor >= OPEN_MIN {
                        open.push((cursor, z0));
                    }
                    cursor = cursor.max(z1);
                }
                if carcass.height - cursor >= OPEN_MIN {
                    open.push((cursor, carcass.height));
                }
                for (z0, z1) in open {
                    out.push(
                        Diagnostic::new(
                            "SPEC-212",
                            Severity::Warning,
                            format!(
                                "{bay_label} queda abierta entre {} y {} mm: ni puerta ni cajón la cubre",
                                mm(z0),
                                mm(z1)
                            ),
                        )
                        .entity(carcass.id.clone())
                        .suggestion(
                            "Extendé la zona de las puertas o de los cajones, o agregá un frente para ese tramo.",
                        ),
                    );
                }
            }
        }
    }

    // Modules of a run: sorted by X, each against the next one that shares
    // its height and depth range.
    let mut run: Vec<_> = ctx.carcasses.values().collect();
    run.sort_by(|a, b| {
        a.origin
            .0
            .partial_cmp(&b.origin.0)
            .unwrap()
            .then_with(|| a.id.cmp(&b.id))
    });
    for (i, a) in run.iter().enumerate() {
        for b in &run[i + 1..] {
            let same_row = a.origin.2 < b.origin.2 + b.height
                && b.origin.2 < a.origin.2 + a.height
                && a.origin.1 < b.origin.1 + b.depth
                && b.origin.1 < a.origin.1 + a.depth;
            if !same_row {
                continue;
            }
            let gap = b.origin.0 - (a.origin.0 + a.width);
            if gap < -EPS {
                out.push(
                    Diagnostic::new(
                        "SPEC-213",
                        Severity::Error,
                        format!(
                            "los módulos '{}' y '{}' se superponen {} mm",
                            a.id,
                            b.id,
                            mm(-gap)
                        ),
                    )
                    .entity(b.id.clone())
                    .location(a.id.clone())
                    .suggestion("Corré el 'origin.x' del segundo módulo al ancho del primero.")
                    .fix(
                        format!(
                            "Pegar '{}' a '{}' (origin.x = {})",
                            b.id,
                            a.id,
                            mm(a.origin.0 + a.width)
                        ),
                        b.id.clone(),
                        "origin.x",
                        serde_json::json!(a.origin.0 + a.width),
                    ),
                );
            } else if gap > EPS && gap <= RUN_GAP_MAX {
                out.push(
                    Diagnostic::new(
                        "SPEC-213",
                        Severity::Warning,
                        format!(
                            "quedan {} mm entre los módulos '{}' y '{}'",
                            mm(gap),
                            a.id,
                            b.id
                        ),
                    )
                    .entity(b.id.clone())
                    .location(a.id.clone())
                    .suggestion(
                        "Si van pegados, el 'origin.x' del segundo es el ancho acumulado de los anteriores.",
                    )
                    .fix(
                        format!("Pegar '{}' a '{}' (origin.x = {})", b.id, a.id, mm(a.origin.0 + a.width)),
                        b.id.clone(),
                        "origin.x",
                        serde_json::json!(a.origin.0 + a.width),
                    ),
                );
            }
            // Only the nearest neighbour to the right matters for a gap.
            if gap > -EPS {
                break;
            }
        }
    }
    for d in out {
        ctx.diagnostics.push(d);
    }
}

impl OccupancyKind {
    pub fn label(self) -> &'static str {
        match self {
            OccupancyKind::Doors => "las puertas",
            OccupancyKind::Drawers => "los cajones",
            OccupancyKind::InnerDrawers => "los cajones interiores",
            OccupancyKind::Shelves => "los estantes",
            OccupancyKind::Rail => "el barral (y lo colgado)",
        }
    }

    /// What closes the bay from the front. Inner drawers sit behind a
    /// door, so they do not count.
    pub fn is_front(self) -> bool {
        matches!(self, OccupancyKind::Doors | OccupancyKind::Drawers)
    }

    /// Doors close over shelves, and two shelf sets may share a zone (a
    /// fixed divider spans the carcass the spread shelves sit in; if two
    /// panels really meet, the collision rule says so). Drawers against
    /// anything, and fronts against fronts, collide.
    pub fn clashes_with(self, other: OccupancyKind) -> bool {
        use OccupancyKind::*;
        match (self, other) {
            (Doors, Doors) | (Doors, Drawers) | (Drawers, Doors) => true,
            // Doors close over shelves, inner drawers and rails.
            (Doors, _) | (_, Doors) => false,
            (Shelves, Shelves) => false,
            // Drawers, inner drawers and rails take the height they cover.
            _ => true,
        }
    }
}
