use axum::extract::{Path, State};
use axum::Json;
use bollard::container::{MemoryStatsStats, StatsOptions};
use futures_util::StreamExt;
use serde::Serialize;

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;

#[derive(Serialize)]
pub struct ContainerStats {
    pub name: String,
    pub cpu_percent: f64,
    pub mem_used_bytes: u64,
    pub mem_limit_bytes: u64,
    pub mem_percent: f64,
}

/// GET /api/services/:name/stats — one-shot CPU/memory snapshot. (auth)
pub async fn service_stats(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResult<Json<ContainerStats>> {
    let opts = StatsOptions {
        stream: false,
        one_shot: false,
    };
    let mut stream = state.docker.stats(&name, Some(opts));
    let s = stream
        .next()
        .await
        .ok_or_else(|| ApiError::NotFound("no stats for container".to_string()))??;

    // CPU percent: delta(container) / delta(system) * online CPUs * 100.
    let cpu_delta = s.cpu_stats.cpu_usage.total_usage as f64
        - s.precpu_stats.cpu_usage.total_usage as f64;
    let sys_delta = s.cpu_stats.system_cpu_usage.unwrap_or(0) as f64
        - s.precpu_stats.system_cpu_usage.unwrap_or(0) as f64;
    let online = s.cpu_stats.online_cpus.unwrap_or(1).max(1) as f64;
    let cpu_percent = if sys_delta > 0.0 && cpu_delta > 0.0 {
        (cpu_delta / sys_delta) * online * 100.0
    } else {
        0.0
    };

    // Memory: subtract cache to match `docker stats` reporting where possible.
    let used = s.memory_stats.usage.unwrap_or(0);
    let cache = match s.memory_stats.stats {
        Some(MemoryStatsStats::V1(v1)) => v1.cache,
        Some(MemoryStatsStats::V2(v2)) => v2.inactive_file,
        None => 0,
    };
    let mem_used = used.saturating_sub(cache);
    let mem_limit = s.memory_stats.limit.unwrap_or(0);
    let mem_percent = if mem_limit > 0 {
        (mem_used as f64 / mem_limit as f64) * 100.0
    } else {
        0.0
    };

    Ok(Json(ContainerStats {
        name,
        cpu_percent: (cpu_percent * 100.0).round() / 100.0,
        mem_used_bytes: mem_used,
        mem_limit_bytes: mem_limit,
        mem_percent: (mem_percent * 100.0).round() / 100.0,
    }))
}
