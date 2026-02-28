use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
    Json,
};
use std::sync::Arc;

use super::jwt::{hash_api_key, Claims, JwtManager};
use crate::errors::ApiError;
use crate::db::UserRepo;

#[derive(Clone)]
pub struct AuthState {
    pub jwt: Arc<JwtManager>,
    pub pool: sqlx::PgPool,
}

/// Extract claims from request extensions (set by auth middleware).
pub fn get_claims(req: &Request) -> Option<&Claims> {
    req.extensions().get::<Claims>()
}

/// Auth middleware: validates JWT Bearer or ApiKey from Authorization header.
pub async fn auth_middleware(
    State(state): State<AuthState>,
    mut req: Request,
    next: Next,
) -> Result<Response, (StatusCode, Json<ApiError>)> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let claims = if let Some(token) = auth_header.strip_prefix("Bearer ") {
        state
            .jwt
            .validate_token(token)
            .map_err(|_| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(ApiError {
                        code: "UNAUTHORIZED".into(),
                        message: "Invalid or expired token".into(),
                        details: None,
                    }),
                )
            })?
    } else if let Some(key) = auth_header.strip_prefix("ApiKey ") {
        let hash = hash_api_key(key);
        let user = UserRepo::get_by_api_key_hash(&state.pool, &hash)
            .await
            .map_err(|_| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(ApiError {
                        code: "UNAUTHORIZED".into(),
                        message: "Invalid API key".into(),
                        details: None,
                    }),
                )
            })?
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(ApiError {
                        code: "UNAUTHORIZED".into(),
                        message: "Invalid API key".into(),
                        details: None,
                    }),
                )
            })?;

        Claims {
            sub: user.id,
            org_id: user.organization_id,
            email: user.email,
            role: user.role,
            iat: 0,
            exp: i64::MAX,
        }
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApiError {
                code: "UNAUTHORIZED".into(),
                message: "Missing authorization header".into(),
                details: None,
            }),
        ));
    };

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

/// Role-based access control middleware factory.
pub fn require_role(allowed_roles: &'static [&'static str]) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, (StatusCode, Json<ApiError>)>> + Send>> + Clone + Send {
    move |req: Request, next: Next| {
        let roles = allowed_roles;
        Box::pin(async move {
            let claims = req.extensions().get::<Claims>().ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(ApiError {
                        code: "UNAUTHORIZED".into(),
                        message: "Not authenticated".into(),
                        details: None,
                    }),
                )
            })?;

            if !roles.contains(&claims.role.as_str()) {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ApiError {
                        code: "FORBIDDEN".into(),
                        message: "Insufficient permissions".into(),
                        details: None,
                    }),
                ));
            }

            Ok(next.run(req).await)
        })
    }
}
