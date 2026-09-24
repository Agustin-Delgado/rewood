//! The HTTP API of §41. The engine has no state; this layer stores specs,
//! recalculates on demand and freezes orders.
//!
//! ```text
//! POST /projects                          { name }             → Project
//! GET  /projects                                               → [Project]
//! GET  /projects/:id                                           → Project
//! POST /furniture                         { projectId, spec }  → Furniture
//! GET  /furniture                                              → [Furniture summary]
//! GET  /furniture/:id                                          → Furniture
//! PUT  /furniture/:id                     spec                 → Furniture (new version)
//! POST /furniture/:id/recalculate                              → ManufacturingPlan
//! GET  /furniture/:id/parts                                    → parts of the current plan
//! GET  /furniture/:id/bom                                      → BOM
//! GET  /furniture/:id/operations                               → [{ part, operations }]
//! POST /furniture/:id/validate                                 → { status, blocked, diagnostics }
//! POST /furniture/:id/manufacturing-plan                       → ManufacturingPlan (alias)
//! POST /manufacturing-orders              { furnitureId }      → ManufacturingOrder (frozen)
//! GET  /manufacturing-orders                                   → [order summary]
//! GET  /manufacturing-orders/:id                               → ManufacturingOrder
//! GET  /manufacturing-orders/:id/package[?role=cnc|cutting|assembly|purchasing|supplier] → application/zip
//! GET  /manufacturing-orders/:id/package/{*path}               → one frozen file
//! GET  /manufacturing-orders/:id/production                    → tracking record + summary
//! POST /manufacturing-orders/:id/production/steps { part?, step, done } → same
//! POST /manufacturing-orders/:id/production/status { status }  → same
//! GET  /manufacturing-orders/:id/qc                            → measurements
//! POST /manufacturing-orders/:id/qc { part, length, width, thickness, notes? } → the record
//! GET  /libraries                                              → default libraries
//! POST /assistant                        { message, spec?, history? } → { reply, spec?, status?, diagnostics }
//! GET  /health                                                 → { engine, schema, assistant }
//! ```
//!
//! CORS is open (any origin, no credentials): there is no authentication yet
//! and nothing in the API is per-user. With `--static <dir>` the built UI is
//! served from `/`, unknown paths falling back to `index.html` for the SPA
//! router; API routes always win.

use std::io::Write;
use std::sync::Arc;

use axum::extract::{Extension, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use rewood_core::spec::FurnitureSpec;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};

use crate::assistant::{self, Model};
use crate::store::{FsStore, StoreError};

pub type AppState = Arc<FsStore>;
/// The AI model behind `/assistant`, when a key was configured.
pub type ModelHandle = Option<Arc<dyn Model>>;

pub struct ApiError(StatusCode, String);

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::NotFound(what) => {
                ApiError(StatusCode::NOT_FOUND, format!("no existe: {what}"))
            }
            other => ApiError(StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

/// A JSON body whose rejection (bad JSON, a missing field, a wrong type)
/// answers like every other error, `{ "error": … }`, not axum's plain text.
pub struct Body<T>(pub T);

impl<S, T> axum::extract::FromRequest<S> for Body<T>
where
    Json<T>: axum::extract::FromRequest<S, Rejection = axum::extract::rejection::JsonRejection>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, ApiError> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(v)) => Ok(Body(v)),
            Err(e) => Err(ApiError(e.status(), e.body_text())),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewProject {
    pub name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewFurniture {
    pub project_id: String,
    pub spec: FurnitureSpec,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewOrder {
    pub furniture_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FurnitureSummary {
    pub id: String,
    pub project_id: String,
    pub version: u32,
    pub name: String,
    pub updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderSummary {
    pub id: String,
    pub furniture_id: String,
    pub furniture_version: u32,
    pub status: rewood_core::plan::PlanStatus,
    pub manufacturing_blocked: bool,
    pub created_at: String,
    pub package_sha256: String,
}

/// The published demo, which can talk to a server on this machine.
const DEMO_ORIGIN: &str = "https://rewood-mu.vercel.app";

/// Which pages may call the API from a browser: this machine (the dev
/// server, the UI it serves), the published demo and whatever `--cors`
/// adds. Not any page: with no login, an open CORS let every site the
/// user visits read and change their projects.
fn allowed_origin(origin: &str, extra: &[String]) -> bool {
    let local = ["http://localhost", "http://127.0.0.1", "http://[::1]"]
        .iter()
        .any(|h| {
            origin == *h
                || origin
                    .strip_prefix(h)
                    .is_some_and(|rest| rest.starts_with(':'))
        });
    local || origin == DEMO_ORIGIN || extra.iter().any(|o| o == origin)
}

pub fn router(
    store: AppState,
    static_dir: Option<&std::path::Path>,
    model: ModelHandle,
    origins: Vec<String>,
) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(move |origin, _| {
            origin.to_str().is_ok_and(|o| allowed_origin(o, &origins))
        }))
        .allow_methods(Any)
        .allow_headers(Any);
    let api = Router::new()
        .route("/health", get(health))
        .route("/libraries", get(libraries))
        .route("/assistant", post(assistant_turn))
        .route("/projects", post(create_project).get(list_projects))
        .route("/projects/{id}", get(get_project))
        .route("/furniture", post(create_furniture).get(list_furniture))
        .route("/furniture/{id}", get(get_furniture).put(update_furniture))
        .route("/furniture/{id}/recalculate", post(recalculate))
        .route("/furniture/{id}/manufacturing-plan", post(recalculate))
        .route("/furniture/{id}/parts", get(parts))
        .route("/furniture/{id}/bom", get(bom))
        .route("/furniture/{id}/operations", get(operations))
        .route("/furniture/{id}/validate", post(validate))
        .route("/manufacturing-orders", post(create_order).get(list_orders))
        .route("/manufacturing-orders/{id}", get(get_order))
        .route("/manufacturing-orders/{id}/package", get(order_package_zip))
        .route("/manufacturing-orders/{id}/production", get(get_production))
        .route(
            "/manufacturing-orders/{id}/production/steps",
            post(set_step),
        )
        .route(
            "/manufacturing-orders/{id}/production/status",
            post(set_status),
        )
        .route("/manufacturing-orders/{id}/qc", get(get_qc).post(record_qc))
        .route(
            "/manufacturing-orders/{id}/package/{*path}",
            get(order_package_file),
        )
        .with_state(store)
        .layer(Extension(model))
        .layer(cors);
    match static_dir {
        Some(dir) => api
            .fallback_service(ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html")))),
        None => api,
    }
}

async fn health(Extension(model): Extension<ModelHandle>) -> Json<serde_json::Value> {
    Json(json!({
        "engine": rewood_core::ENGINE_VERSION,
        "schema": rewood_core::spec::SCHEMA_VERSION,
        "assistant": model.is_some(),
        "model": model.as_ref().map(|m| m.name()),
    }))
}

async fn assistant_turn(
    Extension(model): Extension<ModelHandle>,
    Body(body): Body<assistant::AssistantRequest>,
) -> ApiResult<Json<assistant::AssistantReply>> {
    let Some(model) = model else {
        return Err(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "asistente apagado: el servidor no tiene ANTHROPIC_API_KEY".into(),
        ));
    };
    assistant::run(model.as_ref(), &body)
        .await
        .map(Json)
        .map_err(|e| ApiError(StatusCode::BAD_GATEWAY, e))
}

async fn libraries() -> Json<rewood_core::library::Libraries> {
    Json(rewood_core::library::Libraries::default())
}

async fn create_project(
    State(store): State<AppState>,
    Body(body): Body<NewProject>,
) -> ApiResult<(StatusCode, Json<crate::store::Project>)> {
    Ok((StatusCode::CREATED, Json(store.create_project(&body.name)?)))
}

async fn list_projects(
    State(store): State<AppState>,
) -> ApiResult<Json<Vec<crate::store::Project>>> {
    Ok(Json(store.projects()?))
}

async fn get_project(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<crate::store::Project>> {
    Ok(Json(store.project(&id)?))
}

async fn create_furniture(
    State(store): State<AppState>,
    Body(body): Body<NewFurniture>,
) -> ApiResult<(StatusCode, Json<crate::store::Furniture>)> {
    Ok((
        StatusCode::CREATED,
        Json(store.create_furniture(&body.project_id, body.spec)?),
    ))
}

async fn list_furniture(State(store): State<AppState>) -> ApiResult<Json<Vec<FurnitureSummary>>> {
    let items = store
        .furniture_list()?
        .into_iter()
        .map(|f| FurnitureSummary {
            id: f.id,
            project_id: f.project_id,
            version: f.version,
            name: f.spec["name"].as_str().unwrap_or_default().to_string(),
            updated_at: f.updated_at,
        })
        .collect();
    Ok(Json(items))
}

async fn get_furniture(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<crate::store::Furniture>> {
    Ok(Json(store.furniture(&id)?))
}

async fn update_furniture(
    State(store): State<AppState>,
    Path(id): Path<String>,
    Body(spec): Body<FurnitureSpec>,
) -> ApiResult<Json<crate::store::Furniture>> {
    Ok(Json(store.update_furniture(&id, spec)?))
}

/// Compiling is CPU work: it runs off the async workers, so a heavy spec
/// cannot starve every other request (health included). A saved spec that
/// no longer parses (a field renamed since) comes back as a blocked plan
/// that says why, not as a 500.
async fn compile(
    spec: crate::store::StoredSpec,
) -> ApiResult<rewood_core::plan::ManufacturingPlan> {
    tokio::task::spawn_blocking(move || rewood_core::compile_json(&spec.to_string()))
        .await
        .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

async fn plan_of(store: &FsStore, id: &str) -> ApiResult<rewood_core::plan::ManufacturingPlan> {
    let f = store.furniture(id)?;
    compile(f.spec).await
}

async fn recalculate(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(plan_of(&store, &id).await?.to_json_value()))
}

async fn parts(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let plan = plan_of(&store, &id).await?.to_json_value();
    Ok(Json(
        json!({ "parts": plan["parts"], "partList": plan["partList"] }),
    ))
}

async fn bom(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let plan = plan_of(&store, &id).await?.to_json_value();
    Ok(Json(plan["bom"].clone()))
}

async fn operations(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let plan = plan_of(&store, &id).await?.to_json_value();
    let ops: Vec<serde_json::Value> = plan["parts"]
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .map(|p| json!({ "part": p["id"], "operations": p["operations"] }))
                .collect()
        })
        .unwrap_or_default();
    Ok(Json(json!(ops)))
}

async fn validate(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let plan = plan_of(&store, &id).await?;
    Ok(Json(json!({
        "status": plan.status,
        "manufacturingBlocked": plan.manufacturing_blocked,
        "diagnostics": plan.diagnostics.items,
    })))
}

async fn create_order(
    State(store): State<AppState>,
    Body(body): Body<NewOrder>,
) -> ApiResult<(StatusCode, Json<crate::store::ManufacturingOrder>)> {
    let f = store.furniture(&body.furniture_id)?;
    let plan = compile(f.spec.clone()).await?;
    if plan.manufacturing_blocked {
        return Err(ApiError(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!(
                "fabricación bloqueada: {} hallazgo(s) fatal(es); corregí la spec antes de emitir la orden",
                plan.diagnostics.count(rewood_core::diagnostics::Severity::Fatal)
            ),
        ));
    }
    let files = rewood_core::export::package(&plan);
    Ok((
        StatusCode::CREATED,
        Json(store.create_order(&f, plan, &files)?),
    ))
}

async fn list_orders(State(store): State<AppState>) -> ApiResult<Json<Vec<OrderSummary>>> {
    let items = store
        .orders()?
        .into_iter()
        .map(|o| OrderSummary {
            id: o.id,
            furniture_id: o.furniture_id,
            furniture_version: o.furniture_version,
            status: o.status,
            manufacturing_blocked: o.manufacturing_blocked,
            created_at: o.created_at,
            package_sha256: o.snapshot.package_sha256,
        })
        .collect();
    Ok(Json(items))
}

async fn get_order(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<crate::store::ManufacturingOrder>> {
    Ok(Json(store.order(&id)?))
}

async fn order_package_file(
    State(store): State<AppState>,
    Path((id, path)): Path<(String, String)>,
) -> ApiResult<Response> {
    let bytes = store.package_file(&id, &path)?;
    let mime = match path.rsplit('.').next() {
        Some("json") => "application/json",
        Some("html") => "text/html; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("csv") => "text/csv; charset=utf-8",
        Some("dxf") | Some("nc") | Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    };
    Ok(([(header::CONTENT_TYPE, mime)], bytes).into_response())
}

#[derive(Deserialize)]
pub struct PackageQuery {
    /// Provider role: only that provider's files go in the zip.
    pub role: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepBody {
    pub part: Option<String>,
    pub step: String,
    #[serde(default = "yes")]
    pub done: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
pub struct StatusBody {
    pub status: crate::production::ProductionStatus,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QcBody {
    pub part: String,
    pub length: f64,
    pub width: f64,
    pub thickness: f64,
    #[serde(default)]
    pub notes: String,
}

fn production_json(p: &crate::production::Production) -> serde_json::Value {
    let mut v = serde_json::to_value(p).unwrap_or_default();
    v["summary"] = serde_json::to_value(p.summary()).unwrap_or_default();
    v
}

async fn get_production(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(production_json(&store.production(&id)?)))
}

async fn set_step(
    State(store): State<AppState>,
    Path(id): Path<String>,
    Body(body): Body<StepBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let (p, ()) = store
        .update_production(&id, |p| {
            p.set_step(
                body.part.as_deref(),
                &body.step,
                body.done,
                &FsStore::now_string(),
            )
        })?
        .map_err(|e| ApiError(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(production_json(&p)))
}

async fn set_status(
    State(store): State<AppState>,
    Path(id): Path<String>,
    Body(body): Body<StatusBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let (p, ()) = store
        .update_production(&id, |p| p.set_status(body.status, &FsStore::now_string()))?
        .map_err(|e| ApiError(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok(Json(production_json(&p)))
}

async fn get_qc(
    State(store): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<crate::production::QcRecord>>> {
    Ok(Json(store.production(&id)?.qc))
}

async fn record_qc(
    State(store): State<AppState>,
    Path(id): Path<String>,
    Body(body): Body<QcBody>,
) -> ApiResult<(StatusCode, Json<crate::production::QcRecord>)> {
    let order = store.order(&id)?;
    let (_, rec) = store
        .update_production(&id, |p| {
            p.record_qc(
                &order.snapshot.plan,
                &body.part,
                [body.length, body.width, body.thickness],
                &body.notes,
                &FsStore::now_string(),
            )
            .cloned()
        })?
        .map_err(|e| ApiError(StatusCode::UNPROCESSABLE_ENTITY, e))?;
    Ok((StatusCode::CREATED, Json(rec)))
}

async fn order_package_zip(
    State(store): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<PackageQuery>,
) -> ApiResult<Response> {
    let role = q.role.as_deref().unwrap_or("all");
    if crate::production::files_for_role(role, "manifest.json").is_none() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            format!(
                "rol de proveedor desconocido '{role}' (cnc, cutting, assembly, purchasing, all)"
            ),
        ));
    }
    let files: std::collections::BTreeMap<String, Vec<u8>> = store
        .package_files(&id)?
        .into_iter()
        .filter(|(path, _)| crate::production::files_for_role(role, path) == Some(true))
        .collect();
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (path, bytes) in &files {
            zip.start_file(path, opts)
                .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            zip.write_all(bytes)
                .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
        zip.finish()
            .map_err(|e| ApiError(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_string()),
            (
                header::CONTENT_DISPOSITION,
                if role == "all" {
                    format!("attachment; filename=\"{id}.zip\"")
                } else {
                    format!("attachment; filename=\"{id}-{role}.zip\"")
                },
            ),
        ],
        buf.into_inner(),
    )
        .into_response())
}
