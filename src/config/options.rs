use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    #[serde(default = "default_server_host")]
    pub host: String,
    #[serde(default = "default_server_port")]
    pub port: u16,
}

fn default_server_host() -> String {
    "0.0.0.0".to_string()
}

const fn default_server_port() -> u16 {
    8080
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_server_host(),
            port: default_server_port(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_url")]
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_min_connections")]
    pub min_connections: u32,
    #[serde(default = "default_busy_timeout_ms")]
    pub busy_timeout_ms: u64,
}

fn default_db_url() -> String {
    "sqlite://data/rsgate.db?mode=rwc".to_string()
}

const fn default_max_connections() -> u32 {
    32
}

const fn default_min_connections() -> u32 {
    4
}

const fn default_busy_timeout_ms() -> u64 {
    5000
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: default_db_url(),
            max_connections: default_max_connections(),
            min_connections: default_min_connections(),
            busy_timeout_ms: default_busy_timeout_ms(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub default_admin_password: Option<String>,
    #[serde(default = "default_audit_retention_days")]
    pub audit_retention_days: u32,
}

fn default_audit_retention_days() -> u32 {
    30
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            default_admin_password: None,
            audit_retention_days: default_audit_retention_days(),
        }
    }
}
