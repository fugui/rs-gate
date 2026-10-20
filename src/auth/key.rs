use super::ip::IpWhitelistMatcher;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::RwLock;
use subtle::ConstantTimeEq;

/// 默认虚拟 Key 前缀
pub const KEY_PREFIX: &str = "sk-gate-";
/// 默认加盐凭据 (若未在配置文件中自定义)
pub const DEFAULT_KEY_SALT: &str = "rs-gate-auth-salt-v1";

/// 计算原始虚拟 Key 的加盐 SHA-256 哈希字符串 (小写十六进制)
pub fn hash_virtual_key(raw_key: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw_key.as_bytes());
    hasher.update(b":");
    hasher.update(salt.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 内存高速虚拟 Key 元数据记录
#[derive(Debug, Clone)]
pub struct VirtualKeyRecord {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub key_hash: String,
    pub key_prefix: String,
    pub allowed_models: Vec<String>,
    pub rpm_limit: u32,
    pub max_concurrency: usize,
    pub daily_token_quota: u64,
    pub weekly_token_quota: u64,
    pub ip_whitelist: Option<IpWhitelistMatcher>,
    pub is_active: bool,
    pub expires_at: Option<i64>,
}

impl VirtualKeyRecord {
    /// 检查是否授权调用指定模型
    pub fn is_model_allowed(&self, model: &str) -> bool {
        self.allowed_models.iter().any(|m| m == "*" || m == model)
    }

    /// 检查来源客户端 IP 是否通过白名单
    pub fn is_ip_allowed(&self, ip: &str) -> bool {
        match &self.ip_whitelist {
            Some(matcher) => matcher.is_allowed(ip),
            None => true,
        }
    }

    /// 检查 Key 是否处于有效生命周期内
    pub fn is_valid_now(&self) -> bool {
        if !self.is_active {
            return false;
        }
        if let Some(exp) = self.expires_at {
            let now = chrono::Utc::now().timestamp_millis();
            if now >= exp {
                return false;
            }
        }
        true
    }
}

/// 虚拟 Key 内存缓存只读映射
#[derive(Debug, Default)]
pub struct KeyStore {
    salt: String,
    // key_hash -> VirtualKeyRecord
    keys: RwLock<HashMap<String, VirtualKeyRecord>>,
}

impl KeyStore {
    pub fn new(salt: impl Into<String>) -> Self {
        Self {
            salt: salt.into(),
            keys: RwLock::new(HashMap::new()),
        }
    }

    /// 注册或替换一个虚拟 Key 记录
    pub fn insert(&self, record: VirtualKeyRecord) {
        let mut map = self.keys.write().unwrap_or_else(|e| e.into_inner());
        map.insert(record.key_hash.clone(), record);
    }

    /// 根据原始入站 Bearer Key 校验并返回对应的身份记录 (恒定时间安全比对)
    pub fn authenticate(&self, raw_key: &str) -> Option<VirtualKeyRecord> {
        if !raw_key.starts_with(KEY_PREFIX) {
            return None;
        }

        let computed_hash = hash_virtual_key(raw_key, &self.salt);
        let map = self.keys.read().unwrap_or_else(|e| e.into_inner());

        // 使用恒定时间比对以防时序侧信道攻击
        for (hash, record) in map.iter() {
            if hash.as_bytes().ct_eq(computed_hash.as_bytes()).into() {
                if record.is_valid_now() {
                    return Some(record.clone());
                } else {
                    return None;
                }
            }
        }

        None
    }

    /// 从 SQLite `virtual_keys` 表加载全量活跃 Key
    pub async fn load_from_db(&self, pool: &sqlx::SqlitePool) -> Result<usize, sqlx::Error> {
        #[derive(sqlx::FromRow)]
        struct DbKeyRow {
            id: String,
            user_id: String,
            name: String,
            key_hash: String,
            key_prefix: String,
            allowed_models: String,
            rpm_limit: i64,
            max_concurrency: i64,
            daily_token_quota: i64,
            weekly_token_quota: i64,
            ip_whitelist: Option<String>,
            is_active: i64,
            expires_at: Option<i64>,
        }

        let rows = sqlx::query_as::<_, DbKeyRow>(
            "SELECT id, user_id, name, key_hash, key_prefix, allowed_models, rpm_limit, max_concurrency, daily_token_quota, weekly_token_quota, ip_whitelist, is_active, expires_at FROM virtual_keys WHERE is_active = 1",
        )
        .fetch_all(pool)
        .await?;

        let mut count = 0;
        let mut map = self.keys.write().unwrap_or_else(|e| e.into_inner());
        map.clear();

        for row in rows {
            let allowed_models: Vec<String> =
                serde_json::from_str(&row.allowed_models).unwrap_or_else(|_| vec!["*".to_string()]);

            let ip_whitelist = row
                .ip_whitelist
                .as_deref()
                .and_then(|wl| IpWhitelistMatcher::parse(wl).ok());

            let record = VirtualKeyRecord {
                id: row.id,
                user_id: row.user_id,
                name: row.name,
                key_hash: row.key_hash.clone(),
                key_prefix: row.key_prefix,
                allowed_models,
                rpm_limit: row.rpm_limit.max(0) as u32,
                max_concurrency: row.max_concurrency.max(0) as usize,
                daily_token_quota: row.daily_token_quota.max(0) as u64,
                weekly_token_quota: row.weekly_token_quota.max(0) as u64,
                ip_whitelist,
                is_active: row.is_active != 0,
                expires_at: row.expires_at,
            };

            map.insert(record.key_hash.clone(), record);
            count += 1;
        }

        Ok(count)
    }
}
