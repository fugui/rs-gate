use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::Rng;
use sqlx::SqlitePool;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::FromRow)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
    pub role: String,
    pub password_hash: String,
}

/// 首次冷启动时自动探查并安全注入初始超级管理员 (admin) 账号
pub async fn ensure_superadmin(
    pool: &SqlitePool,
    preset_password: Option<&str>,
) -> anyhow::Result<UserInfo> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users;")
        .fetch_one(pool)
        .await?;

    if count.0 == 0 {
        let (raw_password, is_generated) = match preset_password {
            Some(pass) if !pass.trim().is_empty() => (pass.trim().to_string(), false),
            _ => {
                const CHARSET: &[u8] =
                    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*";
                let mut rng = rand::thread_rng();
                let gen: String = (0..16)
                    .map(|_| {
                        let idx = rng.gen_range(0..CHARSET.len());
                        CHARSET[idx] as char
                    })
                    .collect();
                (gen, true)
            }
        };

        if is_generated {
            tracing::warn!("============================================================");
            tracing::warn!(
                "INITIAL SUPERADMIN CREATED: username=admin password={}",
                raw_password
            );
            tracing::warn!("PLEASE CHANGE THIS PASSWORD IMMEDIATELY UPON FIRST LOGIN!");
            tracing::warn!("============================================================");
        }

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(raw_password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("Argon2 hash error: {}", e))?
            .to_string();

        let now_ms = chrono::Utc::now().timestamp_millis();
        let user_id = "usr-superadmin-default".to_string();

        sqlx::query(
            r#"
            INSERT INTO users (
                id, username, password_hash, role, display_name, email, is_active, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?);
            "#,
        )
        .bind(&user_id)
        .bind("admin")
        .bind(&password_hash)
        .bind("superadmin")
        .bind("System SuperAdmin")
        .bind("admin@internal.corp")
        .bind(now_ms)
        .bind(now_ms)
        .execute(pool)
        .await?;

        Ok(UserInfo {
            id: user_id,
            username: "admin".to_string(),
            role: "superadmin".to_string(),
            password_hash,
        })
    } else {
        if let Some(user) = sqlx::query_as::<_, UserInfo>(
            "SELECT id, username, role, password_hash FROM users WHERE username = 'admin' LIMIT 1;",
        )
        .fetch_optional(pool)
        .await?
        {
            return Ok(user);
        }

        let user = sqlx::query_as::<_, UserInfo>(
            "SELECT id, username, role, password_hash FROM users WHERE role = 'superadmin' LIMIT 1;",
        )
        .fetch_one(pool)
        .await?;

        Ok(user)
    }
}

/// 基于 Argon2id 验证密码明文与持久化哈希是否匹配
pub fn verify_password(password: &str, password_hash: &str) -> anyhow::Result<bool> {
    let parsed_hash = PasswordHash::new(password_hash)
        .map_err(|e| anyhow::anyhow!("Invalid password hash: {}", e))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}
