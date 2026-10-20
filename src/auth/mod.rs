pub mod ip;
pub mod key;

use key::VirtualKeyRecord;

/// 注入请求链路上下文的鉴权凭据与配额门限
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub key_id: String,
    pub user_id: String,
    pub key_name: String,
    pub rpm_limit: u32,
    pub max_concurrency: usize,
    pub daily_token_quota: u64,
    pub weekly_token_quota: u64,
    pub allowed_models: Vec<String>,
    pub client_ip: String,
}

impl AuthContext {
    pub fn from_record(record: &VirtualKeyRecord, client_ip: &str) -> Self {
        Self {
            key_id: record.id.clone(),
            user_id: record.user_id.clone(),
            key_name: record.name.clone(),
            rpm_limit: record.rpm_limit,
            max_concurrency: record.max_concurrency,
            daily_token_quota: record.daily_token_quota,
            weekly_token_quota: record.weekly_token_quota,
            allowed_models: record.allowed_models.clone(),
            client_ip: client_ip.to_string(),
        }
    }
}
