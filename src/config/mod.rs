pub mod options;

pub use options::{AppConfig, DatabaseConfig, SecurityConfig, ServerConfig};

use std::path::Path;

/// 加载系统配置，支持多源优先级：
/// 1. 环境变量 (RSGATE_*，双下划线表示嵌套层级，如 RSGATE_SERVER__PORT)
/// 2. 配置文件 (指定路径或当前目录 config.yaml)
/// 3. 代码硬编码默认值
pub fn load_config(custom_path: Option<&str>) -> anyhow::Result<AppConfig> {
    let mut builder = config::Config::builder()
        .set_default("server.host", "0.0.0.0")?
        .set_default("server.port", 8080)?
        .set_default("database.url", "sqlite://data/rsgate.db?mode=rwc")?
        .set_default("database.max_connections", 32)?
        .set_default("database.min_connections", 4)?
        .set_default("database.busy_timeout_ms", 5000)?
        .set_default("security.audit_retention_days", 30)?;

    if let Some(path) = custom_path {
        builder = builder.add_source(config::File::with_name(path).required(true));
    } else if Path::new("config.yaml").exists() {
        builder = builder.add_source(config::File::with_name("config.yaml").required(false));
    } else if Path::new("config.toml").exists() {
        builder = builder.add_source(config::File::with_name("config.toml").required(false));
    }

    builder = builder.add_source(
        config::Environment::with_prefix("RSGATE")
            .prefix_separator("_")
            .separator("__"),
    );

    let built = builder.build()?;
    let mut app_config: AppConfig = built.try_deserialize()?;

    // 兼顾 RSGATE_ADMIN_INITIAL_PASSWORD 环境变量直通
    if app_config.security.default_admin_password.is_none() {
        if let Ok(admin_pass) = std::env::var("RSGATE_ADMIN_INITIAL_PASSWORD") {
            if !admin_pass.is_empty() {
                app_config.security.default_admin_password = Some(admin_pass);
            }
        }
    }

    Ok(app_config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_default_config_loading() {
        let _guard = TEST_LOCK.lock().unwrap();
        std::env::remove_var("RSGATE_SERVER__PORT");
        std::env::remove_var("RSGATE_DATABASE__MAX_CONNECTIONS");

        let config = load_config(None).unwrap();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.database.max_connections, 32);
        assert_eq!(config.database.min_connections, 4);
        assert_eq!(config.database.busy_timeout_ms, 5000);
    }

    #[test]
    fn test_env_var_override() {
        let _guard = TEST_LOCK.lock().unwrap();
        std::env::set_var("RSGATE_SERVER__PORT", "9090");
        std::env::set_var("RSGATE_DATABASE__MAX_CONNECTIONS", "64");

        let config = load_config(None).unwrap();
        assert_eq!(config.server.port, 9090);
        assert_eq!(config.database.max_connections, 64);

        std::env::remove_var("RSGATE_SERVER__PORT");
        std::env::remove_var("RSGATE_DATABASE__MAX_CONNECTIONS");
    }
}
