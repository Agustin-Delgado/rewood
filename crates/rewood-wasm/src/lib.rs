//! WASM surface of the engine: JSON in, JSON out. Types live on the TS side
//! (`packages/engine`); this crate only moves strings across the boundary.

use wasm_bindgen::prelude::*;

/// Compile a furniture spec (JSON) into a manufacturing plan (JSON).
/// Never throws: a malformed spec comes back as a blocked plan with a
/// `SPEC-000` diagnostic.
#[wasm_bindgen]
pub fn compile(spec_json: &str) -> String {
    serde_json::to_string(&rewood_core::compile_json(spec_json).to_json_value())
        .expect("plan serialises")
}

/// The manufacturing package as a JSON array of `{ path, contents }`.
#[wasm_bindgen]
pub fn package_files(spec_json: &str) -> String {
    let plan = rewood_core::compile_json(spec_json);
    let files: Vec<serde_json::Value> = rewood_core::export::package(&plan)
        .into_iter()
        .map(|f| serde_json::json!({ "path": f.path, "contents": f.contents }))
        .collect();
    serde_json::to_string(&files).expect("files serialise")
}

#[wasm_bindgen]
pub fn engine_version() -> String {
    rewood_core::ENGINE_VERSION.to_string()
}

#[wasm_bindgen]
pub fn schema_version() -> String {
    rewood_core::spec::SCHEMA_VERSION.to_string()
}

/// The default libraries (materials, edge bands, hardware, profile) as JSON,
/// so a UI can offer what exists instead of free text.
#[wasm_bindgen]
pub fn libraries() -> String {
    serde_json::to_string(&rewood_core::library::Libraries::default()).expect("libraries serialise")
}
