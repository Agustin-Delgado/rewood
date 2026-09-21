//! The AI front door (§43, §53): natural language in, a `FurnitureSpec`
//! out, the deterministic engine in between. The model only ever writes
//! the specification — dimensions, components, hardware ids from the
//! libraries — and never a hole position; the engine compiles what it
//! proposes and hands the diagnostics back, so a spec that does not
//! manufacture is corrected in a loop before the user sees it.
//!
//! The model is behind a trait so the loop is tested with a scripted fake;
//! `AnthropicModel` is the real one (`ANTHROPIC_API_KEY`, `REWOOD_MODEL`).

use rewood_core::diagnostics::Severity;
use rewood_core::spec::FurnitureSpec;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// One turn of the conversation as the client keeps it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantRequest {
    pub message: String,
    /// The spec on screen, so "hacelo 20 cm más ancho" has something to
    /// edit.
    #[serde(default)]
    pub spec: Option<FurnitureSpec>,
    #[serde(default)]
    pub history: Vec<Turn>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantReply {
    pub reply: String,
    /// The proposed spec, when the model proposed one, as it compiled last.
    pub spec: Option<FurnitureSpec>,
    pub status: Option<rewood_core::plan::PlanStatus>,
    pub manufacturing_blocked: Option<bool>,
    pub diagnostics: Vec<rewood_core::diagnostics::Diagnostic>,
    /// How many times the engine sent findings back to the model.
    pub corrections: usize,
    pub model: String,
}

/// What the model answered: prose, and/or a call to `propose_spec`.
#[derive(Debug, Clone, Default)]
pub struct ModelReply {
    pub text: String,
    pub spec: Option<Value>,
}

#[async_trait::async_trait]
pub trait Model: Send + Sync {
    fn name(&self) -> String;
    /// `messages` are Anthropic-shaped (`role`, `content` blocks); the
    /// implementation may keep or flatten them.
    async fn complete(&self, system: &str, messages: &[Value]) -> Result<ModelReply, String>;
}

const TOOL_NAME: &str = "propose_spec";
const MAX_ROUNDS: usize = 4;

/// The reference the model works from: the spec format, the ids it may
/// use, and one complete example. Built from the libraries, so a hardware
/// item added to `hardware.json` is offered without touching this.
pub fn system_prompt() -> String {
    let libs = rewood_core::library::Libraries::default();
    let mut s = String::new();
    s.push_str(
        "Sos el asistente de rewood, un compilador determinista de mobiliario. \
Tu única salida técnica es una FurnitureSpec (JSON) que proponés con la herramienta `propose_spec`; \
el motor la compila, calcula piezas, uniones, perforaciones y CNC, y te devuelve los hallazgos. \
Nunca calculás geometría de fabricación (posiciones de agujeros, cantos, offsets): eso lo hace el motor. \
Si el usuario pide algo que el formato no puede expresar, decilo en una línea y proponé lo más cercano.\n\n\
Formato (schemaVersion \"1.0\"): { id, name, version?, parameters: { nombre: número | expresión }, material, edgeMaterial?, components: [...], constraints?: [...] }.\n\
Todo campo numérico acepta una expresión sobre los parámetros (\"width - 2 * 18\", \"if(width > 600, 2, 1)\").\n\
Componentes (campo `type`):\n\
- carcass: { id, width?, height?, depth? (por defecto \"width\"/\"height\"/\"depth\"), material?, joint: { hardware: [ids] }, back?: { material, groove?: { inset, depth, clearance } }, bays? (N bahías, N-1 divisores verticales), bayWidths?: [500, \"auto\"], legs?: { plinth?: { setback } }, hanging?: {} (alacena colgada), edges? }\n\
- shelves: { id, carcass?, bay? (1-based; omitido = todas), zone?: { from, to } (mm desde la base), count? (reparto parejo) o positions?: [alturas de estantes fijos], setback?, material?, support?: \"joint\" | \"pins\" (regulables sobre hileras Sistema 32, sin joint), joint (si support es joint), edges? }\n\
- worktop: { id, carcasses?: [ids] (vacío = todas), overhang?: { front, back, sides }, material?, fixing?, edges? } — una tapa sobre una o varias carcasas (escritorio sobre dos cajoneras)\n\
- doors: { id, carcass?, bay?, zone?, count (1 o 2 por bahía), gap?, material?, hinge?: { hardware: [id] } | null, handle?: { hardware: [id], fromEdge?, position? }, edges? }\n\
- drawers: { id, carcass?, bay?, zone?, count, frontHeight?, gap?, boxHeight?, material?, boxMaterial?, bottomMaterial?, joint, slide: { hardware: [id] }, frontFixing?, handle?, edges? }\n\
edges: \"default\" | \"none\" | \"front\" | \"all\". constraints: [{ id, expr (booleana), severity?: INFO|WARNING|ERROR|FATAL, message? }].\n\
Zonas: cajones y puertas de la misma bahía se reparten la altura con `zone`; lo que se superpone lo acusa el motor (FAB-101).\n\n",
    );
    // Through the JSON view: it is what the `/libraries` endpoint and the
    // UI see, and the model gets exactly those ids.
    let v = serde_json::to_value(&libs).unwrap_or(Value::Null);
    let obj = |x: &Value| x.as_object().cloned().unwrap_or_default();
    s.push_str("Materiales: ");
    for (id, m) in obj(&v["materials"]["materials"]) {
        s.push_str(&format!(
            "{id} ({}, {} mm); ",
            m["name"].as_str().unwrap_or(""),
            m["nominalThickness"]
        ));
    }
    s.push_str(
        "
Cantos: ",
    );
    for (id, m) in obj(&v["materials"]["edgeMaterials"]) {
        s.push_str(&format!("{id} ({}); ", m["name"].as_str().unwrap_or("")));
    }
    s.push_str(
        "
Herrajes: ",
    );
    for (id, h) in obj(&v["hardware"]["items"]) {
        s.push_str(&format!(
            "{id} [{}] ({}); ",
            h["kind"].as_str().unwrap_or(""),
            h["name"].as_str().unwrap_or("")
        ));
    }
    s.push_str(&format!(
        "
Perfil de máquina: {} ({}×{} mm máx.).

",
        libs.profile.name, libs.profile.max_part_size[0], libs.profile.max_part_size[1]
    ));
    s.push_str(
        "Ejemplo completo (placard de 1800 con 3 módulos, 2 puertas, estantes y cajones):\n",
    );
    s.push_str(include_str!("../../../fixtures/wardrobe_1800/input.json"));
    s.push_str(
        "\n\nReglas: respondé en español, corto. Cuando el pedido cambia el mueble, llamá a `propose_spec` con la spec COMPLETA \
(no un parche). Si hay una spec actual, partí de ella y cambiá sólo lo pedido. Usá parámetros con nombre para las medidas \
principales (width, height, depth). Si el motor devuelve hallazgos FATAL o ERROR, corregí la spec y volvé a proponer; \
un WARNING se explica y se deja. Terminá con una línea que diga qué quedó y qué hallazgos hay.",
    );
    s
}

fn tool_definition() -> Value {
    json!({
        "name": TOOL_NAME,
        "description": "Proponer la especificación completa del mueble. El motor la compila y devuelve los hallazgos.",
        "input_schema": {
            "type": "object",
            "properties": { "spec": { "type": "object", "description": "FurnitureSpec completa, schemaVersion 1.0" } },
            "required": ["spec"]
        }
    })
}

/// A short, model-facing summary of what the engine found.
fn findings_for_model(plan: &rewood_core::plan::ManufacturingPlan) -> String {
    let mut s = format!(
        "Compilado: estado {:?}, {} piezas, bloqueado={}.",
        plan.status,
        plan.parts.len(),
        plan.manufacturing_blocked
    );
    let mut shown = 0;
    for d in &plan.diagnostics.items {
        if d.severity < Severity::Warning {
            continue;
        }
        if shown == 12 {
            s.push_str("\n… (más hallazgos omitidos)");
            break;
        }
        s.push_str(&format!(
            "\n[{:?}] {} {}: {}",
            d.severity,
            d.code,
            d.entity.as_deref().unwrap_or(""),
            d.message
        ));
        if let Some(sug) = &d.suggestion {
            s.push_str(&format!(" → {sug}"));
        }
        shown += 1;
    }
    s
}

/// Run the conversation turn: model → spec → engine → (findings → model)*.
pub async fn run(model: &dyn Model, req: &AssistantRequest) -> Result<AssistantReply, String> {
    let system = system_prompt();
    let mut messages: Vec<Value> = req
        .history
        .iter()
        .map(|t| json!({ "role": t.role, "content": t.content }))
        .collect();
    let mut user = req.message.clone();
    if let Some(spec) = &req.spec {
        user.push_str("\n\nSpec actual:\n");
        user.push_str(&serde_json::to_string(spec).map_err(|e| e.to_string())?);
    }
    messages.push(json!({ "role": "user", "content": user }));

    let mut last_text = String::new();
    let mut result: Option<(FurnitureSpec, rewood_core::plan::ManufacturingPlan)> = None;
    let mut corrections = 0;
    for round in 0..MAX_ROUNDS {
        let reply = model.complete(&system, &messages).await?;
        if !reply.text.trim().is_empty() {
            last_text = reply.text.trim().to_string();
        }
        let Some(raw) = reply.spec else {
            break;
        };
        // The engine is the judge: parse, compile, report.
        let spec: FurnitureSpec = match serde_json::from_value(raw.clone()) {
            Ok(s) => s,
            Err(e) => {
                messages.push(json!({ "role": "assistant", "content": [
                    { "type": "text", "text": reply.text },
                    { "type": "tool_use", "id": format!("call_{round}"), "name": TOOL_NAME, "input": { "spec": raw } }
                ]}));
                messages.push(json!({ "role": "user", "content": [
                    { "type": "tool_result", "tool_use_id": format!("call_{round}"), "content": format!("La spec no respeta el formato: {e}. Corregila y volvé a proponer.") }
                ]}));
                corrections += 1;
                continue;
            }
        };
        let plan = rewood_core::compile(&spec);
        let summary = findings_for_model(&plan);
        let needs_fix = plan.manufacturing_blocked || plan.diagnostics.count(Severity::Error) > 0;
        result = Some((spec, plan));
        messages.push(json!({ "role": "assistant", "content": [
            { "type": "text", "text": reply.text },
            { "type": "tool_use", "id": format!("call_{round}"), "name": TOOL_NAME, "input": { "spec": raw } }
        ]}));
        messages.push(json!({ "role": "user", "content": [
            { "type": "tool_result", "tool_use_id": format!("call_{round}"), "content": summary }
        ]}));
        if needs_fix && round + 1 < MAX_ROUNDS {
            corrections += 1;
            continue;
        }
        // Accepted: one more turn for the closing line, no tool expected.
        let closing = model.complete(&system, &messages).await?;
        if !closing.text.trim().is_empty() {
            last_text = closing.text.trim().to_string();
        }
        break;
    }
    let (spec, plan) = match result {
        Some((s, p)) => (Some(s), Some(p)),
        None => (None, None),
    };
    Ok(AssistantReply {
        reply: last_text,
        status: plan.as_ref().map(|p| p.status),
        manufacturing_blocked: plan.as_ref().map(|p| p.manufacturing_blocked),
        diagnostics: plan
            .as_ref()
            .map(|p| {
                p.diagnostics
                    .items
                    .iter()
                    .filter(|d| d.severity >= Severity::Warning)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default(),
        spec,
        corrections,
        model: model.name(),
    })
}

/// Anthropic Messages API with tool use.
pub struct AnthropicModel {
    pub api_key: String,
    pub model: String,
    client: reqwest::Client,
}

impl AnthropicModel {
    /// `None` when there is no key: the endpoint then answers 503.
    pub fn from_env() -> Option<AnthropicModel> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .filter(|k| !k.is_empty())?;
        Some(AnthropicModel {
            api_key,
            model: std::env::var("REWOOD_MODEL").unwrap_or_else(|_| "claude-sonnet-5".into()),
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait::async_trait]
impl Model for AnthropicModel {
    fn name(&self) -> String {
        self.model.clone()
    }

    async fn complete(&self, system: &str, messages: &[Value]) -> Result<ModelReply, String> {
        let body = json!({
            "model": self.model,
            "max_tokens": 8192,
            "system": system,
            "tools": [tool_definition()],
            "messages": messages,
        });
        let res = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("no se pudo llamar al modelo: {e}"))?;
        let status = res.status();
        let v: Value = res
            .json()
            .await
            .map_err(|e| format!("respuesta del modelo ilegible: {e}"))?;
        if !status.is_success() {
            return Err(format!(
                "el modelo respondió {status}: {}",
                v["error"]["message"].as_str().unwrap_or("sin detalle")
            ));
        }
        let mut out = ModelReply::default();
        for block in v["content"].as_array().cloned().unwrap_or_default() {
            match block["type"].as_str() {
                Some("text") => {
                    out.text.push_str(block["text"].as_str().unwrap_or(""));
                }
                Some("tool_use") if block["name"] == TOOL_NAME => {
                    out.spec = block["input"]["spec"]
                        .as_object()
                        .map(|o| Value::Object(o.clone()));
                }
                _ => {}
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Scripted replies, in order; records what it was asked.
    pub struct FakeModel {
        pub replies: Mutex<Vec<ModelReply>>,
        pub seen: Mutex<Vec<Vec<Value>>>,
    }

    impl FakeModel {
        pub fn new(replies: Vec<ModelReply>) -> FakeModel {
            FakeModel {
                replies: Mutex::new(replies),
                seen: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl Model for FakeModel {
        fn name(&self) -> String {
            "fake".into()
        }
        async fn complete(&self, _system: &str, messages: &[Value]) -> Result<ModelReply, String> {
            self.seen.lock().unwrap().push(messages.to_vec());
            let mut r = self.replies.lock().unwrap();
            if r.is_empty() {
                Ok(ModelReply::default())
            } else {
                Ok(r.remove(0))
            }
        }
    }

    fn spec_json(width: u32) -> Value {
        json!({
            "schemaVersion": "1.0", "id": "mesa_luz", "name": "Mesa de luz",
            "parameters": { "width": width, "height": 600, "depth": 400 },
            "material": "melamine_18", "edgeMaterial": "abs_1mm",
            "components": [
                { "type": "carcass", "id": "carcass", "joint": { "hardware": ["minifix_15", "dowel_8x30"] }, "back": { "material": "hdf_3" } },
                { "type": "drawers", "id": "drawers", "count": 2, "joint": { "hardware": ["dowel_8x30"] }, "slide": { "hardware": ["slide_ball_450"] } }
            ]
        })
    }

    #[tokio::test]
    async fn the_engine_sends_findings_back_until_the_spec_manufactures() {
        // 450 mm slides in a 400 mm deep cabinet: the first proposal fails
        // and the model gets the findings; the second one is fine.
        let mut bad = spec_json(500);
        bad["parameters"]["depth"] = json!(400);
        let mut good = spec_json(500);
        good["parameters"]["depth"] = json!(500);
        let model = FakeModel::new(vec![
            ModelReply {
                text: "Propongo una mesa de luz.".into(),
                spec: Some(bad),
            },
            ModelReply {
                text: "Corrijo la profundidad para las correderas.".into(),
                spec: Some(good),
            },
            ModelReply {
                text: "Listo: mesa de luz 500×600×500, 2 cajones, sin hallazgos.".into(),
                spec: None,
            },
        ]);
        let req = AssistantRequest {
            message: "quiero una mesa de luz con dos cajones".into(),
            spec: None,
            history: vec![],
        };
        let out = run(&model, &req).await.unwrap();
        assert_eq!(out.corrections, 1);
        assert_eq!(out.manufacturing_blocked, Some(false));
        assert!(out.reply.starts_with("Listo"));
        assert_eq!(
            out.spec.unwrap().parameters["depth"],
            rewood_core::params::ParamInput::Number(500.0)
        );
        // The second call carried the tool result with the findings.
        let seen = model.seen.lock().unwrap();
        assert_eq!(seen.len(), 3);
        let second = serde_json::to_string(&seen[1]).unwrap();
        assert!(second.contains("tool_result"));
        assert!(
            second.contains("JOINT-") || second.contains("FAB-") || second.contains("SPEC-"),
            "{second}"
        );
    }

    #[tokio::test]
    async fn a_reply_without_a_spec_is_just_text() {
        let model = FakeModel::new(vec![ModelReply {
            text: "¿De qué ancho lo querés?".into(),
            spec: None,
        }]);
        let req = AssistantRequest {
            message: "un placard".into(),
            spec: None,
            history: vec![],
        };
        let out = run(&model, &req).await.unwrap();
        assert!(out.spec.is_none());
        assert_eq!(out.reply, "¿De qué ancho lo querés?");
        assert_eq!(out.corrections, 0);
    }

    #[test]
    fn the_prompt_lists_the_libraries_and_the_example() {
        let p = system_prompt();
        assert!(p.contains("minifix_15"));
        assert!(p.contains("melamine_18"));
        assert!(p.contains("\"schemaVersion\": \"1.0\""));
    }
}
