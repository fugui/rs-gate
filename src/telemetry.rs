use tracing_subscriber::{util::SubscriberInitExt, EnvFilter};

/// 初始化结构化追踪日志系统
/// - `format`: "json" 时输出 JSON 格式日志（生产环境），否则输出紧凑控制台日志（开发环境）
/// - `default_level`: 当 `RUST_LOG` 未指定时的默认日志过滤级别（如 "info"）
pub fn init_telemetry(format: &str, default_level: &str) -> anyhow::Result<()> {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    if format.eq_ignore_ascii_case("json") {
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .json()
            .finish();
        let _ = subscriber.try_init();
    } else {
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .compact()
            .finish();
        let _ = subscriber.try_init();
    }

    Ok(())
}
