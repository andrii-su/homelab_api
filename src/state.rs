use std::sync::Arc;

use bollard::Docker;

use crate::config::Config;

/// Shared application state passed to every handler.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub docker: Arc<Docker>,
    pub http: reqwest::Client,
}

impl AppState {
    pub fn new(config: Config, docker: Docker) -> Self {
        Self {
            config: Arc::new(config),
            docker: Arc::new(docker),
            http: reqwest::Client::new(),
        }
    }
}
