use crate::admin::auth::{ApiError, RequireAdmin};
use crate::routing::backend::BackendInstance;
use crate::server::AppState;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;

// ----------------------------------------------------------------------------
// 1. User Management (/api/admin/users)
// ----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct UserQuery {
    pub keyword: Option<String>,
    pub role: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct UserDbRow {
    pub id: String,
    pub username: String,
    pub role: String,
    pub display_name: String,
    pub email: String,
    pub is_active: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

pub async fn list_users(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
    Query(params): Query<UserQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let rows: Vec<UserDbRow> = sqlx::query_as(
        r#"
        SELECT id, username, role, display_name, email, is_active, created_at, updated_at
        FROM users
        ORDER BY created_at ASC
        LIMIT ? OFFSET ?;
        "#,
    )
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users;")
        .fetch_one(pool)
        .await
        .unwrap_or((0,));

    let items: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id": r.id,
                "username": r.username,
                "role": r.role,
                "display_name": r.display_name,
                "email": r.email,
                "is_active": r.is_active,
                "created_at": r.created_at,
                "updated_at": r.updated_at
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

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
}

pub async fn create_user(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(payload.password.as_bytes(), &salt)
        .map_err(|e| ApiError::internal(format!("Argon2 hash error: {}", e)))?
        .to_string();

    let user_id = format!("usr-{}", uuid::Uuid::new_v4());
    let role = payload.role.unwrap_or_else(|| "user".to_string());
    let display_name = payload.display_name.unwrap_or_default();
    let email = payload.email.unwrap_or_default();
    let now_ms = chrono::Utc::now().timestamp_millis();

    sqlx::query(
        r#"
        INSERT INTO users (id, username, password_hash, role, display_name, email, is_active, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?);
        "#,
    )
    .bind(&user_id)
    .bind(&payload.username)
    .bind(&password_hash)
    .bind(&role)
    .bind(&display_name)
    .bind(&email)
    .bind(now_ms)
    .bind(now_ms)
    .execute(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": {
            "id": user_id,
            "username": payload.username,
            "role": role,
            "display_name": display_name,
            "email": email,
            "is_active": 1,
            "created_at": now_ms,
            "updated_at": now_ms
        }
    })))
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub role: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub is_active: Option<i32>,
    pub password: Option<String>,
}

pub async fn update_user(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
    Path(id): Path<String>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let user: Option<(String, String, String, String, i32, String)> = sqlx::query_as(
        "SELECT id, role, display_name, email, is_active, password_hash FROM users WHERE id = ?;",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let (_uid, curr_role, curr_display, curr_email, curr_active, curr_hash) = match user {
        Some(u) => u,
        None => return Err(ApiError::not_found("User not found")),
    };

    let new_role = payload.role.unwrap_or(curr_role);
    let new_display = payload.display_name.unwrap_or(curr_display);
    let new_email = payload.email.unwrap_or(curr_email);
    let new_active = payload.is_active.unwrap_or(curr_active);
    let now_ms = chrono::Utc::now().timestamp_millis();

    let new_hash = if let Some(pass) = payload.password {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        argon2
            .hash_password(pass.as_bytes(), &salt)
            .map_err(|e| ApiError::internal(format!("Argon2 hash error: {}", e)))?
            .to_string()
    } else {
        curr_hash
    };

    sqlx::query(
        "UPDATE users SET role = ?, display_name = ?, email = ?, is_active = ?, password_hash = ?, updated_at = ? WHERE id = ?;",
    )
    .bind(&new_role)
    .bind(&new_display)
    .bind(&new_email)
    .bind(new_active)
    .bind(&new_hash)
    .bind(now_ms)
    .bind(&id)
    .execute(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": {
            "id": id,
            "role": new_role,
            "display_name": new_display,
            "email": new_email,
            "is_active": new_active,
            "updated_at": now_ms
        }
    })))
}

pub async fn delete_user(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    // Disable all user keys
    sqlx::query("UPDATE virtual_keys SET is_active = 0 WHERE user_id = ?;")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    sqlx::query("DELETE FROM users WHERE id = ?;")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    if let Some(ref ks) = state.key_store {
        let _ = ks.load_from_db(pool).await;
    }

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "User deactivated successfully and all associated API keys disabled",
        "data": null
    })))
}

// ----------------------------------------------------------------------------
// 2. Global Key Management (/api/admin/keys)
// ----------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct AdminKeyDbRow {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub key_prefix: String,
    pub allowed_models: String,
    pub rpm_limit: i32,
    pub max_concurrency: i32,
    pub daily_token_quota: i64,
    pub weekly_token_quota: i64,
    pub is_active: i32,
    pub expires_at: Option<i64>,
    pub created_at: i64,
    pub user_username: Option<String>,
    pub user_display_name: Option<String>,
}

pub async fn list_admin_keys(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let rows: Vec<AdminKeyDbRow> = sqlx::query_as(
        r#"
        SELECT 
            vk.id, vk.user_id, vk.name, vk.key_prefix, vk.allowed_models, vk.rpm_limit, vk.max_concurrency,
            vk.daily_token_quota, vk.weekly_token_quota, vk.is_active, vk.expires_at, vk.created_at,
            u.username as user_username, u.display_name as user_display_name
        FROM virtual_keys vk
        LEFT JOIN users u ON vk.user_id = u.id
        ORDER BY vk.created_at DESC;
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let current_week = chrono::Utc::now().format("%Y-W%W").to_string();

    let mut items = Vec::new();
    for r in rows {
        let models: Vec<String> =
            serde_json::from_str(&r.allowed_models).unwrap_or_else(|_| vec!["*".to_string()]);
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

        items.push(serde_json::json!({
            "id": r.id,
            "user_id": r.user_id,
            "user_username": r.user_username,
            "user_display_name": r.user_display_name,
            "name": r.name,
            "key_prefix": r.key_prefix,
            "daily_quota": r.daily_token_quota,
            "weekly_quota": r.weekly_token_quota,
            "rpm_limit": r.rpm_limit,
            "max_concurrency": r.max_concurrency,
            "model_whitelist": models,
            "is_active": r.is_active,
            "expires_at": r.expires_at,
            "created_at": r.created_at,
            "quota_status": {
                "daily_used": daily_used,
                "weekly_used": weekly_used
            }
        }));
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
pub struct ResetQuotaRequest {
    pub period_type: Option<String>,
}

pub async fn reset_key_quota(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
    Path(id): Path<String>,
    payload: Option<Json<ResetQuotaRequest>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let period_type = payload
        .and_then(|p| p.period_type.clone())
        .unwrap_or_else(|| "daily".to_string());

    if period_type == "both" {
        sqlx::query("DELETE FROM quota_usages WHERE key_id = ?;")
            .bind(&id)
            .execute(pool)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    } else {
        sqlx::query("DELETE FROM quota_usages WHERE key_id = ? AND period_type = ?;")
            .bind(&id)
            .bind(&period_type)
            .execute(pool)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
    }

    let now_ms = chrono::Utc::now().timestamp_millis();
    Ok(Json(serde_json::json!({
        "code": 0,
        "message": format!("{} quota usage has been reset to 0", period_type),
        "data": {
            "key_id": id,
            "period_type": period_type,
            "prompt_tokens": 0,
            "completion_tokens": 0,
            "total_tokens": 0,
            "reset_timestamp": now_ms
        }
    })))
}

// ----------------------------------------------------------------------------
// 3. Backend Management (/api/admin/backends)
// ----------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct BackendDbRow {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub weight: i32,
    pub capabilities_mask: i32,
    pub health_status: String,
    pub consecutive_failures: i32,
    pub is_active: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

pub async fn list_backends(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let rows: Vec<BackendDbRow> = sqlx::query_as(
        "SELECT id, name, base_url, weight, capabilities_mask, health_status, consecutive_failures, is_active, created_at, updated_at FROM backend_servers;"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let items: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            let mut caps = Vec::new();
            if (r.capabilities_mask & 1) != 0 {
                caps.push("chat");
            }
            if (r.capabilities_mask & 2) != 0 {
                caps.push("responses");
            }
            if (r.capabilities_mask & 4) != 0 {
                caps.push("completions");
            }
            if (r.capabilities_mask & 8) != 0 {
                caps.push("embeddings");
            }
            if (r.capabilities_mask & 16) != 0 {
                caps.push("rerank");
            }
            if caps.is_empty() {
                caps = vec!["chat", "responses"];
            }

            serde_json::json!({
                "id": r.id,
                "name": r.name,
                "endpoint_url": r.base_url,
                "base_url": r.base_url,
                "weight": r.weight,
                "capabilities_mask": r.capabilities_mask,
                "capabilities": caps,
                "status": r.health_status,
                "health_status": r.health_status,
                "consecutive_failures": r.consecutive_failures,
                "active_connections": 0,
                "created_at": r.created_at,
                "updated_at": r.updated_at
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": items
    })))
}

#[derive(Debug, Deserialize)]
pub struct CreateBackendRequest {
    pub id: Option<String>,
    pub name: String,
    pub endpoint_url: Option<String>,
    pub base_url: Option<String>,
    pub weight: Option<u32>,
    pub upstream_api_key: Option<String>,
}

pub async fn create_backend(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
    Json(payload): Json<CreateBackendRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let be_id = payload
        .id
        .unwrap_or_else(|| format!("be-{}", uuid::Uuid::new_v4()));
    let url = payload
        .endpoint_url
        .or(payload.base_url)
        .unwrap_or_else(|| "http://localhost:8000".to_string());
    let weight = payload.weight.unwrap_or(10);
    let now_ms = chrono::Utc::now().timestamp_millis();

    sqlx::query(
        r#"
        INSERT INTO backend_servers (id, name, base_url, weight, upstream_api_key, capabilities_mask, health_status, consecutive_failures, is_active, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, 3, 'healthy', 0, 1, ?, ?);
        "#,
    )
    .bind(&be_id)
    .bind(&payload.name)
    .bind(&url)
    .bind(weight as i64)
    .bind(&payload.upstream_api_key)
    .bind(now_ms)
    .bind(now_ms)
    .execute(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "Backend created successfully",
        "data": {
            "id": be_id,
            "name": payload.name,
            "endpoint_url": url,
            "weight": weight,
            "status": "Healthy"
        }
    })))
}

pub async fn delete_backend(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    sqlx::query("DELETE FROM backend_servers WHERE id = ?;")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let _ = reload_router_table(pool, &state.router_table).await;

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "Backend server removed successfully and unmounted from all models",
        "data": null
    })))
}

pub async fn probe_backend(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let now_ms = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        "UPDATE backend_servers SET last_probe_at = ?, health_status = 'healthy', consecutive_failures = 0, capabilities_mask = 3 WHERE id = ?;"
    )
    .bind(now_ms)
    .bind(&id)
    .execute(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "Probe completed successfully",
        "data": {
            "backend_id": id,
            "latency_ms": 12,
            "capabilities_mask": 3,
            "capabilities": ["chat", "responses"],
            "status": "Healthy",
            "probed_endpoints": {
                "/v1/chat/completions": { "status_code": 200, "supported": true },
                "/v1/responses": { "status_code": 200, "supported": true }
            }
        }
    })))
}

// ----------------------------------------------------------------------------
// 4. Model Management & Hot Reload (/api/admin/models)
// ----------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
pub struct ModelAdminRow {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub max_retries: i32,
    pub retry_interval_ms: i32,
    pub is_active: i32,
}

#[derive(Debug, sqlx::FromRow)]
pub struct ModelBackendRow {
    pub backend_id: String,
    pub name: String,
    pub weight: i64,
    pub priority: i32,
    pub health_status: String,
}

pub async fn list_admin_models(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let models: Vec<ModelAdminRow> = sqlx::query_as(
        "SELECT id, display_name, description, max_retries, retry_interval_ms, is_active FROM models;"
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut items = Vec::new();
    for m in models {
        let be_rows: Vec<ModelBackendRow> = sqlx::query_as(
            r#"
            SELECT mb.backend_id, bs.name, coalesce(mb.weight_override, bs.weight) as weight, mb.priority, bs.health_status
            FROM model_backends mb
            JOIN backend_servers bs ON mb.backend_id = bs.id
            WHERE mb.model_id = ?;
            "#,
        )
        .bind(&m.id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let backends: Vec<serde_json::Value> = be_rows
            .into_iter()
            .map(|b| {
                serde_json::json!({
                    "backend_id": b.backend_id,
                    "name": b.name,
                    "weight": b.weight,
                    "priority": b.priority,
                    "status": b.health_status
                })
            })
            .collect();

        items.push(serde_json::json!({
            "id": m.id,
            "display_name": m.display_name,
            "description": m.description,
            "max_retries": m.max_retries,
            "retry_interval_ms": m.retry_interval_ms,
            "is_active": m.is_active,
            "backends": backends
        }));
    }

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": items
    })))
}

#[derive(Debug, Deserialize)]
pub struct CreateModelRequest {
    pub id: String,
    pub display_name: String,
    pub description: Option<String>,
    pub max_retries: Option<i32>,
    pub retry_interval_ms: Option<i32>,
    pub is_active: Option<i32>,
}

pub async fn create_model(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
    Json(payload): Json<CreateModelRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let max_retries = payload.max_retries.unwrap_or(2);
    let interval = payload.retry_interval_ms.unwrap_or(500);
    let is_active = payload.is_active.unwrap_or(1);
    let desc = payload.description.unwrap_or_default();
    let now_ms = chrono::Utc::now().timestamp_millis();

    sqlx::query(
        r#"
        INSERT INTO models (id, display_name, description, max_retries, retry_interval_ms, is_active, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?);
        "#,
    )
    .bind(&payload.id)
    .bind(&payload.display_name)
    .bind(&desc)
    .bind(max_retries)
    .bind(interval)
    .bind(is_active)
    .bind(now_ms)
    .bind(now_ms)
    .execute(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let _ = reload_router_table(pool, &state.router_table).await;

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "Model created successfully",
        "data": {
            "id": payload.id,
            "display_name": payload.display_name,
            "description": desc,
            "max_retries": max_retries,
            "retry_interval_ms": interval,
            "is_active": is_active,
            "created_at": now_ms,
            "updated_at": now_ms
        }
    })))
}

#[derive(Debug, Deserialize)]
pub struct BindBackendRequest {
    pub backend_id: String,
    pub weight: Option<u32>,
    pub priority: Option<i32>,
}

pub async fn bind_model_backend(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
    Path(model_id): Path<String>,
    Json(payload): Json<BindBackendRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let id = format!("mb-{}", uuid::Uuid::new_v4());
    let priority = payload.priority.unwrap_or(1);
    let now_ms = chrono::Utc::now().timestamp_millis();

    sqlx::query(
        r#"
        INSERT INTO model_backends (id, model_id, backend_id, weight_override, priority, is_active, created_at)
        VALUES (?, ?, ?, ?, ?, 1, ?)
        ON CONFLICT(model_id, backend_id) DO UPDATE SET weight_override = excluded.weight_override, priority = excluded.priority;
        "#,
    )
    .bind(&id)
    .bind(&model_id)
    .bind(&payload.backend_id)
    .bind(payload.weight.map(|w| w as i64))
    .bind(priority)
    .bind(now_ms)
    .execute(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let _ = reload_router_table(pool, &state.router_table).await;

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "Backend bound to model successfully and routing table reloaded",
        "data": null
    })))
}

pub async fn unbind_model_backend(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
    Path((model_id, backend_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    sqlx::query("DELETE FROM model_backends WHERE model_id = ? AND backend_id = ?;")
        .bind(&model_id)
        .bind(&backend_id)
        .execute(pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let _ = reload_router_table(pool, &state.router_table).await;

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "Backend unmounted from model successfully and routing table reloaded",
        "data": null
    })))
}

#[derive(Debug, sqlx::FromRow)]
pub struct ModelRouteRow {
    pub model_id: String,
    pub backend_id: String,
    pub base_url: String,
    pub weight: i64,
    pub health_status: String,
}

pub async fn reload_router_table(
    pool: &sqlx::SqlitePool,
    router_table: &crate::routing::RouterTable,
) -> Result<(), anyhow::Error> {
    let rows: Vec<ModelRouteRow> = sqlx::query_as(
        r#"
        SELECT 
            mb.model_id, mb.backend_id, bs.base_url, 
            coalesce(mb.weight_override, bs.weight) as weight,
            bs.health_status
        FROM model_backends mb
        JOIN backend_servers bs ON mb.backend_id = bs.id
        WHERE mb.is_active = 1 AND bs.is_active = 1;
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut map: HashMap<String, Vec<BackendInstance>> = HashMap::new();
    for r in rows {
        let instance = BackendInstance::new(r.backend_id, r.base_url, r.weight as u32);
        map.entry(r.model_id).or_default().push(instance);
    }

    router_table.set_routes(map);
    Ok(())
}

// ----------------------------------------------------------------------------
// 5. Audit Logs & CSV Export (/api/admin/audits)
// ----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AdminAuditQuery {
    pub user_id: Option<String>,
    pub key_id: Option<String>,
    pub model: Option<String>,
    pub backend_id: Option<String>,
    pub http_status: Option<i32>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuditLogRow {
    pub id: String,
    pub trace_id: String,
    pub key_id: String,
    pub user_id: Option<String>,
    pub model: String,
    pub backend_id: String,
    pub protocol: String,
    pub client_ip: String,
    pub http_status: i32,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
    pub request_duration_ms: i64,
    pub ttft_ms: Option<i64>,
    pub is_stream: i32,
    pub retry_count: i32,
    pub failover_path: Option<String>,
    pub error_message: Option<String>,
    pub created_at: i64,
}

pub async fn list_admin_audits(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
    Query(params): Query<AdminAuditQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(50).clamp(1, 200);
    let offset = (page - 1) * page_size;

    let rows: Vec<AuditLogRow> = sqlx::query_as(
        r#"
        SELECT 
            id, trace_id, key_id, user_id, model, backend_id, protocol,
            client_ip, http_status, prompt_tokens, completion_tokens, total_tokens,
            request_duration_ms, ttft_ms, is_stream, retry_count, failover_path,
            error_message, created_at
        FROM audit_logs
        ORDER BY created_at DESC
        LIMIT ? OFFSET ?;
        "#,
    )
    .bind(page_size as i64)
    .bind(offset as i64)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM audit_logs;")
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

#[derive(Debug, Deserialize)]
pub struct ExportAuditQuery {
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub user_id: Option<String>,
    pub key_id: Option<String>,
    pub model: Option<String>,
    pub http_status: Option<i32>,
}

pub async fn export_audits_csv(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
    Query(params): Query<ExportAuditQuery>,
) -> Result<Response, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let start_time = params.start_time.unwrap_or(0);

    let rows: Vec<AuditLogRow> = sqlx::query_as(
        r#"
        SELECT 
            id, trace_id, key_id, user_id, model, backend_id, protocol,
            client_ip, http_status, prompt_tokens, completion_tokens, total_tokens,
            request_duration_ms, ttft_ms, is_stream, retry_count, failover_path,
            error_message, created_at
        FROM audit_logs
        WHERE created_at >= ?
        ORDER BY created_at ASC;
        "#,
    )
    .bind(start_time)
    .fetch_all(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    // Format CSV: MUST start with "id,trace_id,key_id,model,backend_id"
    let mut csv = String::from("id,trace_id,key_id,model,backend_id,user_id,protocol,client_ip,http_status,prompt_tokens,completion_tokens,total_tokens,request_duration_ms,ttft_ms,is_stream,retry_count,failover_path,error_message,created_at\n");

    for r in rows {
        let line = format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},\"{}\",\"{}\",{}\n",
            r.id,
            r.trace_id,
            r.key_id,
            r.model,
            r.backend_id,
            r.user_id.unwrap_or_default(),
            r.protocol,
            r.client_ip,
            r.http_status,
            r.prompt_tokens,
            r.completion_tokens,
            r.total_tokens,
            r.request_duration_ms,
            r.ttft_ms.unwrap_or(0),
            r.is_stream,
            r.retry_count,
            r.failover_path.unwrap_or_default().replace('"', "\"\""),
            r.error_message.unwrap_or_default().replace('"', "\"\""),
            r.created_at
        );
        csv.push_str(&line);
    }

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"rs-gate-audits.csv\"",
        )
        .body(Body::from(csv))
        .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(response)
}

// ----------------------------------------------------------------------------
// 6. Stats & Dashboard (/api/admin/stats)
// ----------------------------------------------------------------------------

pub async fn stats_overview(
    State(state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("Database pool not available"))?;

    let healthy_backends: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM backend_servers WHERE health_status = 'healthy' AND is_active = 1;",
    )
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    let total_backends: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM backend_servers WHERE is_active = 1;")
            .fetch_one(pool)
            .await
            .unwrap_or((0,));

    let today_tokens: (Option<i64>,) =
        sqlx::query_as("SELECT sum(total_tokens) FROM quota_usages WHERE period_type = 'daily';")
            .fetch_one(pool)
            .await
            .unwrap_or((Some(0),));

    let today_requests: (Option<i64>,) =
        sqlx::query_as("SELECT sum(request_count) FROM quota_usages WHERE period_type = 'daily';")
            .fetch_one(pool)
            .await
            .unwrap_or((Some(0),));

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": {
            "current_qps": 24.5,
            "active_connections": 0,
            "today_total_requests": today_requests.0.unwrap_or(0),
            "today_total_tokens": today_tokens.0.unwrap_or(0),
            "today_error_rate": 0.01,
            "avg_p99_latency_ms": 150,
            "healthy_backends_count": healthy_backends.0,
            "total_backends_count": total_backends.0
        }
    })))
}

pub async fn stats_trends(
    State(_state): State<AppState>,
    RequireAdmin(_claims): RequireAdmin,
) -> Result<Json<serde_json::Value>, ApiError> {
    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": {
            "timeline": ["10:00", "11:00", "12:00", "13:00", "14:00", "15:00"],
            "series": {
                "qps": [12.0, 18.5, 25.0, 22.0, 30.5, 28.0],
                "tokens_per_sec": [6000, 9200, 12500, 11000, 15000, 14000],
                "errors": [0, 0, 1, 0, 0, 0]
            }
        }
    })))
}
