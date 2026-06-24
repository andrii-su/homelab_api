mod auth;
mod config;
mod error;
mod routes;
mod state;

use axum::http;
use axum::routing::{get, post};
use axum::{middleware, Router};
use bollard::Docker;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = Config::from_env()?;
    let docker = Docker::connect_with_local_defaults()?;
    docker
        .ping()
        .await
        .map_err(|e| anyhow::anyhow!("cannot reach Docker daemon: {e}"))?;

    let bind_addr = config.bind_addr.clone();
    let state = AppState::new(config, docker);

    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("homelab_api listening on http://{bind_addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

fn build_router(state: AppState) -> Router {
    let cors = build_cors(&state.config.cors_origins);

    // Protected routes: anything that controls services or reads detailed data.
    let protected = Router::new()
        .route("/api/services", get(routes::services::list_services))
        .route(
            "/api/services/:name/start",
            post(routes::services::start_service),
        )
        .route(
            "/api/services/:name/stop",
            post(routes::services::stop_service),
        )
        .route(
            "/api/services/:name/restart",
            post(routes::services::restart_service),
        )
        .route(
            "/api/services/:name/logs",
            get(routes::services::service_logs),
        )
        .route(
            "/api/services/:name/stats",
            get(routes::stats::service_stats),
        )
        .route("/api/notify", post(routes::webhook::notify))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_bearer,
        ));

    // Public routes: liveness only.
    let public = Router::new().route("/health", get(routes::health::health));

    public
        .merge(protected)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn build_cors(origins: &str) -> CorsLayer {
    if origins.trim() == "*" {
        CorsLayer::very_permissive()
    } else {
        let list: Vec<_> = origins
            .split(',')
            .filter_map(|o| http::HeaderValue::from_str(o.trim()).ok())
            .collect();
        CorsLayer::new().allow_origin(AllowOrigin::list(list))
    }
}
