pub mod admin_api;
pub mod auth;
pub mod embedded;
pub mod user_api;

use crate::admin::auth::{encode_jwt, ApiError, AuthUser, Claims};
use crate::server::AppState;
use crate::storage::bootstrap::verify_password;
use axum::{
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

pub async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginPayload>,
) -> Result<Response, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("DB pool unavailable"))?;

    let user_row: Option<(String, String, String, String, String, String, i64)> = sqlx::query_as(
        "SELECT id, username, password_hash, role, display_name, email, created_at FROM users WHERE username = ?;"
    )
    .bind(&payload.username)
    .fetch_optional(pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let (id, username, password_hash, role, display_name, email, created_at) = match user_row {
        Some(u) => u,
        None => {
            return Err(ApiError::unauthorized("Incorrect username or password"));
        }
    };

    let is_valid = verify_password(&payload.password, &password_hash).unwrap_or(false);
    if !is_valid {
        return Err(ApiError::unauthorized("Incorrect username or password"));
    }

    let now_sec = chrono::Utc::now().timestamp() as usize;
    let exp = now_sec + 7 * 86400;
    let claims = Claims {
        sub: id.clone(),
        username: username.clone(),
        role: role.clone(),
        exp,
    };

    let token =
        encode_jwt(&claims).map_err(|e| ApiError::internal(format!("JWT encode error: {}", e)))?;

    let cookie = format!("rsgate_token={}; Path=/; HttpOnly; SameSite=Lax", token);

    let res = Response::builder()
        .status(StatusCode::OK)
        .header(header::SET_COOKIE, HeaderValue::from_str(&cookie).unwrap())
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(
            serde_json::to_string(&serde_json::json!({
                "code": 0,
                "message": "success",
                "data": {
                    "token": token,
                    "expires_at": exp,
                    "user": {
                        "id": id,
                        "username": username,
                        "display_name": display_name,
                        "email": email,
                        "role": role,
                        "created_at": created_at
                    }
                }
            }))
            .unwrap(),
        ))
        .unwrap();

    Ok(res)
}

pub async fn logout_handler() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::SET_COOKIE,
            HeaderValue::from_static("rsgate_token=; Path=/; HttpOnly; Max-Age=0"),
        )
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(
            serde_json::to_string(&serde_json::json!({
                "code": 0,
                "message": "success",
                "data": null
            }))
            .unwrap(),
        ))
        .unwrap()
}

pub async fn me_handler(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pool = state
        .pool
        .as_ref()
        .ok_or_else(|| ApiError::internal("DB pool unavailable"))?;

    let user_row: Option<(String, String)> =
        sqlx::query_as("SELECT display_name, email FROM users WHERE id = ?;")
            .bind(&claims.sub)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

    let display_name = user_row
        .as_ref()
        .map(|u| u.0.clone())
        .unwrap_or_else(|| claims.username.clone());
    let email = user_row.as_ref().map(|u| u.1.clone()).unwrap_or_default();

    // Summary of tokens
    let total_keys: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM virtual_keys WHERE user_id = ? AND is_active = 1;")
            .bind(&claims.sub)
            .fetch_one(pool)
            .await
            .unwrap_or((0,));

    let daily_used: (Option<i64>,) = sqlx::query_as(
        r#"
        SELECT sum(qu.total_tokens)
        FROM quota_usages qu
        JOIN virtual_keys vk ON qu.key_id = vk.id
        WHERE vk.user_id = ? AND qu.period_type = 'daily';
        "#,
    )
    .bind(&claims.sub)
    .fetch_one(pool)
    .await
    .unwrap_or((Some(0),));

    let weekly_used: (Option<i64>,) = sqlx::query_as(
        r#"
        SELECT sum(qu.total_tokens)
        FROM quota_usages qu
        JOIN virtual_keys vk ON qu.key_id = vk.id
        WHERE vk.user_id = ? AND qu.period_type = 'weekly';
        "#,
    )
    .bind(&claims.sub)
    .fetch_one(pool)
    .await
    .unwrap_or((Some(0),));

    Ok(Json(serde_json::json!({
        "code": 0,
        "message": "success",
        "data": {
            "user": {
                "id": claims.sub,
                "username": claims.username,
                "display_name": display_name,
                "email": email,
                "role": claims.role
            },
            "quota_summary": {
                "total_keys": total_keys.0,
                "daily_tokens_used": daily_used.0.unwrap_or(0),
                "daily_tokens_limit": 1_000_000,
                "weekly_tokens_used": weekly_used.0.unwrap_or(0),
                "weekly_tokens_limit": 5_000_000
            }
        }
    })))
}

/// 注册所有 /api 控制台与自服务路由
pub fn build_admin_router(state: AppState) -> Router {
    let auth_router = Router::new()
        .route("/api/auth/login", post(login_handler))
        .route("/api/auth/logout", post(logout_handler))
        .route("/api/auth/me", get(me_handler));

    let user_router = Router::new()
        .route("/api/user/models", get(user_api::list_models))
        .route(
            "/api/user/keys",
            get(user_api::list_keys).post(user_api::create_key),
        )
        .route(
            "/api/user/keys/{id}",
            axum::routing::put(user_api::update_key).delete(user_api::delete_key),
        )
        .route("/api/user/audits", get(user_api::list_user_audits));

    let admin_ops_router = Router::new()
        // Users
        .route(
            "/api/admin/users",
            get(admin_api::list_users).post(admin_api::create_user),
        )
        .route(
            "/api/admin/users/{id}",
            axum::routing::put(admin_api::update_user).delete(admin_api::delete_user),
        )
        // Keys
        .route("/api/admin/keys", get(admin_api::list_admin_keys))
        .route(
            "/api/admin/keys/{id}/reset-quota",
            post(admin_api::reset_key_quota),
        )
        // Backends
        .route(
            "/api/admin/backends",
            get(admin_api::list_backends).post(admin_api::create_backend),
        )
        .route(
            "/api/admin/backends/{id}",
            axum::routing::delete(admin_api::delete_backend),
        )
        .route(
            "/api/admin/backends/{id}/probe",
            post(admin_api::probe_backend),
        )
        // Models
        .route(
            "/api/admin/models",
            get(admin_api::list_admin_models).post(admin_api::create_model),
        )
        .route(
            "/api/admin/models/{id}/backends",
            post(admin_api::bind_model_backend),
        )
        .route(
            "/api/admin/models/{id}/backends/{backend_id}",
            axum::routing::delete(admin_api::unbind_model_backend),
        )
        // Audits
        .route("/api/admin/audits", get(admin_api::list_admin_audits))
        .route(
            "/api/admin/audits/export",
            get(admin_api::export_audits_csv),
        )
        // Stats
        .route("/api/admin/stats/overview", get(admin_api::stats_overview))
        .route("/api/admin/stats/trends", get(admin_api::stats_trends));

    auth_router
        .merge(user_router)
        .merge(admin_ops_router)
        .with_state(state)
}
