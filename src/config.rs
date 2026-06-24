use std::env;

/// Runtime configuration, loaded from environment (.env in dev).
#[derive(Clone)]
pub struct Config {
    /// Address to bind the HTTP server, e.g. "0.0.0.0:8087".
    pub bind_addr: String,
    /// Bearer token required for all mutating (control) endpoints.
    pub api_token: String,
    /// Optional outbound webhook URL the relay forwards push events to.
    pub webhook_url: Option<String>,
    /// Comma-separated allowed CORS origins ("*" allows any).
    pub cors_origins: String,
    /// Path to the homelab repo root (holds stacks/ + .env) for compose actions.
    pub repo_root: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let api_token = env::var("API_TOKEN")
            .map_err(|_| anyhow::anyhow!("API_TOKEN not set — refusing to start without auth"))?;
        if api_token.trim().is_empty() {
            anyhow::bail!("API_TOKEN is empty — refusing to start without auth");
        }
        Ok(Self {
            bind_addr: env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8087".to_string()),
            api_token,
            webhook_url: env::var("WEBHOOK_URL").ok().filter(|s| !s.is_empty()),
            cors_origins: env::var("CORS_ORIGINS").unwrap_or_else(|_| "*".to_string()),
            repo_root: env::var("REPO_ROOT").unwrap_or_else(|_| "/homelab".to_string()),
        })
    }
}
