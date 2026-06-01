use std::path::PathBuf;

use agent_policy_core::render_bundle_json;
use agent_policy_discover::discover;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::net::TcpListener;

use crate::cli::{GetArgs, GlobalArgs, InstructionDiscoveryMode, ServeArgs};
use crate::commands::get::build_instruction_bundle_for_get;
use crate::commands::inspect::{inspect_repo, render_inspection_json};

#[derive(Clone, Debug)]
struct ServeState {
    repo: Option<PathBuf>,
    config: Option<PathBuf>,
    no_network: bool,
}

#[derive(Debug, Deserialize)]
struct RepoRequest {
    #[serde(default)]
    repo: Option<PathBuf>,
    #[serde(default)]
    config: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct InstructionsRequest {
    #[serde(default)]
    repo: Option<PathBuf>,
    #[serde(default)]
    config: Option<PathBuf>,
    #[serde(default)]
    task: Option<String>,
    #[serde(default, rename = "type", alias = "task_type")]
    task_type: Option<String>,
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    risk: Vec<String>,
    #[serde(default)]
    max_instructions: Option<u32>,
    #[serde(default)]
    max_tokens: Option<u32>,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

pub(crate) fn run(global: &GlobalArgs, args: ServeArgs) -> anyhow::Result<()> {
    ensure_loopback_host(&args.host)?;

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let bind_addr = bind_addr(&args.host, args.port);
        let listener = TcpListener::bind(&bind_addr).await?;
        let local_addr = listener.local_addr()?;
        eprintln!("agent-policy serving on http://{local_addr}");
        axum::serve(listener, app(global)).await?;
        Ok(())
    })
}

fn app(global: &GlobalArgs) -> Router {
    let state = ServeState {
        repo: global.repo.clone(),
        config: global.config.clone(),
        no_network: global.no_network,
    };

    Router::new()
        .route("/health", get(health))
        .route("/instructions", post(instructions))
        .route("/discover", post(discover_repo))
        .route("/inspect", post(inspect))
        .with_state(state)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn instructions(
    State(state): State<ServeState>,
    Json(request): Json<InstructionsRequest>,
) -> Result<Json<Value>, ApiError> {
    reject_request_paths(request.repo.as_ref(), request.config.as_ref())?;

    let global = request_global(&state);
    let args = GetArgs {
        task: request.task,
        task_type: request.task_type,
        files: request.files,
        risk: request.risk,
        max_instructions: request.max_instructions,
        max_tokens: request.max_tokens,
        instruction_mode: InstructionDiscoveryMode::Generic,
    };
    let bundle = build_instruction_bundle_for_get(&global, &args).map_err(ApiError::internal)?;
    let value = serde_json::from_str(&render_bundle_json(&bundle).map_err(ApiError::internal)?)
        .map_err(ApiError::internal)?;
    Ok(Json(value))
}

async fn discover_repo(
    State(state): State<ServeState>,
    Json(request): Json<RepoRequest>,
) -> Result<Json<Value>, ApiError> {
    reject_request_paths(request.repo.as_ref(), request.config.as_ref())?;

    let repo = state.repo.unwrap_or_else(|| PathBuf::from("."));
    let discovered = discover(repo).map_err(ApiError::internal)?;
    let value = serde_json::to_value(discovered).map_err(ApiError::internal)?;
    Ok(Json(value))
}

async fn inspect(
    State(state): State<ServeState>,
    Json(request): Json<RepoRequest>,
) -> Result<Json<Value>, ApiError> {
    reject_request_paths(request.repo.as_ref(), request.config.as_ref())?;

    let repo = state.repo.unwrap_or_else(|| PathBuf::from("."));
    let discovered = discover(&repo).map_err(ApiError::internal)?;
    let report = inspect_repo(&repo, discovered);
    let value =
        serde_json::from_str(&render_inspection_json(&report)).map_err(ApiError::internal)?;
    Ok(Json(value))
}

fn request_global(state: &ServeState) -> GlobalArgs {
    GlobalArgs {
        repo: state.repo.clone(),
        config: state.config.clone(),
        format: None,
        verbose: false,
        quiet: false,
        no_network: state.no_network,
    }
}

fn bind_addr(host: &str, port: u16) -> String {
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V6(_)) => format!("[{host}]:{port}"),
        _ => format!("{host}:{port}"),
    }
}

fn ensure_loopback_host(host: &str) -> anyhow::Result<()> {
    if host.eq_ignore_ascii_case("localhost") {
        return Ok(());
    }

    let ip: IpAddr = host.parse().map_err(|_| {
        anyhow::anyhow!("serve --host must be a loopback IP address or localhost; got {host:?}")
    })?;
    if ip.is_loopback() {
        Ok(())
    } else {
        anyhow::bail!("serve --host must be loopback-only; got {host}")
    }
}

fn reject_request_paths(repo: Option<&PathBuf>, config: Option<&PathBuf>) -> Result<(), ApiError> {
    if repo.is_some() || config.is_some() {
        return Err(ApiError::bad_request(
            "request-level repo/config paths are not allowed; start the server with --repo/--config instead",
        ));
    }
    Ok(())
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.into(),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "status": "error",
            "code": self.code,
            "message": self.message,
        }));
        (self.status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::{app, bind_addr, ensure_loopback_host};
    use crate::cli::{GetArgs, GlobalArgs, InstructionDiscoveryMode};
    use crate::commands::get::build_instruction_bundle_for_get;
    use agent_policy_core::render_bundle_json;
    use axum::body::{to_bytes, Body};
    use axum::http::{header, Method, Request, StatusCode};
    use serde_json::{json, Value};
    use std::path::PathBuf;
    use tower::ServiceExt;

    fn global() -> GlobalArgs {
        GlobalArgs {
            repo: Some(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/simple-repo"),
            ),
            config: None,
            format: None,
            verbose: false,
            quiet: false,
            no_network: true,
        }
    }

    #[test]
    fn serve_host_must_be_loopback() {
        assert!(ensure_loopback_host("127.0.0.1").is_ok());
        assert!(ensure_loopback_host("localhost").is_ok());
        assert!(ensure_loopback_host("::1").is_ok());
        assert!(ensure_loopback_host("0.0.0.0").is_err());
        assert!(ensure_loopback_host("192.168.1.10").is_err());
        assert!(ensure_loopback_host("example.com").is_err());
    }

    #[test]
    fn bind_addr_brackets_ipv6_hosts() {
        assert_eq!(bind_addr("127.0.0.1", 8765), "127.0.0.1:8765");
        assert_eq!(bind_addr("localhost", 8765), "localhost:8765");
        assert_eq!(bind_addr("::1", 8765), "[::1]:8765");
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let response = app(&global())
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value, json!({ "status": "ok" }));
    }

    #[tokio::test]
    async fn instructions_matches_cli_get_json_shape() {
        let request_body = json!({
            "task": "fix refund retry handling",
            "type": "fix_bug",
            "files": ["src/payments/refunds.ts"],
            "risk": ["payments"],
            "max_instructions": 4,
            "max_tokens": 600
        });
        let response = app(&global())
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/instructions")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(request_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let actual: Value = serde_json::from_slice(&body).unwrap();

        let args = GetArgs {
            task: Some("fix refund retry handling".to_string()),
            task_type: Some("fix_bug".to_string()),
            files: vec!["src/payments/refunds.ts".to_string()],
            risk: vec!["payments".to_string()],
            max_instructions: Some(4),
            max_tokens: Some(600),
            instruction_mode: InstructionDiscoveryMode::Generic,
        };
        let expected_bundle = build_instruction_bundle_for_get(&global(), &args).unwrap();
        let expected: Value =
            serde_json::from_str(&render_bundle_json(&expected_bundle).unwrap()).unwrap();

        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn endpoints_reject_request_level_paths() {
        for path in ["/instructions", "/discover", "/inspect"] {
            for request_body in [
                json!({ "repo": "/tmp/other-repo" }),
                json!({ "config": "/tmp/other-config.json" }),
            ] {
                let response = app(&global())
                    .oneshot(
                        Request::builder()
                            .method(Method::POST)
                            .uri(path)
                            .header(header::CONTENT_TYPE, "application/json")
                            .body(Body::from(request_body.to_string()))
                            .unwrap(),
                    )
                    .await
                    .unwrap();

                assert_eq!(response.status(), StatusCode::BAD_REQUEST);
                let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
                let value: Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(value["code"], "bad_request");
            }
        }
    }

    #[tokio::test]
    async fn discover_and_inspect_return_json() {
        for path in ["/discover", "/inspect"] {
            let response = app(&global())
                .oneshot(
                    Request::builder()
                        .method(Method::POST)
                        .uri(path)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let value: Value = serde_json::from_slice(&body).unwrap();
            assert!(value.as_object().is_some());
        }
    }
}
