use axum::{extract::State, Extension, Json};
use serde_json::json;

use crate::auth::{Claims, generate_api_key, JwtManager};
use crate::db::{OrgRepo, UserRepo};
use crate::errors::AppError;
use crate::models::{ApiKeyResponse, LoginRequest, SignupRequest, TokenResponse, UserInfo};
use crate::handlers::AppState;

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, AppError> {
    let user = UserRepo::get_by_email(&state.pool, &req.email)
        .await
        .map_err(|_| AppError::internal("Database error"))?
        .ok_or_else(|| AppError::unauthorized("Invalid email or password"))?;

    let valid = bcrypt::verify(&req.password, &user.password_hash)
        .map_err(|_| AppError::internal("Password verification failed"))?;

    if !valid {
        return Err(AppError::unauthorized("Invalid email or password"));
    }

    let _ = UserRepo::update_last_login(&state.pool, user.id).await;

    let token = state
        .jwt
        .generate_token(user.id, user.organization_id, &user.email, &user.role)
        .map_err(|_| AppError::internal("Token generation failed"))?;

    Ok(Json(TokenResponse {
        token,
        user: UserInfo {
            id: user.id,
            email: user.email,
            first_name: user.first_name,
            last_name: user.last_name,
            role: user.role,
            organization_id: user.organization_id,
        },
    }))
}

pub async fn signup(
    State(state): State<AppState>,
    Json(req): Json<SignupRequest>,
) -> Result<Json<TokenResponse>, AppError> {
    // Create organization
    let settings = json!({
        "default_currency": "USD",
        "timezone": "UTC",
        "fiscal_year_start": 1,
        "alerts_enabled": true
    });

    let org = OrgRepo::create(&state.pool, &req.organization_name, &settings)
        .await
        .map_err(|_| AppError::conflict("Organization already exists"))?;

    // Hash password
    let password_hash = bcrypt::hash(&req.password, bcrypt::DEFAULT_COST)
        .map_err(|_| AppError::internal("Password hashing failed"))?;

    // Create user
    let user = UserRepo::create(
        &state.pool,
        org.id,
        &req.email,
        &password_hash,
        &req.first_name,
        &req.last_name,
        "admin",
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to create user: {:?}", e);
        AppError::conflict("User with this email already exists")
    })?;

    let token = state
        .jwt
        .generate_token(user.id, org.id, &user.email, &user.role)
        .map_err(|_| AppError::internal("Token generation failed"))?;

    Ok(Json(TokenResponse {
        token,
        user: UserInfo {
            id: user.id,
            email: user.email,
            first_name: user.first_name,
            last_name: user.last_name,
            role: user.role,
            organization_id: org.id,
        },
    }))
}

pub async fn me(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<UserInfo>, AppError> {
    let user = UserRepo::get_by_id(&state.pool, claims.sub)
        .await
        .map_err(|_| AppError::not_found("User", &claims.sub.to_string()))?;

    Ok(Json(UserInfo {
        id: user.id,
        email: user.email,
        first_name: user.first_name,
        last_name: user.last_name,
        role: user.role,
        organization_id: user.organization_id,
    }))
}

pub async fn create_api_key(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<ApiKeyResponse>, AppError> {
    let (key, hash) = generate_api_key();

    UserRepo::set_api_key_hash(&state.pool, claims.sub, &hash)
        .await
        .map_err(|_| AppError::internal("Failed to store API key"))?;

    Ok(Json(ApiKeyResponse {
        key,
        message: "Store this key securely. It will not be shown again.".into(),
    }))
}
