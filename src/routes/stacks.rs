use std::path::{Path as FsPath, PathBuf};

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::process::Command;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

/// A compose stack discovered under the repo, plus its live container count.
#[derive(Serialize)]
pub struct StackInfo {
    pub name: String,
    /// Containers currently running for this compose project.
    pub running: usize,
    /// Total containers (running + stopped) for this compose project.
    pub total: usize,
}

/// Allowed compose actions, mapped to the args passed to `docker compose`.
fn action_args(action: &str) -> Option<&'static [&'static str]> {
    match action {
        "up" => Some(&["up", "-d"]),
        "down" => Some(&["down"]),
        "restart" => Some(&["restart"]),
        _ => None,
    }
}

/// Resolve a stack name to its compose file, rejecting anything that could
/// escape the repo (path traversal, separators, etc.).
fn compose_file(repo_root: &str, name: &str) -> ApiResult<PathBuf> {
    let valid = !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if !valid {
        return Err(ApiError::BadRequest(format!("invalid stack name: {name}")));
    }

    // "infra" lives at the repo root; everything else under stacks/<name>/.
    let path = if name == "infra" {
        FsPath::new(repo_root).join("infra/docker-compose.yml")
    } else {
        FsPath::new(repo_root).join(format!("stacks/{name}/docker-compose.yml"))
    };
    if !path.is_file() {
        return Err(ApiError::NotFound(format!(
            "no compose file for stack {name}"
        )));
    }
    Ok(path)
}

/// GET /api/stacks — list deployable stacks with live container counts. (auth)
///
/// Lets the app / homepage launch stacks that aren't created yet (e.g. the
/// `data`/airflow stack), not just start already-created containers.
pub async fn list_stacks(State(state): State<AppState>) -> ApiResult<Json<Vec<StackInfo>>> {
    use bollard::container::ListContainersOptions;

    // Discover stack names from the filesystem.
    let mut names: Vec<String> = Vec::new();
    let stacks_dir = FsPath::new(&state.config.repo_root).join("stacks");
    if let Ok(mut rd) = tokio::fs::read_dir(&stacks_dir).await {
        while let Ok(Some(entry)) = rd.next_entry().await {
            if entry.path().join("docker-compose.yml").is_file() {
                if let Some(n) = entry.file_name().to_str() {
                    names.push(n.to_string());
                }
            }
        }
    }
    if FsPath::new(&state.config.repo_root)
        .join("infra/docker-compose.yml")
        .is_file()
    {
        names.push("infra".to_string());
    }
    names.sort();

    // Count containers per compose project in one Docker call.
    let containers = state
        .docker
        .list_containers(Some(ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await?;

    let stacks = names
        .into_iter()
        .map(|name| {
            let mut running = 0;
            let mut total = 0;
            for c in &containers {
                let project = c
                    .labels
                    .as_ref()
                    .and_then(|l| l.get("com.docker.compose.project"));
                if project.map(String::as_str) == Some(name.as_str()) {
                    total += 1;
                    if c.state.as_deref() == Some("running") {
                        running += 1;
                    }
                }
            }
            StackInfo {
                name,
                running,
                total,
            }
        })
        .collect();

    Ok(Json(stacks))
}

/// POST /api/stacks/:name/:action — run `docker compose <action>` for a stack.
/// action ∈ {up, down, restart}. (auth)
pub async fn stack_action(
    State(state): State<AppState>,
    Path((name, action)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    let args = action_args(&action)
        .ok_or_else(|| ApiError::BadRequest(format!("unsupported action: {action}")))?;
    let file = compose_file(&state.config.repo_root, &name)?;
    let env_file = FsPath::new(&state.config.repo_root).join(".env");

    let mut cmd = Command::new("docker");
    cmd.current_dir(&state.config.repo_root)
        .arg("compose")
        .arg("-f")
        .arg(&file);
    if env_file.is_file() {
        cmd.arg("--env-file").arg(&env_file);
    }
    cmd.args(args);

    tracing::info!(stack = %name, action = %action, "running docker compose");
    let output = cmd
        .output()
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("failed to spawn docker compose: {e}")))?;

    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Compose writes progress to stderr even on success; surface a bounded tail.
    let stderr = String::from_utf8_lossy(&output.stderr);
    let tail = |s: &str| {
        s.lines()
            .rev()
            .take(40)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n")
    };

    Ok(Json(json!({
        "stack": name,
        "action": action,
        "ok": output.status.success(),
        "exit_code": code,
        "stdout": tail(&stdout),
        "stderr": tail(&stderr),
    })))
}
