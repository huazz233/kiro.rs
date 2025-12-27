mod anthropic;
mod debug;
mod kiro;
mod model;
mod test;

use kiro::model::credentials::KiroCredentials;
use model::config::Config;

#[tokio::main]
async fn main() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    // 加载配置
    let config = Config::load_default().unwrap_or_else(|e| {
        tracing::error!("加载配置失败: {}", e);
        std::process::exit(1);
    });

    // 加载凭证
    let credentials = KiroCredentials::load_default().unwrap_or_else(|e| {
        tracing::error!("加载凭证失败: {}", e);
        std::process::exit(1);
    });

    tracing::debug!("凭证已加载: {:?}", credentials);

    // 获取 API Key
    let api_key = config.api_key.clone().unwrap_or_else(|| {
        tracing::error!("配置文件中未设置 apiKey");
        std::process::exit(1);
    });

    // 构建路由
    let app = anthropic::create_router(&api_key);

    // 启动服务器
    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Starting Anthropic API server on {}", addr);
    tracing::info!("API Key: {}***", &api_key[..api_key.len().min(4)]);
    tracing::info!("Endpoints:");
    tracing::info!("  GET  /v1/models");
    tracing::info!("  POST /v1/messages");
    tracing::info!("  POST /v1/messages/count_tokens");

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
