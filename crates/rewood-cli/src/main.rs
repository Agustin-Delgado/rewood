//! `rewood`: the engine without a UI.
//!
//! ```text
//! rewood compile <spec.json>   # prints the manufacturing plan as JSON
//! rewood validate <spec.json>  # prints the diagnostics, exit 1 if blocked
//! rewood cutlist <spec.json>   # prints the cut list and BOM as text
//! rewood package <spec.json> <out_dir>   # writes the manufacturing package
//! ```

use std::process::ExitCode;

use rewood_core::diagnostics::Severity;
use rewood_core::plan::ManufacturingPlan;

fn usage() -> ExitCode {
    eprintln!("uso: rewood <compile|validate|cutlist> <spec.json>");
    eprintln!("     rewood package <spec.json> <directorio_salida>");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(cmd), Some(path)) = (args.first(), args.get(1)) else {
        return usage();
    };
    let json = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("no se pudo leer {path}: {e}");
            return ExitCode::from(2);
        }
    };
    let plan = rewood_core::compile_json(&json);
    match cmd.as_str() {
        "compile" => println!("{}", plan.to_json_pretty()),
        "validate" => print_diagnostics(&plan),
        "cutlist" => print_cutlist(&plan),
        "package" => {
            let Some(out_dir) = args.get(2) else {
                return usage();
            };
            if let Err(e) = write_package(&plan, std::path::Path::new(out_dir)) {
                eprintln!("no se pudo escribir el paquete en {out_dir}: {e}");
                return ExitCode::from(2);
            }
            print_diagnostics(&plan);
        }
        _ => return usage(),
    }
    if plan.manufacturing_blocked {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn print_diagnostics(plan: &ManufacturingPlan) {
    let d = &plan.diagnostics;
    println!(
        "{} {} — estado: {:?} ({} fatal, {} error, {} warning, {} info)",
        plan.furniture.id,
        plan.furniture.name,
        plan.status,
        d.count(Severity::Fatal),
        d.count(Severity::Error),
        d.count(Severity::Warning),
        d.count(Severity::Info)
    );
    for item in &d.items {
        let entity = item.entity.as_deref().unwrap_or("-");
        println!(
            "[{:?}] {} {}: {}",
            item.severity, item.code, entity, item.message
        );
        if let Some(s) = &item.suggestion {
            println!("    → {s}");
        }
    }
    if plan.manufacturing_blocked {
        println!("MANUFACTURING BLOCKED");
    }
}

fn print_cutlist(plan: &ManufacturingPlan) {
    println!("Despiece — {} ({})", plan.furniture.name, plan.furniture.id);
    println!(
        "{:<18} {:<20} {:>3} {:>8} {:>8} {:>5} {:<8} {:<30} {:>4}",
        "ID", "Pieza", "Cant", "Largo", "Ancho", "Esp", "Veta", "Cantos (izq/der/inf/sup)", "Ops"
    );
    for row in &plan.part_list {
        println!(
            "{:<18} {:<20} {:>3} {:>8.1} {:>8.1} {:>5.1} {:<8} {:<30} {:>4}",
            row.part_ids.join(","),
            row.name,
            row.quantity,
            row.cut_length,
            row.cut_width,
            row.thickness,
            format!("{:?}", row.grain).to_lowercase(),
            row.edges,
            row.operations
        );
    }
    println!();
    println!("Materiales");
    for s in &plan.bom.sheets {
        println!(
            "  {:<28} {:>3} piezas  {:>6.3} m²  ≈ {} placa(s) de {}×{}",
            s.name, s.parts, s.net_area_m2, s.estimated_sheets, s.sheet_length, s.sheet_width
        );
    }
    println!("Herrajes");
    for h in &plan.bom.hardware {
        println!("  {:<28} {:>4}", h.name, h.quantity);
        for i in &h.items {
            println!("      {:<24} {:>4}", i.name, i.quantity);
        }
    }
    println!("Consumibles");
    for c in &plan.bom.consumables {
        println!("  {:<28} {:>7.2} m", c.name, c.length_m);
    }
    println!("Peso total: {:.2} kg", plan.bom.total_weight_kg);
    if plan.bom.total_cost > 0.0 {
        let b = &plan.bom;
        println!(
            "Costo: materiales {:.2} + máquina {:.2} = {:.2} {}{}",
            b.materials_cost,
            b.machining_cost,
            b.total_cost,
            b.currency,
            if b.unpriced.is_empty() {
                String::new()
            } else {
                format!(" (sin precio: {})", b.unpriced.join(", "))
            }
        );
    }
    let m = &plan.machining;
    println!(
        "Mecanizado: {} programas ({}), {:.1} min de máquina estimados",
        m.programs.len(),
        m.post_processor,
        m.total_seconds / 60.0
    );
    println!();
    print_diagnostics(plan);
}

/// Writes every file of the manufacturing package under `out_dir`. A
/// blocked plan is still written — the diagnostics travel with it in
/// `manifest.json` — so the operator can see why.
fn write_package(plan: &ManufacturingPlan, out_dir: &std::path::Path) -> std::io::Result<()> {
    let files = rewood_core::export::package(plan);
    for f in &files {
        let path = out_dir.join(&f.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &f.contents)?;
    }
    println!("{} archivos escritos en {}", files.len(), out_dir.display());
    Ok(())
}
