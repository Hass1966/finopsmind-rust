use axum::{
    extract::{Path, State},
    Extension, Json,
};
use uuid::Uuid;

use crate::auth::Claims;
use crate::cloud;
use crate::db::{CloudProviderRepo, CostRepo};
use crate::errors::AppError;
use crate::models::{CloudProviderConfig, CreateProviderRequest, UpdateProviderRequest, AwsCredentials, AzureCredentials, CostRecord};
use crate::handlers::AppState;

pub async fn list(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<CloudProviderConfig>>, AppError> {
    let providers = CloudProviderRepo::list(&state.pool, claims.org_id).await?;
    Ok(Json(providers))
}

pub async fn create(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(create_req): Json<CreateProviderRequest>,
) -> Result<Json<CloudProviderConfig>, AppError> {
    let creds_json = serde_json::to_vec(&create_req.credentials)
        .map_err(|_| AppError::bad_request("Invalid credentials format"))?;

    let encrypted = crate::crypto::encrypt(&creds_json, &state.encryption_key)
        .map_err(|e| AppError::internal(format!("Encryption error: {e}")))?;

    let config = CloudProviderConfig {
        id: Uuid::new_v4(),
        organization_id: claims.org_id,
        provider_type: create_req.provider_type,
        name: create_req.name,
        credentials: Some(encrypted),
        enabled: true,
        status: "pending".into(),
        status_message: None,
        last_sync_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let provider = CloudProviderRepo::create(&state.pool, &config).await?;
    Ok(Json(provider))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(update_req): Json<UpdateProviderRequest>,
) -> Result<Json<CloudProviderConfig>, AppError> {
    let encrypted = if let Some(creds) = &update_req.credentials {
        let creds_json = serde_json::to_vec(creds)
            .map_err(|_| AppError::bad_request("Invalid credentials format"))?;
        Some(crate::crypto::encrypt(&creds_json, &state.encryption_key)
            .map_err(|e| AppError::internal(format!("Encryption error: {e}")))?)
    } else {
        None
    };

    let provider = CloudProviderRepo::update(
        &state.pool,
        claims.org_id,
        id,
        update_req.name.as_deref(),
        encrypted.as_deref(),
        update_req.enabled,
    )
    .await?;
    Ok(Json(provider))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    CloudProviderRepo::delete(&state.pool, claims.org_id, id).await?;
    Ok(Json(serde_json::json!({"message": "Provider deleted"})))
}

/// Decrypt credentials from a provider record.
fn decrypt_credentials(provider: &CloudProviderConfig, encryption_key: &str) -> Result<serde_json::Value, AppError> {
    let creds_enc = provider.credentials.as_ref()
        .ok_or_else(|| AppError::bad_request("No credentials stored for this provider"))?;
    let creds_bytes = crate::crypto::decrypt(creds_enc, encryption_key)
        .map_err(|e| AppError::internal(format!("Decryption error: {e}")))?;
    serde_json::from_slice(&creds_bytes)
        .map_err(|e| AppError::internal(format!("Credential parse error: {e}")))
}

pub async fn test_connection(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let provider = CloudProviderRepo::get_by_id(&state.pool, claims.org_id, id).await
        .map_err(|_| AppError::not_found("Provider", &id.to_string()))?;

    let creds_json = decrypt_credentials(&provider, &state.encryption_key)?;

    let result = match provider.provider_type.as_str() {
        "aws" => {
            let aws_creds: AwsCredentials = serde_json::from_value(creds_json)
                .map_err(|e| AppError::bad_request(format!("Invalid AWS credentials: {e}")))?;
            cloud::aws::test_credentials(&aws_creds).await
        }
        "azure" => {
            let azure_creds: AzureCredentials = serde_json::from_value(creds_json)
                .map_err(|e| AppError::bad_request(format!("Invalid Azure credentials: {e}")))?;
            cloud::azure::test_credentials(&azure_creds).await
        }
        _ => {
            // GCP and others: mark as connected for now
            cloud::TestResult {
                success: true,
                message: format!("Provider type '{}' connection simulated", provider.provider_type),
            }
        }
    };

    let status = if result.success { "connected" } else { "failed" };
    CloudProviderRepo::update_status(&state.pool, id, status, Some(&result.message)).await?;

    Ok(Json(serde_json::json!({
        "status": status,
        "provider": provider.provider_type,
        "message": result.message
    })))
}

pub async fn trigger_sync(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    let provider = CloudProviderRepo::get_by_id(&state.pool, claims.org_id, id).await
        .map_err(|_| AppError::not_found("Provider", &id.to_string()))?;

    let creds_json = decrypt_credentials(&provider, &state.encryption_key)?;

    // Determine sync window: last 30 days
    let end_date = chrono::Utc::now().date_naive();
    let start_date = end_date - chrono::Duration::days(30);

    let cost_items = match provider.provider_type.as_str() {
        "aws" => {
            let aws_creds: AwsCredentials = serde_json::from_value(creds_json)
                .map_err(|e| AppError::bad_request(format!("Invalid AWS credentials: {e}")))?;
            let account = aws_creds.access_key_id.clone();
            cloud::aws::sync_costs(&aws_creds, start_date, end_date, &account).await
                .map_err(|e| AppError::internal(format!("AWS sync error: {e}")))?
        }
        "azure" => {
            let azure_creds: AzureCredentials = serde_json::from_value(creds_json)
                .map_err(|e| AppError::bad_request(format!("Invalid Azure credentials: {e}")))?;
            cloud::azure::sync_costs(&azure_creds, start_date, end_date).await
                .map_err(|e| AppError::internal(format!("Azure sync error: {e}")))?
        }
        _ => {
            Vec::new()
        }
    };

    // Convert CloudCostItems to CostRecords and insert
    let records: Vec<CostRecord> = cost_items
        .into_iter()
        .map(|item| CostRecord {
            id: Uuid::new_v4(),
            organization_id: claims.org_id,
            date: item.date,
            amount: rust_decimal::Decimal::from_f64_retain(item.amount).unwrap_or_default(),
            currency: item.currency,
            provider: provider.provider_type.clone(),
            service: item.service,
            account_id: item.account_id,
            region: item.region,
            resource_id: item.resource_id,
            tags: item.tags,
            estimated: item.estimated,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .collect();

    let count = records.len();
    if !records.is_empty() {
        CostRepo::create_batch(&state.pool, &records).await
            .map_err(|e| AppError::internal(format!("Failed to insert cost records: {e}")))?;
    }

    CloudProviderRepo::update_sync_time(&state.pool, id).await?;

    // Invalidate cost caches after sync
    let _ = crate::cache::invalidate_pattern(&state.redis, &format!("costs:{}:*", claims.org_id)).await;

    Ok(Json(serde_json::json!({
        "message": "Sync completed",
        "records_synced": count,
        "status": "connected"
    })))
}
