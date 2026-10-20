use crate::admin::admin_api::AuditLogRow;
use crate::admin::auth::{ApiError, AuthUser};
use crate::auth::key::{hash_virtual_key, VirtualKeyRecord, DEFAULT_KEY_SALT};
use crate::server::AppState;
use axum::{
    extract::{Path, Query, State},
    Json,
};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelSnippet {
    pub base_url: String,
    pub python: String,
    pub curl: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserModelItem {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub max_retries: u32,
    pub is_active: bool,
    pub code_snippets: ModelSnippet,
}

#[derive(Debug, sqlx::FromRow)]
struct ModelDbRow {
    id: String,
    display_name: String,
    description: String,
    max_retries: i32,
    is_active: i32,
}

pub async fn list_models(
    State(state): State<AppState>,
    AuthUser(_claims): AuthUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut models = Vec::new();

    if let Some(ref pool) = state.pool {
        let rows: Result<Vec<ModelDbRow>, _> = sqlx::query_as(
            "SELECT id, display_name, description, max_retries, is_active FROM models WHERE is_active = 1 ORDER BY id ASC;"
        )
        .fetch_all(pool)
        .await;

        if let Ok(rows) = rows {
            for r in rows {
                let py_snippet = format!(
                    "from openai import OpenAI\n\nclient = OpenAI(\n    base_url=\"http://localhost:8080/v1\",\n    api_key=\"YOUR_API_KEY\",\n)\nresponse = client.chat.completions.create(\n    model=\"{}\",\n    messages=[{{\"role\": \"user\", \"content\": \"你好\"}}]\n)\nprint(response.choices[0].message.content)",
                    r.id
                );
                let curl_snippet = format!(
                    "curl http://localhost:8080/v1/chat/completions \\\n  -H \"Authorization: Bearer YOUR_API_KEY\" \\\n  -H \"Content-Type: application/json\" \\\n  -d '{{\"model\": \"{}\", \"messages\": [{{\"role\": \"user\", \"content\": \"你好\"}}]}}'",
                    r.id
                );
                models.push(UserModelItem {
                    id: r.id,
                    display_name: r.display_name,
                    description: r.description,
                    capabilities: vec!["chat".to_string(), "responses".to_string()],
                    max_retries: r.max_retries as u32,
                    is_active: r.is_active != 0,
                    code_snippets: ModelSnippet {
                        base_url: "http://localhost:8080/v1".to_string(),
                        python: py_snippet,
                        curl: curl_snippet,
                    },
                });
            }
        }
    }

    if models.is_empty() {
        let id = "deepseek-r1".to_string();
        let py_snippet = format!(
            "from openai import OpenAI\n\nclient = OpenAI(\n    base_url=\"http://localhost:8080/v1\",\n    api_key=\"YOUR_API_KEY\",\n)\nresponse = client.chat.completions.create(\n    model=\"{}\",\n    messages=[{{\"role\": \"user\", \"content\": \"你好\"}}]\n)\nprint(response.choices[0].message.content)",
            id
        );
        let curl_snippet = format!(
            "curl http://localhost:8080/v1/chat/completions \\\n  -H \"Authorization: Bearer YOUR_API_KEY\" \\\n  -H \"Content-Type: application/json\" \\\n  -d '{{\"model\": \"{}\", \"messages\": [{{\"role\": \"user\", \"content\": \"你好\"}}]}}'",
            id
        );
        models.push(UserModelItem {
            id,
            display_name: "DeepSeek R1 (旗舰推理模型)".to_string(),
            description: "企业级高精推理大模型".to_string(),
            capabilities: vec!["chat".to_string(), "responses".to_string()],
            max_retries: 2,
            is_active: true,
            code_snippets: ModelSnippet {
                base_url: "http://localhost:8080/v1".to_string(),
                python: py_snippet,
                curl: curl_snippet,
            },
        });
    }

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": models
    })))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QuotaStatus {
    pub daily_used: i64,
    pub daily_remaining: i64,
    pub daily_percentage: f64,
    pub daily_reset_at: i64,
    pub weekly_used: i64,
    pub weekly_remaining: i64,
    pub weekly_percentage: f64,
    pub weekly_reset_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserKeyItem {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub key_prefix: String,
    pub daily_quota: i64,
    pub weekly_quota: i64,
    pub rpm_limit: u32,
    pub max_concurrency: u32,
    pub model_whitelist: Vec<String>,
    pub is_active: bool,
    pub expires_at: Option<i64>,
    pub created_at: i64,
    pub quota_status: QuotaStatus,
}

#[derive(Debug, sqlx::FromRow)]
struct KeyDbRow {
    id: String,
    user_id: String,
    name: String,
    key_prefix: String,
    allowed_models: String,
    rpm_limit: i32,
    max_concurrency: i32,
    daily_token_quota: i64,
    weekly_token_quota: i64,
    is_active: i32,
    expires_at: Option<i64>,
    created_at: i64,
}

pub async fn list_keys(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let rows: Vec<KeyDbRow> = sqlx::query_as(
        r#"
        SELECT 
            id, user_id, name, key_prefix, allowed_models, rpm_limit, max_concurrency,
            daily_token_quota, weekly_token_quota, is_active, expires_at, created_at
        FROM virtual_keys 
        WHERE user_id = ? 
        ORDER BY created_at DESC;
        "#,
    )
    .bind(&claims.sub)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let current_week = chrono::Utc::now().format("%Y-W%W").to_string();

    let mut items = Vec::new();

    for r in rows {
        let models: Vec<String> =
            serde_json::from_str(&r.allowed_models).unwrap_or_else(|_| vec!["*".to_string()]);

        // Query quota usages for key
        let quota_rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT period_type, total_tokens FROM quota_usages WHERE key_id = ? AND ((period_type = 'daily' AND period_start = ?) OR (period_type = 'weekly' AND period_start = ?));"
        )
        .bind(&r.id)
        .bind(&today)
        .bind(&current_week)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mut daily_used = 0i64;
        let mut weekly_used = 0i64;
        for (pt, tot) in quota_rows {
            if pt == "daily" {
                daily_used = tot;
            } else if pt == "weekly" {
                weekly_used = tot;
            }
        }

        let daily_remaining = if r.daily_token_quota > 0 {
            (r.daily_token_quota - daily_used).max(0)
        } else {
            0
        };
        let daily_pct = if r.daily_token_quota > 0 {
            ((daily_used as f64) / (r.daily_token_quota as f64) * 100.0).min(100.0)
        } else {
            0.0
        };

        let weekly_remaining = if r.weekly_token_quota > 0 {
            (r.weekly_token_quota - weekly_used).max(0)
        } else {
            0
        };
        let weekly_pct = if r.weekly_token_quota > 0 {
            ((weekly_used as f64) / (r.weekly_token_quota as f64) * 100.0).min(100.0)
        } else {
            0.0
        };

        items.push(UserKeyItem {
            id: r.id,
            user_id: r.user_id,
            name: r.name,
            key_prefix: r.key_prefix,
            daily_quota: r.daily_token_quota,
            weekly_quota: r.weekly_token_quota,
            rpm_limit: r.rpm_limit as u32,
            max_concurrency: r.max_concurrency as u32,
            model_whitelist: models,
            is_active: r.is_active != 0,
            expires_at: r.expires_at,
            created_at: r.created_at,
            quota_status: QuotaStatus {
                daily_used,
                daily_remaining,
                daily_percentage: daily_pct,
                daily_reset_at: 0,
                weekly_used,
                weekly_remaining,
                weekly_percentage: weekly_pct,
                weekly_reset_at: 0,
            },
        });
    }

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": {
            "items": items,
            "total": items.len(),
            "has_more": false
        }
    })))
}

#[derive(Debug, Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
    pub model_whitelist: Option<Vec<String>>,
    pub allowed_models: Option<Vec<String>>,
    pub daily_token_quota: Option<i64>,
    pub weekly_token_quota: Option<i64>,
    pub rpm_limit: Option<u32>,
    pub max_concurrency: Option<u32>,
    pub expires_at: Option<i64>,
}

pub async fn create_key(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(payload): Json<CreateKeyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    // Generate random 32 character plaintext key
    let random_bytes: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();
    let plaintext_key = format!("sk-gate-{}", random_bytes);
    let key_prefix = format!("sk-gate-{}***", &random_bytes[..4]);
    let key_hash = hash_virtual_key(&plaintext_key, DEFAULT_KEY_SALT);

    let key_id = format!("key-{}", uuid::Uuid::new_v4());
    let models = payload
        .model_whitelist
        .or(payload.allowed_models)
        .unwrap_or_else(|| vec!["*".to_string()]);
    let models_json = serde_json::to_string(&models).unwrap_or_else(|_| "[\"*\"]".to_string());

    let daily_quota = payload.daily_token_quota.unwrap_or(1_000_000);
    let weekly_quota = payload.weekly_token_quota.unwrap_or(5_000_000);
    let rpm_limit = payload.rpm_limit.unwrap_or(60);
    let max_concurrency = payload.max_concurrency.unwrap_or(5);
    let now_ms = chrono::Utc::now().timestamp_millis();

    sqlx::query(
        r#"
        INSERT INTO virtual_keys (
            id, user_id, name, key_hash, key_prefix, allowed_models,
            rpm_limit, max_concurrency, daily_token_quota, weekly_token_quota,
            is_active, expires_at, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?);
        "#,
    )
    .bind(&key_id)
    .bind(&claims.sub)
    .bind(&payload.name)
    .bind(&key_hash)
    .bind(&key_prefix)
    .bind(&models_json)
    .bind(rpm_limit as i64)
    .bind(max_concurrency as i64)
    .bind(daily_quota)
    .bind(weekly_quota)
    .bind(payload.expires_at)
    .bind(now_ms)
    .bind(now_ms)
    .execute(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    // Update in-memory KeyStore
    if let Some(ref ks) = state.key_store {
        ks.insert(VirtualKeyRecord {
            id: key_id.clone(),
            user_id: claims.sub.clone(),
            name: payload.name.clone(),
            key_hash,
            key_prefix: key_prefix.clone(),
            allowed_models: models.clone(),
            rpm_limit,
            max_concurrency: max_concurrency as usize,
            daily_token_quota: daily_quota.max(0) as u64,
            weekly_token_quota: weekly_quota.max(0) as u64,
            ip_whitelist: None,
            is_active: true,
            expires_at: payload.expires_at,
        });
    }

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": {
            "id": key_id,
            "user_id": claims.sub,
            "name": payload.name,
            "plaintext_key": plaintext_key,
            "key_prefix": key_prefix,
            "daily_quota": daily_quota,
            "weekly_quota": weekly_quota,
            "rpm_limit": rpm_limit,
            "max_concurrency": max_concurrency,
            "model_whitelist": models,
            "created_at": now_ms
        }
    })))
}

#[derive(Debug, Deserialize)]
pub struct UpdateKeyRequest {
    pub name: Option<String>,
    pub model_whitelist: Option<Vec<String>>,
    pub allowed_models: Option<Vec<String>>,
}

pub async fn update_key(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Json(payload): Json<UpdateKeyRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    // Check ownership
    let key: Option<(String, String, String, String)> =
        sqlx::query_as("SELECT id, user_id, name, allowed_models FROM virtual_keys WHERE id = ?;")
            .bind(&id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;

    let (_k_id, user_id, curr_name, curr_models) = match key {
        Some(k) => k,
        None => return Err(ApiError::not_found("Key not found")),
    };

    if user_id != claims.sub && claims.role != "admin" && claims.role != "superadmin" {
        return Err(ApiError::forbidden("Forbidden"));
    }

    let new_name = payload.name.unwrap_or(curr_name);
    let new_models = payload.model_whitelist.or(payload.allowed_models);
    let new_models_json = if let Some(ref m) = new_models {
        serde_json::to_string(m).unwrap_or(curr_models)
    } else {
        curr_models
    };

    let now_ms = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        "UPDATE virtual_keys SET name = ?, allowed_models = ?, updated_at = ? WHERE id = ?;",
    )
    .bind(&new_name)
    .bind(&new_models_json)
    .bind(now_ms)
    .bind(&id)
    .execute(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    // Reload into key store
    if let Some(ref ks) = state.key_store {
        let _ = ks.load_from_db(pool).await;
    }

    let parsed_models: Vec<String> = serde_json::from_str(&new_models_json).unwrap_or_default();

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": {
            "id": id,
            "name": new_name,
            "model_whitelist": parsed_models
        }
    })))
}

pub async fn delete_key(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let key: Option<(String, String)> =
        sqlx::query_as("SELECT id, user_id FROM virtual_keys WHERE id = ?;")
            .bind(&id)
            .fetch_optional(pool)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;

    let (_k_id, user_id) = match key {
        Some(k) => k,
        None => return Err(ApiError::not_found("Key not found")),
    };

    if user_id != claims.sub && claims.role != "admin" && claims.role != "superadmin" {
        return Err(ApiError::forbidden("Forbidden"));
    }

    sqlx::query("DELETE FROM virtual_keys WHERE id = ?;")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    if let Some(ref ks) = state.key_store {
        let _ = ks.load_from_db(pool).await;
    }

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "Key revoked successfully",
        "data": null
    })))
}

#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    pub key_id: Option<String>,
    pub model: Option<String>,
    pub status: Option<i32>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

pub async fn list_user_audits(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(params): Query<AuditQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let rows: Vec<AuditLogRow> = sqlx::query_as(
        r#"
        SELECT 
            id, trace_id, key_id, user_id, model, backend_id, protocol,
            client_ip, http_status, prompt_tokens, completion_tokens, total_tokens,
            request_duration_ms, ttft_ms, is_stream, retry_count, failover_path,
            error_message, created_at
        FROM audit_logs
        WHERE user_id = ?
        ORDER BY created_at DESC
        LIMIT ? OFFSET ?;
        "#,
    )
    .bind(&claims.sub)
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_logs WHERE user_id = ?;")
        .bind(&claims.sub)
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

    let items: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "trace_id": r.trace_id,
                "key_id": r.key_id,
                "user_id": r.user_id,
                "model": r.model,
                "backend_id": r.backend_id,
                "protocol": r.protocol,
                "client_ip": r.client_ip,
                "http_status": r.http_status,
                "prompt_tokens": r.prompt_tokens,
                "completion_tokens": r.completion_tokens,
                "total_tokens": r.total_tokens,
                "request_duration_ms": r.request_duration_ms,
                "ttft_ms": r.ttft_ms,
                "is_stream": r.is_stream,
                "retry_count": r.retry_count,
                "failover_path": r.failover_path,
                "error_message": r.error_message,
                "created_at": r.created_at,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": {
            "items": items,
            "total": total.0,
            "page": page,
            "page_size": page_size,
            "has_more": (offset + (items.len() as u32)) < (total.0 as u32)
        }
    })))
}
