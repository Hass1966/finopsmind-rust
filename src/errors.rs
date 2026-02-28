use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub body: ApiError,
}

impl AppError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            body: ApiError {
                code: "BAD_REQUEST".into(),
                message: msg.into(),
                details: None,
            },
        }
    }

    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            body: ApiError {
                code: "UNAUTHORIZED".into(),
                message: msg.into(),
                details: None,
            },
        }
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            body: ApiError {
                code: "FORBIDDEN".into(),
                message: msg.into(),
                details: None,
            },
        }
    }

    pub fn not_found(resource: &str, id: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            body: ApiError {
                code: "NOT_FOUND".into(),
                message: format!("{resource} with id '{id}' not found"),
                details: None,
            },
        }
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            body: ApiError {
                code: "CONFLICT".into(),
                message: msg.into(),
                details: None,
            },
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            body: ApiError {
                code: "INTERNAL_ERROR".into(),
                message: msg.into(),
                details: None,
            },
        }
    }

    pub fn service_unavailable(service: &str) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            body: ApiError {
                code: "SERVICE_UNAVAILABLE".into(),
                message: format!("{service} is currently unavailable"),
                details: None,
            },
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        tracing::error!("Database error: {:?}", err);
        match err {
            sqlx::Error::RowNotFound => Self::not_found("Resource", "unknown"),
            sqlx::Error::Database(ref db_err) => {
                if db_err.code().as_deref() == Some("23505") {
                    Self::conflict("Resource already exists")
                } else {
                    Self::internal("Database error")
                }
            }
            _ => Self::internal("Database error"),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        tracing::error!("Internal error: {:?}", err);
        Self::internal(err.to_string())
    }
}
