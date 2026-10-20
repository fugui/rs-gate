use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

static JWT_SECRET: OnceLock<String> = OnceLock::new();

pub fn get_jwt_secret() -> &'static str {
    JWT_SECRET.get_or_init(|| {
        if let Ok(val) = std::env::var("JWT_SECRET") {
            if !val.trim().is_empty() {
                return val.trim().to_string();
            }
        }
        if cfg!(test) {
            "rs_gate_test_jwt_secret_key_fixed_length_32_bytes!".to_string()
        } else {
            use rand::RngCore;
            let mut key = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut key);
            let hex = key.iter().map(|b| format!("{:02x}", b)).collect::<String>();
            tracing::warn!("Generated ephemeral JWT secret. Set JWT_SECRET in production.");
            hex
        }
    })
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String, // user_id
    pub username: String,
    pub role: String, // "superadmin" | "admin" | "user"
    pub exp: usize,
}

pub fn encode_jwt(claims: &Claims) -> Result<String, jsonwebtoken::errors::Error> {
    encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(get_jwt_secret().as_bytes()),
    )
}

pub fn decode_jwt(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::default();
    validation.validate_exp = true;
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(get_jwt_secret().as_bytes()),
        &validation,
    )?;
    Ok(token_data.claims)
}

pub fn generate_test_token(user_id: &str, role: &str) -> String {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        username: user_id.to_string(),
        role: role.to_string(),
        exp: now + 7 * 86400,
    };
    encode_jwt(&claims).expect("Failed to encode test token")
}

fn extract_token(parts: &Parts) -> Option<String> {
    // 1. Try Authorization header: Bearer <token>
    if let Some(auth_val) = parts.headers.get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_val.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                return Some(token.trim().to_string());
            }
        }
    }

    // 2. Try Cookie: rsgate_token=<token> or rsgate_session=<token>
    if let Some(cookie_val) = parts.headers.get(header::COOKIE) {
        if let Ok(cookie_str) = cookie_val.to_str() {
            for item in cookie_str.split(';') {
                let item = item.trim();
                if let Some(val) = item.strip_prefix("rsgate_token=") {
                    return Some(val.to_string());
                }
                if let Some(val) = item.strip_prefix("rsgate_session=") {
                    return Some(val.to_string());
                }
            }
        }
    }

    None
}

/// Lightweight API error type avoiding large Result Err variants
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: i32,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: i32, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, 50001, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, 40301, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, 40401, message)
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, 40101, message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({
                "code": self.code,
                "message": self.message,
                "data": null
            })),
        )
            .into_response()
    }
}

/// Extractor for any authenticated user
#[derive(Debug, Clone)]
pub struct AuthUser(pub Claims);

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = extract_token(parts)
            .ok_or_else(|| ApiError::unauthorized("Missing authorization token"))?;

        match decode_jwt(&token) {
            Ok(claims) => Ok(AuthUser(claims)),
            Err(_) => Err(ApiError::unauthorized("Invalid or expired token")),
        }
    }
}

/// Extractor requiring admin or superadmin role
#[derive(Debug, Clone)]
pub struct RequireAdmin(pub Claims);

impl<S> FromRequestParts<S> for RequireAdmin
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AuthUser(claims) = AuthUser::from_request_parts(parts, state).await?;
        if claims.role == "admin" || claims.role == "superadmin" {
            Ok(RequireAdmin(claims))
        } else {
            Err(ApiError::forbidden("Forbidden"))
        }
    }
}

/// Extractor requiring superadmin role
#[derive(Debug, Clone)]
pub struct RequireSuperAdmin(pub Claims);

impl<S> FromRequestParts<S> for RequireSuperAdmin
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let AuthUser(claims) = AuthUser::from_request_parts(parts, state).await?;
        if claims.role == "superadmin" {
            Ok(RequireSuperAdmin(claims))
        } else {
            Err(ApiError::forbidden("Forbidden: Requires superadmin role"))
        }
    }
}
