use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;

use crate::error::ApiError;
use crate::state::AppState;

/// Middleware enforcing `Authorization: Bearer <API_TOKEN>` on protected routes.
///
/// Applied only to mutating/control endpoints. Read-only health is left open
/// so the Swift app / uptime checks can probe liveness without a secret.
pub async fn require_bearer(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let header = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    let token = header
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(str::trim);

    match token {
        Some(t) if constant_time_eq(t, &state.config.api_token) => Ok(next.run(req).await),
        _ => Err(ApiError::Unauthorized),
    }
}

/// Length-independent constant-time comparison to avoid token timing leaks.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
