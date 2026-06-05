//! Auth — JWT signing, extraction, and user resolution.

use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
    response::IntoResponse,
    Json,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::future::Future;
use std::pin::Pin;
use uuid::Uuid;

use crate::state::AppState;

/// Extracted from a valid JWT in the Authorization header.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
}

/// JWT claims payload.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub exp: usize,
}

/// Generate a JWT for the given user. 24h expiry.
pub fn sign_token(user_id: Uuid, secret: &str) -> anyhow::Result<String> {
    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .unwrap()
        .timestamp() as usize;
    let claims = Claims { sub: user_id, exp };
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok(token)
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = axum::response::Response;

    fn from_request_parts<'life0, 'life1, 'async_trait>(
        parts: &'life0 mut Parts,
        state: &'life1 AppState,
    ) -> Pin<Box<dyn Future<Output = Result<Self, Self::Rejection>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        let secret = state.config.security.jwt_secret.clone();
        Box::pin(async move {
            let auth_header = parts
                .headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "));

            let token = match auth_header {
                Some(t) => t,
                None => {
                    return Err((
                        StatusCode::UNAUTHORIZED,
                        Json(
                            json!({"error": {"code": "UNAUTHORIZED", "message": "Missing Authorization header"}}),
                        ),
                    )
                        .into_response());
                }
            };

            let claims = decode::<Claims>(
                token,
                &DecodingKey::from_secret(secret.as_bytes()),
                &Validation::default(),
            )
            .map(|d| d.claims)
            .map_err(|_| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": {"code": "UNAUTHORIZED", "message": "Invalid token"}})),
                )
                    .into_response()
            })?;

            Ok(AuthUser {
                user_id: claims.sub,
            })
        })
    }
}

/// Resolve a user_id from auth or error. No silent fallback to first user.
pub async fn require_user(auth: Option<AuthUser>) -> Result<Uuid, crate::error::AppError> {
    match auth {
        Some(user) => Ok(user.user_id),
        None => Err(crate::error::AppError::Unauthorized(
            "Local session required. Use POST /api/v1/auth/local-session.".into(),
        )),
    }
}
