use axum::extract::State;
use axum::Json;
use serde_json::{json, Value};

use crate::error::ApiResult;
use crate::state::AppState;

/// Liveness + Docker connectivity probe. Public (no auth).
pub async fn health(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let docker_ok = state.docker.ping().await.is_ok();
    Ok(Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "docker": if docker_ok { "up" } else { "down" },
    })))
}
