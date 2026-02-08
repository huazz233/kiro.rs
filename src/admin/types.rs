//! Admin API 类型定义

use serde::{Deserialize, Serialize};

// ============ 凭据状态 ============

/// 所有凭据状态响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialsStatusResponse {
    /// 凭据总数
    pub total: usize,
    /// 可用凭据数量（未禁用）
    pub available: usize,
    /// 各凭据状态列表
    pub credentials: Vec<CredentialStatusItem>,
}

/// 单个凭据的状态信息
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialStatusItem {
    /// 凭据唯一 ID
    pub id: u64,
    /// 优先级（数字越小优先级越高）
    pub priority: u32,
    /// 是否被禁用
    pub disabled: bool,
    /// 连续失败次数
    pub failure_count: u32,
    /// Token 过期时间（RFC3339 格式）
    pub expires_at: Option<String>,
    /// 认证方式
    pub auth_method: Option<String>,
    /// 是否有 Profile ARN
    pub has_profile_arn: bool,
    /// refreshToken 的 SHA-256 哈希（用于前端重复检测）
    pub refresh_token_hash: Option<String>,
    /// 用户邮箱（用于前端显示）
    pub email: Option<String>,
    /// API 调用成功次数
    pub success_count: u64,
    /// 最后一次 API 调用时间（RFC3339 格式）
    pub last_used_at: Option<String>,
}

// ============ 操作请求 ============

/// 启用/禁用凭据请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetDisabledRequest {
    /// 是否禁用
    pub disabled: bool,
}

/// 修改优先级请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPriorityRequest {
    /// 新优先级值
    pub priority: u32,
}

/// 添加凭据请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCredentialRequest {
    /// 刷新令牌（必填）
    pub refresh_token: String,

    /// 认证方式（可选，默认 social）
    #[serde(default = "default_auth_method")]
    pub auth_method: String,

    /// OIDC Client ID（IdC 认证需要）
    pub client_id: Option<String>,

    /// OIDC Client Secret（IdC 认证需要）
    pub client_secret: Option<String>,

    /// 优先级（可选，默认 0）
    #[serde(default)]
    pub priority: u32,

    /// 凭据级 Region 配置（用于 OIDC token 刷新）
    /// 未配置时回退到 config.json 的全局 region
    pub region: Option<String>,

    /// 凭据级 Machine ID（可选，64 位字符串）
    /// 未配置时回退到 config.json 的 machineId
    pub machine_id: Option<String>,

    /// 用户邮箱（可选，用于前端显示）
    pub email: Option<String>,
}

fn default_auth_method() -> String {
    "social".to_string()
}

/// 添加凭据成功响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddCredentialResponse {
    pub success: bool,
    pub message: String,
    /// 新添加的凭据 ID
    pub credential_id: u64,
    /// 用户邮箱（如果获取成功）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

// ============ 余额查询 ============

/// 余额查询响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceResponse {
    /// 凭据 ID
    pub id: u64,
    /// 订阅类型
    pub subscription_title: Option<String>,
    /// 当前使用量
    pub current_usage: f64,
    /// 使用限额
    pub usage_limit: f64,
    /// 剩余额度
    pub remaining: f64,
    /// 使用百分比
    pub usage_percentage: f64,
    /// 下次重置时间（Unix 时间戳）
    pub next_reset_at: Option<f64>,
}

/// 缓存余额信息
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedBalanceItem {
    /// 凭据 ID
    pub id: u64,
    /// 缓存的剩余额度
    pub remaining: f64,
    /// 缓存时间（Unix 毫秒时间戳）
    pub cached_at: u64,
    /// 缓存存活时间（秒），缓存过期时间 = cached_at + ttl_secs * 1000
    pub ttl_secs: u64,
}

/// 所有凭据的缓存余额响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedBalancesResponse {
    /// 各凭据的缓存余额列表
    pub balances: Vec<CachedBalanceItem>,
}

// ============ 负载均衡配置 ============

/// 负载均衡模式响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadBalancingModeResponse {
    /// 当前模式（"priority" 或 "balanced"）
    pub mode: String,
}

/// 设置负载均衡模式请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetLoadBalancingModeRequest {
    /// 模式（"priority" 或 "balanced"）
    pub mode: String,
}

// ============ 通用响应 ============

/// 操作成功响应
#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
}

impl SuccessResponse {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
        }
    }
}

// ============ 批量导入 token.json ============

/// 官方 token.json 格式（用于解析导入）
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenJsonItem {
    pub provider: Option<String>,
    pub refresh_token: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub auth_method: Option<String>,
    #[serde(default)]
    pub priority: u32,
    pub region: Option<String>,
    pub machine_id: Option<String>,
}

/// 批量导入请求
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportTokenJsonRequest {
    #[serde(default = "default_dry_run")]
    pub dry_run: bool,
    pub items: ImportItems,
}

fn default_dry_run() -> bool {
    true
}

/// 导入项（支持单个或数组）
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ImportItems {
    Single(TokenJsonItem),
    Multiple(Vec<TokenJsonItem>),
}

impl ImportItems {
    pub fn into_vec(self) -> Vec<TokenJsonItem> {
        match self {
            ImportItems::Single(item) => vec![item],
            ImportItems::Multiple(items) => items,
        }
    }
}

/// 批量导入响应
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportTokenJsonResponse {
    pub summary: ImportSummary,
    pub items: Vec<ImportItemResult>,
}

/// 导入汇总
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub parsed: usize,
    pub added: usize,
    pub skipped: usize,
    pub invalid: usize,
}

/// 单项导入结果
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportItemResult {
    pub index: usize,
    pub fingerprint: String,
    pub action: ImportAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credential_id: Option<u64>,
}

/// 导入动作
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImportAction {
    Added,
    Skipped,
    Invalid,
}

/// 错误响应
#[derive(Debug, Serialize)]
pub struct AdminErrorResponse {
    pub error: AdminError,
}

#[derive(Debug, Serialize)]
pub struct AdminError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

impl AdminErrorResponse {
    pub fn new(error_type: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: AdminError {
                error_type: error_type.into(),
                message: message.into(),
            },
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new("invalid_request", message)
    }

    pub fn authentication_error() -> Self {
        Self::new("authentication_error", "Invalid or missing admin API key")
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("not_found", message)
    }

    pub fn api_error(message: impl Into<String>) -> Self {
        Self::new("api_error", message)
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::new("internal_error", message)
    }
}
