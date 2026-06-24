use axum::extract::{Path, State};
use axum::Json;
use bollard::container::{
    ListContainersOptions, RestartContainerOptions, StartContainerOptions, StopContainerOptions,
};
use serde::Serialize;
use serde_json::{json, Value};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Serialize)]
pub struct ServiceSummary {
    pub id: String,
    pub name: String,
    /// Compose project label, if present (e.g. "media", "infra").
    pub stack: Option<String>,
    pub image: Option<String>,
    pub state: Option<String>,
    pub status: Option<String>,
}

/// GET /api/services — list all containers with compose metadata.
/// Replaces the Swift app scraping the Homepage dashboard (:3002).
pub async fn list_services(State(state): State<AppState>) -> ApiResult<Json<Vec<ServiceSummary>>> {
    let opts = ListContainersOptions::<String> {
        all: true,
        ..Default::default()
    };
    let containers = state.docker.list_containers(Some(opts)).await?;

    let services = containers
        .into_iter()
        .map(|c| {
            let labels = c.labels.unwrap_or_default();
            ServiceSummary {
                id: c.id.unwrap_or_default(),
                name: c
                    .names
                    .and_then(|n| n.into_iter().next())
                    .map(|n| n.trim_start_matches('/').to_string())
                    .unwrap_or_default(),
                stack: labels.get("com.docker.compose.project").cloned(),
                image: c.image,
                state: c.state,
                status: c.status,
            }
        })
        .collect();

    Ok(Json(services))
}

/// POST /api/services/:name/start — start a stopped container. (auth)
pub async fn start_service(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResult<Json<Value>> {
    state
        .docker
        .start_container(&name, None::<StartContainerOptions<String>>)
        .await
        .map_err(map_not_found)?;
    Ok(Json(
        json!({ "service": name, "action": "start", "ok": true }),
    ))
}

/// POST /api/services/:name/stop — stop a running container. (auth)
pub async fn stop_service(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResult<Json<Value>> {
    let opts = StopContainerOptions { t: 10 };
    state
        .docker
        .stop_container(&name, Some(opts))
        .await
        .map_err(map_not_found)?;
    Ok(Json(
        json!({ "service": name, "action": "stop", "ok": true }),
    ))
}

/// POST /api/services/:name/restart — restart a container. (auth)
pub async fn restart_service(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResult<Json<Value>> {
    let opts = RestartContainerOptions { t: 10 };
    state
        .docker
        .restart_container(&name, Some(opts))
        .await
        .map_err(map_not_found)?;
    Ok(Json(
        json!({ "service": name, "action": "restart", "ok": true }),
    ))
}

/// GET /api/services/:name/logs — last N lines of container logs. (auth)
pub async fn service_logs(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResult<Json<Value>> {
    use bollard::container::LogsOptions;
    use futures_util::StreamExt;

    // Collect a bounded tail of logs. For live streaming, return the stream
    // as an SSE/WebSocket response instead — see README.
    let opts = LogsOptions::<String> {
        stdout: true,
        stderr: true,
        tail: "200".to_string(),
        ..Default::default()
    };
    let mut stream = state.docker.logs(&name, Some(opts));
    let mut lines: Vec<String> = Vec::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(output) => lines.push(output.to_string()),
            Err(e) => return Err(ApiError::Docker(e)),
        }
    }
    Ok(Json(json!({ "service": name, "lines": lines })))
}

/// Map Docker 404s to a clean API NotFound instead of a 500.
fn map_not_found(e: bollard::errors::Error) -> ApiError {
    match &e {
        bollard::errors::Error::DockerResponseServerError {
            status_code: 404, ..
        } => ApiError::NotFound("container not found".to_string()),
        _ => ApiError::Docker(e),
    }
}
