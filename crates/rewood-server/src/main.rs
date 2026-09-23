//! `rewood-server [--data <dir>] [--listen <addr>] [--static <dir>] [--cors <origin>]…`
//!
//! A modular monolith, as the specification suggests for a first version:
//! one process, the engine in-process, a file store that a Postgres store
//! can replace behind the same interface.

mod api;
mod assistant;
mod production;
mod store;

use std::sync::Arc;

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let mut data = std::path::PathBuf::from("data");
    let mut listen = "127.0.0.1:8080".to_string();
    let mut static_dir: Option<std::path::PathBuf> = None;
    let mut origins: Vec<String> = Vec::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--data" => data = args.next().map(Into::into).unwrap_or(data),
            "--listen" => listen = args.next().unwrap_or(listen),
            "--static" => static_dir = args.next().map(Into::into),
            "--cors" => origins.extend(args.next()),
            other => {
                eprintln!("argumento desconocido: {other}");
                eprintln!("uso: rewood-server [--data <dir>] [--listen <addr>] [--static <dir>] [--cors <origin>]");
                std::process::exit(2);
            }
        }
    }
    let store = match store::FsStore::open(&data) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!(
                "no se pudo abrir el almacenamiento en {}: {e}",
                data.display()
            );
            std::process::exit(1);
        }
    };
    let model: Option<Arc<dyn assistant::Model>> =
        assistant::AnthropicModel::from_env().map(|m| Arc::new(m) as Arc<dyn assistant::Model>);
    match &model {
        Some(m) => eprintln!("asistente: modelo {}", m.name()),
        None => eprintln!("asistente apagado (sin ANTHROPIC_API_KEY)"),
    }
    let app = api::router(store, static_dir.as_deref(), model, origins);
    let listener = tokio::net::TcpListener::bind(&listen).await.expect("bind");
    eprintln!(
        "rewood-server {} escuchando en http://{listen} (datos en {})",
        rewood_core::ENGINE_VERSION,
        data.display()
    );
    if let Some(dir) = &static_dir {
        eprintln!("sirviendo la interfaz desde {}", dir.display());
    }
    axum::serve(listener, app).await.expect("serve");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    const SPEC: &str = include_str!("../../../fixtures/basic_cabinet/input.json");

    async fn call(
        app: &axum::Router,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value, Vec<u8>) {
        let req = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(match body {
                Some(b) => Body::from(serde_json::to_vec(&b).unwrap()),
                None => Body::empty(),
            })
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
        let json = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, json, bytes)
    }

    #[tokio::test]
    async fn full_flow_project_furniture_plan_order_package() {
        let dir = tempfile::tempdir().unwrap();
        let app = api::router(
            Arc::new(store::FsStore::open(dir.path()).unwrap()),
            None,
            None,
            Vec::new(),
        );

        let (s, v, _) = call(&app, "GET", "/health", None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v["schema"], "1.0");
        assert_eq!(v["assistant"], false);

        let (s, p, _) = call(
            &app,
            "POST",
            "/projects",
            Some(serde_json::json!({ "name": "Casa García" })),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED);
        let pid = p["id"].as_str().unwrap().to_string();
        assert_eq!(pid, "prj-000001");

        let spec: serde_json::Value = serde_json::from_str(SPEC).unwrap();
        let (s, f, _) = call(
            &app,
            "POST",
            "/furniture",
            Some(serde_json::json!({ "projectId": pid, "spec": spec })),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED, "{f}");
        let fid = f["id"].as_str().unwrap().to_string();
        assert_eq!(f["version"], 1);

        let (s, plan, _) = call(&app, "POST", &format!("/furniture/{fid}/recalculate"), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(plan["status"], "ok");
        assert_eq!(plan["parts"].as_array().unwrap().len(), 9);
        // Same as compiling directly: the service adds nothing to the plan.
        let direct = rewood_core::compile_json(SPEC).to_json_value();
        assert_eq!(plan, direct);

        let (s, bom, _) = call(&app, "GET", &format!("/furniture/{fid}/bom"), None).await;
        assert_eq!(s, StatusCode::OK);
        assert!(bom["hardware"].as_array().unwrap().len() >= 3);

        let (s, val, _) = call(&app, "POST", &format!("/furniture/{fid}/validate"), None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(val["manufacturingBlocked"], false);

        // New version: wider.
        let mut spec2 = spec.clone();
        spec2["parameters"]["width"] = serde_json::json!(1200);
        let (s, f2, _) = call(&app, "PUT", &format!("/furniture/{fid}"), Some(spec2)).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(f2["version"], 2);
        assert_eq!(f2["versions"].as_array().unwrap().len(), 2);

        let (s, order, _) = call(
            &app,
            "POST",
            "/manufacturing-orders",
            Some(serde_json::json!({ "furnitureId": fid })),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED, "{order}");
        let oid = order["id"].as_str().unwrap().to_string();
        assert_eq!(order["furnitureVersion"], 2);
        assert_eq!(order["snapshot"]["plan"]["parameters"]["width"], 1200.0);
        let files = order["snapshot"]["packageFiles"].as_array().unwrap();
        assert!(files.iter().any(|f| f == "documentation/report.html"));
        let sha = order["snapshot"]["packageSha256"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(sha.len(), 64);

        // The frozen package is served file by file and as a zip.
        let (s, _, bytes) = call(
            &app,
            "GET",
            &format!("/manufacturing-orders/{oid}/package/bom/parts.csv"),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert!(String::from_utf8_lossy(&bytes).starts_with("part_ids,"));
        let (s, _, zipped) = call(
            &app,
            "GET",
            &format!("/manufacturing-orders/{oid}/package"),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(&zipped[..2], b"PK");
        let (s, _, _) = call(
            &app,
            "GET",
            &format!("/manufacturing-orders/{oid}/package/nope.txt"),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::NOT_FOUND);

        // Changing the furniture afterwards does not touch the order.
        let mut spec3 = spec.clone();
        spec3["parameters"]["width"] = serde_json::json!(700);
        call(&app, "PUT", &format!("/furniture/{fid}"), Some(spec3)).await;
        let (_, again, _) = call(&app, "GET", &format!("/manufacturing-orders/{oid}"), None).await;
        assert_eq!(again["snapshot"]["plan"]["parameters"]["width"], 1200.0);
        assert_eq!(again["snapshot"]["packageSha256"], sha);

        let (s, list, _) = call(&app, "GET", "/manufacturing-orders", None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(list.as_array().unwrap().len(), 1);

        // Production tracking: planned → in progress → done; QC judged.
        let (s, prod, _) = call(
            &app,
            "GET",
            &format!("/manufacturing-orders/{oid}/production"),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(prod["status"], "planned");
        assert_eq!(prod["summary"]["parts"], 9);
        let (s, prod, _) = call(
            &app,
            "POST",
            &format!("/manufacturing-orders/{oid}/production/steps"),
            Some(serde_json::json!({ "part": "P001", "step": "cut" })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{prod}");
        assert_eq!(prod["status"], "in_progress");
        assert_eq!(prod["summary"]["cut"], 1);
        let (s, err, _) = call(
            &app,
            "POST",
            &format!("/manufacturing-orders/{oid}/production/steps"),
            Some(serde_json::json!({ "part": "P099", "step": "cut" })),
        )
        .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY, "{err}");
        let (s, rec, _) = call(
            &app,
            "POST",
            &format!("/manufacturing-orders/{oid}/qc"),
            Some(serde_json::json!({ "part": "P001", "length": 800.5, "width": 400, "thickness": 17.8 })),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED, "{rec}");
        assert_eq!(rec["pass"], false);
        assert_eq!(rec["deviation"][0], 0.5);
        let (_, qc, _) = call(
            &app,
            "GET",
            &format!("/manufacturing-orders/{oid}/qc"),
            None,
        )
        .await;
        assert_eq!(qc.as_array().unwrap().len(), 1);
        // The record survives a fresh read (it is on disk, not in memory).
        let (_, prod, _) = call(
            &app,
            "GET",
            &format!("/manufacturing-orders/{oid}/production"),
            None,
        )
        .await;
        assert_eq!(prod["summary"]["qcFailed"], 1);

        // Provider packages: the CNC shop gets programs, not the manual.
        let (s, _, cnc_zip) = call(
            &app,
            "GET",
            &format!("/manufacturing-orders/{oid}/package?role=cnc"),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let names = zip_names(&cnc_zip);
        assert!(names.iter().any(|n| n.starts_with("cnc/")));
        assert!(!names.iter().any(|n| n == "documentation/assembly.txt"));
        let (s, _, asm_zip) = call(
            &app,
            "GET",
            &format!("/manufacturing-orders/{oid}/package?role=assembly"),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::OK);
        let names = zip_names(&asm_zip);
        assert!(names.iter().any(|n| n == "documentation/assembly.txt"));
        assert!(!names.iter().any(|n| n.starts_with("cnc/")));
        let (s, _, _) = call(
            &app,
            "GET",
            &format!("/manufacturing-orders/{oid}/package?role=painter"),
            None,
        )
        .await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    }

    fn zip_names(bytes: &[u8]) -> Vec<String> {
        let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        (0..z.len())
            .map(|i| z.by_index(i).unwrap().name().to_string())
            .collect()
    }

    #[tokio::test]
    async fn the_ui_can_call_the_api_from_another_origin_and_is_served_from_static() {
        let dir = tempfile::tempdir().unwrap();
        let ui = dir.path().join("ui");
        std::fs::create_dir_all(&ui).unwrap();
        std::fs::write(ui.join("index.html"), "<h1>rewood</h1>").unwrap();
        let app = api::router(
            Arc::new(store::FsStore::open(dir.path().join("data")).unwrap()),
            Some(&ui),
            None,
            Vec::new(),
        );
        let req = Request::builder()
            .method("OPTIONS")
            .uri("/projects")
            .header("origin", "http://localhost:5173")
            .header("access-control-request-method", "POST")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers()["access-control-allow-origin"],
            "http://localhost:5173"
        );
        // Any other site the user visits gets no CORS grant.
        let req = Request::builder()
            .method("OPTIONS")
            .uri("/projects")
            .header("origin", "https://evil.example")
            .header("access-control-request-method", "POST")
            .body(Body::empty())
            .unwrap();
        let res = app.clone().oneshot(req).await.unwrap();
        assert!(res.headers().get("access-control-allow-origin").is_none());
        // Ids come from the URL: nothing outside the store is reachable.
        std::fs::write(
            dir.path().join("secret.json"),
            r#"{"id":"x","name":"x","createdAt":"0"}"#,
        )
        .unwrap();
        let (s, _, _) = call(&app, "GET", "/projects/..%2F..%2Fsecret", None).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        // A bad body answers in JSON like every other error.
        let (s, v, _) = call(
            &app,
            "POST",
            "/projects",
            Some(serde_json::json!({ "nombre": 1 })),
        )
        .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(v["error"].is_string(), "{v}");
        // The SPA: an unknown path falls back to index.html, the API does not.
        let (s, _, bytes) = call(&app, "GET", "/", None).await;
        assert_eq!(s, StatusCode::OK);
        assert!(String::from_utf8_lossy(&bytes).contains("rewood"));
        let (s, _, bytes) = call(&app, "GET", "/anything/deep", None).await;
        assert_eq!(s, StatusCode::OK);
        assert!(String::from_utf8_lossy(&bytes).contains("rewood"));
        let (s, _, _) = call(&app, "GET", "/furniture/fur-000001", None).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn the_assistant_endpoint_runs_the_model_through_the_engine() {
        use assistant::tests::FakeModel;
        use assistant::ModelReply;
        let dir = tempfile::tempdir().unwrap();
        let spec: serde_json::Value = serde_json::from_str(SPEC).unwrap();
        let model = Arc::new(FakeModel::new(vec![
            ModelReply {
                text: "Acá va.".into(),
                spec: Some(spec),
            },
            ModelReply {
                text: "Listo, módulo de 1000 sin hallazgos.".into(),
                spec: None,
            },
        ]));
        let app = api::router(
            Arc::new(store::FsStore::open(dir.path()).unwrap()),
            None,
            Some(model as Arc<dyn assistant::Model>),
            Vec::new(),
        );
        let (s, v, _) = call(&app, "GET", "/health", None).await;
        assert_eq!(s, StatusCode::OK);
        assert_eq!(v["assistant"], true);
        let (s, v, _) = call(
            &app,
            "POST",
            "/assistant",
            Some(serde_json::json!({ "message": "un módulo básico de 1000" })),
        )
        .await;
        assert_eq!(s, StatusCode::OK, "{v}");
        assert_eq!(v["status"], "ok");
        assert_eq!(v["spec"]["id"], "basic_cabinet");
        assert_eq!(v["corrections"], 0);
        assert!(v["reply"].as_str().unwrap().starts_with("Listo"));

        // Without a model the endpoint says so instead of failing oddly.
        let app = api::router(
            Arc::new(store::FsStore::open(dir.path()).unwrap()),
            None,
            None,
            Vec::new(),
        );
        let (s, v, _) = call(
            &app,
            "POST",
            "/assistant",
            Some(serde_json::json!({ "message": "hola" })),
        )
        .await;
        assert_eq!(s, StatusCode::SERVICE_UNAVAILABLE);
        assert!(v["error"].as_str().unwrap().contains("ANTHROPIC_API_KEY"));
    }

    #[tokio::test]
    async fn a_blocked_plan_cannot_become_an_order() {
        let dir = tempfile::tempdir().unwrap();
        let app = api::router(
            Arc::new(store::FsStore::open(dir.path()).unwrap()),
            None,
            None,
            Vec::new(),
        );
        let (_, p, _) = call(
            &app,
            "POST",
            "/projects",
            Some(serde_json::json!({ "name": "x" })),
        )
        .await;
        let spec: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/invalid_cabinet/input.json"))
                .unwrap();
        let (s, f, _) = call(
            &app,
            "POST",
            "/furniture",
            Some(serde_json::json!({ "projectId": p["id"], "spec": spec })),
        )
        .await;
        assert_eq!(s, StatusCode::CREATED);
        let (s, body, _) = call(
            &app,
            "POST",
            "/manufacturing-orders",
            Some(serde_json::json!({ "furnitureId": f["id"] })),
        )
        .await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(body["error"].as_str().unwrap().contains("bloqueada"));
        let (s, _, _) = call(&app, "GET", "/furniture/fur-000099", None).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
    }
}
