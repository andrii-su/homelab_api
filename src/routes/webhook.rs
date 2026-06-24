use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// Incoming push event — from container hooks, monitoring alerts, etc.
#[derive(Deserialize, Serialize)]
pub struct PushEvent {
    pub title: String,
    pub message: String,
    /// Optional priority hint passed through to the downstream provider.
    #[serde(default)]
    pub priority: Option<String>,
    /// Optional tags / categories (e.g. ["pihole", "down"]).
    #[serde(default)]
    pub tags: Vec<String>,
}

/// POST /api/notify — generic webhook relay. (auth)
///
/// Forwards the event to `WEBHOOK_URL` (ntfy, Slack-compatible, APNs proxy,
/// whatever you point it at later). If no URL is configured it accepts and
/// logs the event so the pipeline still works end-to-end during setup.
pub async fn notify(
    State(state): State<AppState>,
    Json(event): Json<PushEvent>,
) -> ApiResult<Json<Value>> {
    match &state.config.webhook_url {
        Some(url) => {
            let resp = state
                .http
                .post(url)
                .json(&event)
                .send()
                .await?
                .error_for_status()
                .map_err(ApiError::Upstream)?;
            tracing::info!(status = %resp.status(), title = %event.title, "relayed push event");
            Ok(Json(json!({ "relayed": true, "title": event.title })))
        }
        None => {
            tracing::warn!(title = %event.title, message = %event.message, "WEBHOOK_URL unset — event logged, not relayed");
            Ok(Json(json!({ "relayed": false, "logged": true, "title": event.title })))
        }
    }
}
