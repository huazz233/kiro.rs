//! 匿名健康检查端点
//!
//! 提供无需认证的健康检查接口，供 Prometheus / Uptime Kuma 等监控工具使用。
//!
//! # 端点
//! - `GET /health` - 基础健康检查
//! - `GET /provider_health` - 凭据提供者健康状态

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::kiro::token_manager::MultiTokenManager;

// ============ 请求/响应类型 ============

/// `/health` 响应
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: String,
}

/// `/provider_health` 查询参数
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealthQuery {
    /// 不健康比率阈值，超过此值 summaryHealth 为 false
    /// 默认 0.0001（即任何不健康凭据都会导致 summaryHealth 为 false）
    pub unhealth_ratio_threshold: Option<f64>,
}

/// `/provider_health` 响应中的单个凭据项
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealthItem {
    pub id: u64,
    pub healthy: bool,
    pub disabled: bool,
    pub disabled_reason: Option<String>,
    pub last_used: Option<String>,
}

/// `/provider_health` 响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealthResponse {
    pub timestamp: String,
    pub items: Vec<ProviderHealthItem>,
    pub count: usize,
    pub unhealthy_count: usize,
    pub unhealthy_ratio: f64,
    pub summary_health: bool,
}

// ============ Handlers ============

/// `GET /health` — 基础健康检查（匿名）
async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        timestamp: Utc::now().to_rfc3339(),
    })
}

/// `GET /provider_health` — 凭据提供者健康状态（匿名）
async fn provider_health(
    State(token_manager): State<Arc<MultiTokenManager>>,
    Query(params): Query<ProviderHealthQuery>,
) -> Json<ProviderHealthResponse> {
    let threshold = params.unhealth_ratio_threshold.unwrap_or(0.0001);
    let snapshot = token_manager.snapshot();

    let items: Vec<ProviderHealthItem> = snapshot
        .entries
        .iter()
        .map(|entry| {
            // 凭据被视为"不健康"：已禁用 或 有失败记录
            let healthy = !entry.disabled && entry.failure_count == 0;
            ProviderHealthItem {
                id: entry.id,
                healthy,
                disabled: entry.disabled,
                disabled_reason: entry.disabled_reason.clone(),
                last_used: entry.last_used_at.clone(),
            }
        })
        .collect();

    let count = items.len();
    let unhealthy_count = items.iter().filter(|i| !i.healthy).count();
    let unhealthy_ratio = if count > 0 {
        unhealthy_count as f64 / count as f64
    } else {
        0.0
    };
    let summary_health = unhealthy_ratio <= threshold;

    Json(ProviderHealthResponse {
        timestamp: Utc::now().to_rfc3339(),
        items,
        count,
        unhealthy_count,
        unhealthy_ratio,
        summary_health,
    })
}

// ============ Router ============

/// 创建匿名健康检查路由
///
/// 这些端点不需要任何认证，独立于 auth_middleware。
pub fn create_health_router(token_manager: Arc<MultiTokenManager>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/provider_health", get(provider_health))
        .with_state(token_manager)
}

// ============ Tests ============

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 health_check 返回合法 JSON
    #[tokio::test]
    async fn test_health_returns_valid_json() {
        let Json(resp) = health_check().await;

        assert_eq!(resp.status, "healthy");
        assert!(
            chrono::DateTime::parse_from_rfc3339(&resp.timestamp).is_ok(),
            "timestamp should be valid RFC3339: {}",
            resp.timestamp
        );
    }
}
