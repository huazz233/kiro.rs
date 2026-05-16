//! Kiro API Provider
//!
//! 核心组件，负责与 Kiro API 通信
//! 支持流式和非流式请求
//! 支持多凭据故障转移和重试
//! 支持按凭据级 endpoint 切换不同 Kiro API 端点

use futures::StreamExt;
use reqwest::Client;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

use crate::http_client::{ProxyConfig, build_client};
use crate::kiro::endpoint::{KiroEndpoint, RequestContext};
use crate::kiro::machine_id;
use crate::kiro::model::credentials::KiroCredentials;
use crate::kiro::token_manager::MultiTokenManager;
use crate::model::config::TlsBackend;
use parking_lot::Mutex;

/// 每个凭据的最大重试次数
const MAX_RETRIES_PER_CREDENTIAL: usize = 3;

/// 总重试次数硬上限（避免无限重试）
const MAX_TOTAL_RETRIES: usize = 9;

/// 流式请求首 chunk 等待超时（秒）
const FIRST_TOKEN_TIMEOUT_SECS: u64 = 15;

/// Kiro 上游错误分类
///
/// 将 HTTP 状态码 + 响应体统一归类，替代散落的 inline match。
/// 每个 variant 决定重试策略：是否重试、是否切换凭据、是否禁用凭据。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KiroErrorKind {
    /// 401/403 — 凭据或 token 无效，需切换凭据
    RefreshTokenInvalid,
    /// 408/429/5xx — 上游瞬态错误，重试但不切换凭据
    RateLimit,
    /// 400 + body 含 "CONTENT_LENGTH_EXCEEDS_THRESHOLD"
    ContentLengthExceeded,
    /// 400 + body 含 "INVALID_MODEL_ID"
    InvalidModel,
    /// 422 — 请求语义错误，不应重试
    Fatal,
    /// 其他未知错误 — 当作可重试的瞬态错误
    Recoverable,
}

/// 根据 HTTP 状态码和响应体分类 Kiro 上游错误
fn classify(status: reqwest::StatusCode, body: &str) -> KiroErrorKind {
    match status.as_u16() {
        401 | 403 => KiroErrorKind::RefreshTokenInvalid,
        408 | 429 => KiroErrorKind::RateLimit,
        400 => {
            if body.contains("CONTENT_LENGTH_EXCEEDS_THRESHOLD") {
                KiroErrorKind::ContentLengthExceeded
            } else if body.contains("INVALID_MODEL_ID") {
                KiroErrorKind::InvalidModel
            } else {
                KiroErrorKind::Fatal
            }
        }
        422 => KiroErrorKind::Fatal,
        code if (500..600).contains(&code) => KiroErrorKind::RateLimit,
        _ => KiroErrorKind::Recoverable,
    }
}

/// Kiro API Provider
///
/// 核心组件，负责与 Kiro API 通信
/// 支持多凭据故障转移和重试机制
/// 按凭据 `endpoint` 字段选择 [`KiroEndpoint`] 实现
pub struct KiroProvider {
    token_manager: Arc<MultiTokenManager>,
    /// 全局代理配置（用于凭据无自定义代理时的回退）
    global_proxy: Option<ProxyConfig>,
    /// Client 缓存：key = effective proxy config, value = reqwest::Client
    /// 不同代理配置的凭据使用不同的 Client，共享相同代理的凭据复用 Client
    client_cache: Mutex<HashMap<Option<ProxyConfig>, Client>>,
    /// TLS 后端配置
    tls_backend: TlsBackend,
    /// 端点实现注册表（key: endpoint 名称）
    endpoints: HashMap<String, Arc<dyn KiroEndpoint>>,
    /// 默认端点名称（凭据未指定 endpoint 时使用）
    default_endpoint: String,
}

impl KiroProvider {
    /// 创建带代理配置和端点注册表的 KiroProvider 实例
    ///
    /// # Arguments
    /// * `token_manager` - 多凭据 Token 管理器
    /// * `proxy` - 全局代理配置
    /// * `endpoints` - 端点名 → 实现的注册表（至少包含 `default_endpoint` 对应条目）
    /// * `default_endpoint` - 凭据未显式指定 endpoint 时使用的名称
    pub fn with_proxy(
        token_manager: Arc<MultiTokenManager>,
        proxy: Option<ProxyConfig>,
        endpoints: HashMap<String, Arc<dyn KiroEndpoint>>,
        default_endpoint: String,
    ) -> Self {
        assert!(
            endpoints.contains_key(&default_endpoint),
            "默认端点 {} 未在 endpoints 注册表中",
            default_endpoint
        );
        let tls_backend = token_manager.config().tls_backend;
        // 预热：构建全局代理对应的 Client
        let initial_client = build_client(proxy.as_ref(), 720, tls_backend)
            .expect("创建 HTTP 客户端失败");
        let mut cache = HashMap::new();
        cache.insert(proxy.clone(), initial_client);

        Self {
            token_manager,
            global_proxy: proxy,
            client_cache: Mutex::new(cache),
            tls_backend,
            endpoints,
            default_endpoint,
        }
    }

    /// 根据凭据的代理配置获取（或创建并缓存）对应的 reqwest::Client
    fn client_for(&self, credentials: &KiroCredentials) -> anyhow::Result<Client> {
        let effective = credentials.effective_proxy(self.global_proxy.as_ref());
        let mut cache = self.client_cache.lock();
        if let Some(client) = cache.get(&effective) {
            return Ok(client.clone());
        }
        let client = build_client(effective.as_ref(), 720, self.tls_backend)?;
        cache.insert(effective, client.clone());
        Ok(client)
    }

    /// 根据凭据选择 endpoint 实现
    fn endpoint_for(
        &self,
        credentials: &KiroCredentials,
    ) -> anyhow::Result<Arc<dyn KiroEndpoint>> {
        let name = credentials
            .endpoint
            .as_deref()
            .unwrap_or(&self.default_endpoint);
        self.endpoints
            .get(name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("未知端点: {}", name))
    }

    /// 发送非流式 API 请求
    ///
    /// 支持多凭据故障转移（见 [`Self::call_api_with_retry`]）
    pub async fn call_api(&self, request_body: &str) -> anyhow::Result<reqwest::Response> {
        self.call_api_with_retry(request_body, false).await
    }

    /// 发送流式 API 请求
    pub async fn call_api_stream(&self, request_body: &str) -> anyhow::Result<reqwest::Response> {
        self.call_api_with_retry(request_body, true).await
    }

    /// 发送 MCP API 请求（WebSearch 等工具调用）
    pub async fn call_mcp(&self, request_body: &str) -> anyhow::Result<reqwest::Response> {
        self.call_mcp_with_retry(request_body).await
    }

    /// 内部方法：带重试逻辑的 MCP API 调用
    async fn call_mcp_with_retry(&self, request_body: &str) -> anyhow::Result<reqwest::Response> {
        let total_credentials = self.token_manager.total_count();
        let max_retries = (total_credentials * MAX_RETRIES_PER_CREDENTIAL).min(MAX_TOTAL_RETRIES);
        let mut last_error: Option<anyhow::Error> = None;
        let mut force_refreshed: HashSet<u64> = HashSet::new();

        for attempt in 0..max_retries {
            // MCP 调用（WebSearch 等工具）不涉及模型选择，无需按模型过滤凭据
            let ctx = match self.token_manager.acquire_context(None).await {
                Ok(c) => c,
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            };

            let config = self.token_manager.config();
            let machine_id = machine_id::generate_from_credentials(&ctx.credentials, config);

            let endpoint = match self.endpoint_for(&ctx.credentials) {
                Ok(e) => e,
                Err(e) => {
                    last_error = Some(e);
                    // endpoint 解析失败：记为失败，换下一张凭据
                    self.token_manager.report_failure(ctx.id);
                    continue;
                }
            };

            let rctx = RequestContext {
                credentials: &ctx.credentials,
                token: &ctx.token,
                machine_id: &machine_id,
                config,
            };

            let url = endpoint.mcp_url(&rctx);
            let body = endpoint.transform_mcp_body(request_body, &rctx);

            let base = self
                .client_for(&ctx.credentials)?
                .post(&url)
                .body(body)
                .header("content-type", "application/json")
                .header("Connection", "close");
            let request = endpoint.decorate_mcp(base, &rctx);

            let response = match request.send().await {
                Ok(resp) => resp,
                Err(e) => {
                    tracing::warn!(
                        "MCP 请求发送失败（尝试 {}/{}）: {}",
                        attempt + 1,
                        max_retries,
                        e
                    );
                    last_error = Some(e.into());
                    if attempt + 1 < max_retries {
                        sleep(Self::retry_delay(attempt)).await;
                    }
                    continue;
                }
            };

            let status = response.status();

            // 成功响应
            if status.is_success() {
                self.token_manager.report_success(ctx.id);
                return Ok(response);
            }

            // 失败响应
            let body = response.text().await.unwrap_or_default();

            // 402 额度用尽（独立于 classify，因为需要 endpoint 判断）
            if status.as_u16() == 402 && endpoint.is_monthly_request_limit(&body) {
                let has_available = self.token_manager.report_quota_exhausted(ctx.id);
                if !has_available {
                    anyhow::bail!("MCP 请求失败（所有凭据已用尽）: {} {}", status, body);
                }
                last_error = Some(anyhow::anyhow!("MCP 请求失败: {} {}", status, body));
                continue;
            }

            match classify(status, &body) {
                KiroErrorKind::RefreshTokenInvalid => {
                    // token 被上游失效：先尝试 force-refresh，每凭据仅一次机会
                    if endpoint.is_bearer_token_invalid(&body)
                        && !force_refreshed.contains(&ctx.id)
                    {
                        force_refreshed.insert(ctx.id);
                        tracing::info!("凭据 #{} token 疑似被上游失效，尝试强制刷新", ctx.id);
                        if self
                            .token_manager
                            .force_refresh_token_for(ctx.id)
                            .await
                            .is_ok()
                        {
                            tracing::info!("凭据 #{} token 强制刷新成功，重试请求", ctx.id);
                            continue;
                        }
                        tracing::warn!("凭据 #{} token 强制刷新失败，计入失败", ctx.id);
                    }

                    let has_available = self.token_manager.report_failure(ctx.id);
                    if !has_available {
                        anyhow::bail!("MCP 请求失败（所有凭据已用尽）: {} {}", status, body);
                    }
                    last_error = Some(anyhow::anyhow!("MCP 请求失败: {} {}", status, body));
                    continue;
                }
                KiroErrorKind::RateLimit => {
                    tracing::warn!(
                        "MCP 请求失败（上游瞬态错误，尝试 {}/{}）: {} {}",
                        attempt + 1,
                        max_retries,
                        status,
                        body
                    );
                    last_error = Some(anyhow::anyhow!("MCP 请求失败: {} {}", status, body));
                    if attempt + 1 < max_retries {
                        sleep(Self::retry_delay(attempt)).await;
                    }
                    continue;
                }
                KiroErrorKind::Fatal
                | KiroErrorKind::ContentLengthExceeded
                | KiroErrorKind::InvalidModel => {
                    anyhow::bail!("MCP 请求失败: {} {}", status, body);
                }
                KiroErrorKind::Recoverable => {
                    // 兜底：当作可重试的瞬态错误处理
                    last_error = Some(anyhow::anyhow!("MCP 请求失败: {} {}", status, body));
                    if attempt + 1 < max_retries {
                        sleep(Self::retry_delay(attempt)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!("MCP 请求失败：已达到最大重试次数（{}次）", max_retries)
        }))
    }

    /// 内部方法：带重试逻辑的 API 调用
    ///
    /// 重试策略：
    /// - 每个凭据最多重试 MAX_RETRIES_PER_CREDENTIAL 次
    /// - 总重试次数 = min(凭据数量 × 每凭据重试次数, MAX_TOTAL_RETRIES)
    /// - 硬上限 9 次，避免无限重试
    async fn call_api_with_retry(
        &self,
        request_body: &str,
        is_stream: bool,
    ) -> anyhow::Result<reqwest::Response> {
        let total_credentials = self.token_manager.total_count();
        let max_retries = (total_credentials * MAX_RETRIES_PER_CREDENTIAL).min(MAX_TOTAL_RETRIES);
        let mut last_error: Option<anyhow::Error> = None;
        let mut force_refreshed: HashSet<u64> = HashSet::new();
        let api_type = if is_stream { "流式" } else { "非流式" };

        // 尝试从请求体中提取模型信息
        let model = Self::extract_model_from_request(request_body);

        for attempt in 0..max_retries {
            // 获取调用上下文（绑定 index、credentials、token）
            let ctx = match self.token_manager.acquire_context(model.as_deref()).await {
                Ok(c) => c,
                Err(e) => {
                    last_error = Some(e);
                    continue;
                }
            };

            let config = self.token_manager.config();
            let machine_id = machine_id::generate_from_credentials(&ctx.credentials, config);

            let endpoint = match self.endpoint_for(&ctx.credentials) {
                Ok(e) => e,
                Err(e) => {
                    last_error = Some(e);
                    self.token_manager.report_failure(ctx.id);
                    continue;
                }
            };

            let rctx = RequestContext {
                credentials: &ctx.credentials,
                token: &ctx.token,
                machine_id: &machine_id,
                config,
            };

            let url = endpoint.api_url(&rctx);
            let body = endpoint.transform_api_body(request_body, &rctx);

            let base = self
                .client_for(&ctx.credentials)?
                .post(&url)
                .body(body)
                .header("content-type", "application/json")
                .header("Connection", "close");
            let request = endpoint.decorate_api(base, &rctx);

            let response = match request.send().await {
                Ok(resp) => resp,
                Err(e) => {
                    tracing::warn!(
                        "API 请求发送失败（尝试 {}/{}）: {}",
                        attempt + 1,
                        max_retries,
                        e
                    );
                    // 网络错误通常是上游/链路瞬态问题，不应导致"禁用凭据"或"切换凭据"
                    // （否则一段时间网络抖动会把所有凭据都误禁用，需要重启才能恢复）
                    last_error = Some(e.into());
                    if attempt + 1 < max_retries {
                        sleep(Self::retry_delay(attempt)).await;
                    }
                    continue;
                }
            };

            let status = response.status();

            // 成功响应
            if status.is_success() {
                self.token_manager.report_success(ctx.id);

                // 流式请求：用 first-token timeout 检测上游是否真正开始响应
                if is_stream {
                    let mut body_stream = response.bytes_stream();
                    match tokio::time::timeout(
                        Duration::from_secs(FIRST_TOKEN_TIMEOUT_SECS),
                        body_stream.next(),
                    )
                    .await
                    {
                        Ok(Some(Ok(first_chunk))) => {
                            // 首 chunk 成功到达：拼接首 chunk + 剩余流 → 重建 Response
                            let remaining =
                                futures::stream::once(async { Ok(first_chunk) }).chain(
                                    body_stream.map(|r| r.map_err(|e| {
                                        std::io::Error::new(std::io::ErrorKind::Other, e)
                                    })),
                                );
                            let body = reqwest::Body::wrap_stream(remaining);
                            let rebuilt = http::Response::builder()
                                .status(status)
                                .body(body)
                                .expect("重建流式响应失败");
                            return Ok(reqwest::Response::from(rebuilt));
                        }
                        Ok(Some(Err(e))) => {
                            tracing::warn!(
                                "流式首 chunk 读取失败（尝试 {}/{}）: {}",
                                attempt + 1,
                                max_retries,
                                e
                            );
                            last_error = Some(e.into());
                            if attempt + 1 < max_retries {
                                sleep(Self::retry_delay(attempt)).await;
                            }
                            continue;
                        }
                        Ok(None) => {
                            // 上游立即关闭连接，无数据
                            tracing::warn!(
                                "流式响应无数据（上游立即关闭，尝试 {}/{}）",
                                attempt + 1,
                                max_retries,
                            );
                            last_error = Some(anyhow::anyhow!(
                                "{} API 请求失败: 上游返回空流",
                                api_type,
                            ));
                            if attempt + 1 < max_retries {
                                sleep(Self::retry_delay(attempt)).await;
                            }
                            continue;
                        }
                        Err(_elapsed) => {
                            tracing::warn!(
                                "流式首 chunk 等待超时（{}s，尝试 {}/{}）",
                                FIRST_TOKEN_TIMEOUT_SECS,
                                attempt + 1,
                                max_retries,
                            );
                            last_error = Some(anyhow::anyhow!(
                                "{} API 请求失败: 首 token 超时（{}s）",
                                api_type,
                                FIRST_TOKEN_TIMEOUT_SECS,
                            ));
                            if attempt + 1 < max_retries {
                                sleep(Self::retry_delay(attempt)).await;
                            }
                            continue;
                        }
                    }
                }

                return Ok(response);
            }

            // 失败响应：读取 body 用于日志/错误信息
            let body = response.text().await.unwrap_or_default();

            // 402 Payment Required 且额度用尽：禁用凭据并故障转移（独立于 classify，需 endpoint 判断）
            if status.as_u16() == 402 && endpoint.is_monthly_request_limit(&body) {
                tracing::warn!(
                    "API 请求失败（额度已用尽，禁用凭据并切换，尝试 {}/{}）: {} {}",
                    attempt + 1,
                    max_retries,
                    status,
                    body
                );

                let has_available = self.token_manager.report_quota_exhausted(ctx.id);
                if !has_available {
                    anyhow::bail!(
                        "{} API 请求失败（所有凭据已用尽）: {} {}",
                        api_type,
                        status,
                        body
                    );
                }

                last_error = Some(anyhow::anyhow!(
                    "{} API 请求失败: {} {}",
                    api_type,
                    status,
                    body
                ));
                continue;
            }

            match classify(status, &body) {
                KiroErrorKind::RefreshTokenInvalid => {
                    tracing::warn!(
                        "API 请求失败（可能为凭据错误，尝试 {}/{}）: {} {}",
                        attempt + 1,
                        max_retries,
                        status,
                        body
                    );

                    // token 被上游失效：先尝试 force-refresh，每凭据仅一次机会
                    if endpoint.is_bearer_token_invalid(&body)
                        && !force_refreshed.contains(&ctx.id)
                    {
                        force_refreshed.insert(ctx.id);
                        tracing::info!("凭据 #{} token 疑似被上游失效，尝试强制刷新", ctx.id);
                        if self
                            .token_manager
                            .force_refresh_token_for(ctx.id)
                            .await
                            .is_ok()
                        {
                            tracing::info!("凭据 #{} token 强制刷新成功，重试请求", ctx.id);
                            continue;
                        }
                        tracing::warn!("凭据 #{} token 强制刷新失败，计入失败", ctx.id);
                    }

                    let has_available = self.token_manager.report_failure(ctx.id);
                    if !has_available {
                        anyhow::bail!(
                            "{} API 请求失败（所有凭据已用尽）: {} {}",
                            api_type,
                            status,
                            body
                        );
                    }

                    last_error = Some(anyhow::anyhow!(
                        "{} API 请求失败: {} {}",
                        api_type,
                        status,
                        body
                    ));
                    continue;
                }
                KiroErrorKind::RateLimit => {
                    // 429/408/5xx - 瞬态上游错误：重试但不禁用或切换凭据
                    tracing::warn!(
                        "API 请求失败（上游瞬态错误，尝试 {}/{}）: {} {}",
                        attempt + 1,
                        max_retries,
                        status,
                        body
                    );
                    last_error = Some(anyhow::anyhow!(
                        "{} API 请求失败: {} {}",
                        api_type,
                        status,
                        body
                    ));
                    if attempt + 1 < max_retries {
                        sleep(Self::retry_delay(attempt)).await;
                    }
                    continue;
                }
                KiroErrorKind::Fatal
                | KiroErrorKind::ContentLengthExceeded
                | KiroErrorKind::InvalidModel => {
                    anyhow::bail!("{} API 请求失败: {} {}", api_type, status, body);
                }
                KiroErrorKind::Recoverable => {
                    // 兜底：当作可重试的瞬态错误处理（不切换凭据）
                    tracing::warn!(
                        "API 请求失败（未知错误，尝试 {}/{}）: {} {}",
                        attempt + 1,
                        max_retries,
                        status,
                        body
                    );
                    last_error = Some(anyhow::anyhow!(
                        "{} API 请求失败: {} {}",
                        api_type,
                        status,
                        body
                    ));
                    if attempt + 1 < max_retries {
                        sleep(Self::retry_delay(attempt)).await;
                    }
                }
            }
        }

        // 所有重试都失败
        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!(
                "{} API 请求失败：已达到最大重试次数（{}次）",
                api_type,
                max_retries
            )
        }))
    }

    /// 从请求体中提取模型信息
    ///
    /// 尝试解析 JSON 请求体，提取 conversationState.currentMessage.userInputMessage.modelId
    fn extract_model_from_request(request_body: &str) -> Option<String> {
        use serde_json::Value;

        let json: Value = serde_json::from_str(request_body).ok()?;

        json.get("conversationState")?
            .get("currentMessage")?
            .get("userInputMessage")?
            .get("modelId")?
            .as_str()
            .map(|s| s.to_string())
    }

    fn retry_delay(attempt: usize) -> Duration {
        // 指数退避 + 少量抖动，避免上游抖动时放大故障
        const BASE_MS: u64 = 200;
        const MAX_MS: u64 = 2_000;
        let exp = BASE_MS.saturating_mul(2u64.saturating_pow(attempt.min(6) as u32));
        let backoff = exp.min(MAX_MS);
        let jitter_max = (backoff / 4).max(1);
        let jitter = fastrand::u64(0..=jitter_max);
        Duration::from_millis(backoff.saturating_add(jitter))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use reqwest::StatusCode;

    // ==================== classify() 单测 ====================

    #[test]
    fn classify_401_returns_refresh_token_invalid() {
        assert_eq!(
            classify(StatusCode::UNAUTHORIZED, "any body"),
            KiroErrorKind::RefreshTokenInvalid
        );
    }

    #[test]
    fn classify_403_returns_refresh_token_invalid() {
        assert_eq!(
            classify(StatusCode::FORBIDDEN, "any body"),
            KiroErrorKind::RefreshTokenInvalid
        );
    }

    #[test]
    fn classify_408_returns_rate_limit() {
        assert_eq!(
            classify(StatusCode::REQUEST_TIMEOUT, ""),
            KiroErrorKind::RateLimit
        );
    }

    #[test]
    fn classify_429_returns_rate_limit() {
        assert_eq!(
            classify(StatusCode::TOO_MANY_REQUESTS, ""),
            KiroErrorKind::RateLimit
        );
    }

    #[test]
    fn classify_500_returns_rate_limit() {
        assert_eq!(
            classify(StatusCode::INTERNAL_SERVER_ERROR, ""),
            KiroErrorKind::RateLimit
        );
    }

    #[test]
    fn classify_502_returns_rate_limit() {
        assert_eq!(
            classify(StatusCode::BAD_GATEWAY, ""),
            KiroErrorKind::RateLimit
        );
    }

    #[test]
    fn classify_503_returns_rate_limit() {
        assert_eq!(
            classify(StatusCode::SERVICE_UNAVAILABLE, ""),
            KiroErrorKind::RateLimit
        );
    }

    #[test]
    fn classify_400_content_length_exceeded() {
        assert_eq!(
            classify(
                StatusCode::BAD_REQUEST,
                r#"{"error":"CONTENT_LENGTH_EXCEEDS_THRESHOLD"}"#
            ),
            KiroErrorKind::ContentLengthExceeded
        );
    }

    #[test]
    fn classify_400_invalid_model() {
        assert_eq!(
            classify(
                StatusCode::BAD_REQUEST,
                r#"{"error":"INVALID_MODEL_ID"}"#
            ),
            KiroErrorKind::InvalidModel
        );
    }

    #[test]
    fn classify_400_generic_returns_fatal() {
        assert_eq!(
            classify(StatusCode::BAD_REQUEST, r#"{"error":"bad request"}"#),
            KiroErrorKind::Fatal
        );
    }

    #[test]
    fn classify_422_returns_fatal() {
        assert_eq!(
            classify(StatusCode::UNPROCESSABLE_ENTITY, ""),
            KiroErrorKind::Fatal
        );
    }

    #[test]
    fn classify_404_returns_recoverable() {
        assert_eq!(
            classify(StatusCode::NOT_FOUND, ""),
            KiroErrorKind::Recoverable
        );
    }

    #[test]
    fn classify_200_returns_recoverable() {
        // 虽然不应对成功响应调用 classify，但确认兜底行为
        assert_eq!(classify(StatusCode::OK, ""), KiroErrorKind::Recoverable);
    }

    // ==================== first-token timeout 单测 ====================

    #[tokio::test]
    async fn first_token_timeout_fires_on_pending_stream() {
        // 使用 tokio::time::pause() 快进时间，不真等 15s
        tokio::time::pause();

        let pending_stream = futures::stream::pending::<Result<Bytes, std::io::Error>>();
        futures::pin_mut!(pending_stream);

        let result = tokio::time::timeout(
            Duration::from_secs(FIRST_TOKEN_TIMEOUT_SECS),
            pending_stream.next(),
        )
        .await;

        assert!(result.is_err(), "应在 {}s 后超时", FIRST_TOKEN_TIMEOUT_SECS);
    }

    #[tokio::test]
    async fn first_token_timeout_passes_when_chunk_arrives() {
        tokio::time::pause();

        let chunk = Bytes::from("hello");
        let stream = futures::stream::once(async { Ok::<Bytes, std::io::Error>(chunk.clone()) });
        futures::pin_mut!(stream);

        let result = tokio::time::timeout(
            Duration::from_secs(FIRST_TOKEN_TIMEOUT_SECS),
            stream.next(),
        )
        .await;

        assert!(result.is_ok(), "首 chunk 到达时不应超时");
        let inner = result.unwrap();
        assert!(inner.is_some(), "stream 应有数据");
        assert_eq!(inner.unwrap().unwrap(), chunk);
    }

    #[tokio::test]
    async fn first_token_timeout_triggers_after_exact_duration() {
        tokio::time::pause();

        let pending_stream = futures::stream::pending::<Result<Bytes, std::io::Error>>();
        futures::pin_mut!(pending_stream);

        // 前进到刚好不到超时时间
        let almost = Duration::from_secs(FIRST_TOKEN_TIMEOUT_SECS) - Duration::from_millis(1);
        let timeout_fut = tokio::time::timeout(
            Duration::from_secs(FIRST_TOKEN_TIMEOUT_SECS),
            pending_stream.next(),
        );
        futures::pin_mut!(timeout_fut);

        // 在超时前 poll 一次，不应完成
        tokio::time::advance(almost).await;
        let poll_result = futures::poll!(&mut timeout_fut);
        assert!(
            poll_result.is_pending(),
            "超时前 1ms 不应完成"
        );

        // 再前进 1ms 触发超时
        tokio::time::advance(Duration::from_millis(1)).await;
        let final_result = timeout_fut.await;
        assert!(final_result.is_err(), "精确超时后应返回 Elapsed");
    }
}
