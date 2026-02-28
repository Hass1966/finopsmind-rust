use axum::{
    extract::{Path, State},
    Extension, Json,
};
use chrono::Utc;
use uuid::Uuid;

use crate::auth::Claims;
use crate::db::RemediationRepo;
use crate::errors::AppError;
use crate::models::*;
use crate::handlers::AppState;

pub async fn list(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<PaginatedResponse<RemediationAction>>, AppError> {
    let (actions, total) = RemediationRepo::list_actions(&state.pool, claims.org_id, 50, 0).await?;
    Ok(Json(PaginatedResponse {
        data: actions,
        pagination: Pagination::new(1, 50, total),
    }))
}

pub async fn get_by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<RemediationAction>, AppError> {
    let action = RemediationRepo::get_action(&state.pool, claims.org_id, id).await
        .map_err(|_| AppError::not_found("Remediation", &id.to_string()))?;
    Ok(Json(action))
}

pub async fn propose(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(propose_req): Json<ProposeRemediationRequest>,
) -> Result<Json<RemediationAction>, AppError> {
    let audit_entry = serde_json::json!([{
        "timestamp": Utc::now().to_rfc3339(),
        "actor": claims.email,
        "action": "proposed",
        "details": "Remediation action proposed"
    }]);

    let action = RemediationAction {
        id: Uuid::new_v4(),
        organization_id: claims.org_id,
        recommendation_id: propose_req.recommendation_id,
        action_type: propose_req.action_type,
        status: "pending_approval".into(),
        provider: propose_req.provider,
        account_id: propose_req.account_id,
        region: propose_req.region,
        resource_id: propose_req.resource_id,
        resource_type: propose_req.resource_type,
        description: propose_req.description,
        current_state: propose_req.current_state.unwrap_or_default(),
        desired_state: propose_req.desired_state.unwrap_or_default(),
        estimated_savings: rust_decimal::Decimal::from_f64_retain(propose_req.estimated_savings.unwrap_or(0.0)).unwrap_or_default(),
        currency: "USD".into(),
        risk: propose_req.risk.unwrap_or_else(|| "low".into()),
        auto_approved: false,
        approval_rule: None,
        requested_by: Some(claims.sub),
        approved_by: None,
        approved_at: None,
        executed_at: None,
        completed_at: None,
        rolled_back_at: None,
        failure_reason: None,
        rollback_data: serde_json::json!({}),
        audit_log: audit_entry,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    let created = RemediationRepo::create_action(&state.pool, &action).await?;

    // Check auto-approval rules
    let rules = RemediationRepo::get_active_rules(&state.pool, claims.org_id).await?;
    for rule in &rules {
        if matches_auto_approval(&created, rule) {
            let mut log: Vec<serde_json::Value> = serde_json::from_value(created.audit_log.clone()).unwrap_or_default();
            log.push(serde_json::json!({
                "timestamp": Utc::now().to_rfc3339(),
                "actor": "system",
                "action": "auto_approved",
                "details": format!("Auto-approved by rule: {}", rule.name)
            }));
            let log_val = serde_json::to_value(&log).unwrap_or_default();

            let approved = RemediationRepo::approve_action(&state.pool, created.id, claims.sub, &log_val).await?;
            return Ok(Json(approved));
        }
    }

    Ok(Json(created))
}

fn matches_auto_approval(action: &RemediationAction, rule: &AutoApprovalRule) -> bool {
    let conditions = &rule.conditions;

    if let Some(max) = conditions.get("max_savings").and_then(|v| v.as_f64()) {
        let savings: f64 = action.estimated_savings.try_into().unwrap_or(0.0);
        if savings > max {
            return false;
        }
    }

    if let Some(types) = conditions.get("allowed_types").and_then(|v| v.as_array()) {
        let type_strs: Vec<&str> = types.iter().filter_map(|v| v.as_str()).collect();
        if !type_strs.is_empty() && !type_strs.contains(&action.action_type.as_str()) {
            return false;
        }
    }

    if let Some(risks) = conditions.get("allowed_risks").and_then(|v| v.as_array()) {
        let risk_strs: Vec<&str> = risks.iter().filter_map(|v| v.as_str()).collect();
        if !risk_strs.is_empty() && !risk_strs.contains(&action.risk.as_str()) {
            return false;
        }
    }

    true
}

pub async fn approve(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<RemediationAction>, AppError> {
    let existing = RemediationRepo::get_action(&state.pool, claims.org_id, id).await
        .map_err(|_| AppError::not_found("Remediation", &id.to_string()))?;

    let mut log: Vec<serde_json::Value> = serde_json::from_value(existing.audit_log).unwrap_or_default();
    log.push(serde_json::json!({
        "timestamp": Utc::now().to_rfc3339(),
        "actor": claims.email,
        "action": "approved",
        "details": "Manually approved"
    }));
    let log_val = serde_json::to_value(&log).unwrap_or_default();

    let action = RemediationRepo::approve_action(&state.pool, id, claims.sub, &log_val).await?;
    Ok(Json(action))
}

pub async fn reject(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(reject_req): Json<RejectRemediationRequest>,
) -> Result<Json<RemediationAction>, AppError> {
    let existing = RemediationRepo::get_action(&state.pool, claims.org_id, id).await
        .map_err(|_| AppError::not_found("Remediation", &id.to_string()))?;

    let mut log: Vec<serde_json::Value> = serde_json::from_value(existing.audit_log).unwrap_or_default();
    log.push(serde_json::json!({
        "timestamp": Utc::now().to_rfc3339(),
        "actor": claims.email,
        "action": "rejected",
        "details": reject_req.reason
    }));
    let log_val = serde_json::to_value(&log).unwrap_or_default();

    let action = RemediationRepo::reject_action(&state.pool, id, claims.sub, &reject_req.reason, &log_val).await?;
    Ok(Json(action))
}

pub async fn cancel(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<RemediationAction>, AppError> {
    let existing = RemediationRepo::get_action(&state.pool, claims.org_id, id).await
        .map_err(|_| AppError::not_found("Remediation", &id.to_string()))?;

    let mut log: Vec<serde_json::Value> = serde_json::from_value(existing.audit_log).unwrap_or_default();
    log.push(serde_json::json!({
        "timestamp": Utc::now().to_rfc3339(),
        "actor": claims.email,
        "action": "cancelled",
        "details": "Cancelled by user"
    }));
    let log_val = serde_json::to_value(&log).unwrap_or_default();

    let action = RemediationRepo::update_action_status(&state.pool, id, "cancelled", &log_val).await?;
    Ok(Json(action))
}

pub async fn rollback(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<RemediationAction>, AppError> {
    let existing = RemediationRepo::get_action(&state.pool, claims.org_id, id).await
        .map_err(|_| AppError::not_found("Remediation", &id.to_string()))?;

    if existing.status != "completed" {
        return Err(AppError::bad_request("Can only rollback completed actions"));
    }

    let mut log: Vec<serde_json::Value> = serde_json::from_value(existing.audit_log).unwrap_or_default();
    log.push(serde_json::json!({
        "timestamp": Utc::now().to_rfc3339(),
        "actor": claims.email,
        "action": "rolled_back",
        "details": "Rolled back by user"
    }));
    let log_val = serde_json::to_value(&log).unwrap_or_default();

    let action = RemediationRepo::rollback_action(&state.pool, id, &log_val).await?;
    Ok(Json(action))
}

pub async fn get_summary(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<RemediationSummary>, AppError> {
    let summary = RemediationRepo::get_summary(&state.pool, claims.org_id).await?;
    Ok(Json(summary))
}

pub async fn list_rules(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<Vec<AutoApprovalRule>>, AppError> {
    let rules = RemediationRepo::list_rules(&state.pool, claims.org_id).await?;
    Ok(Json(rules))
}

pub async fn create_rule(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(create_req): Json<CreateAutoApprovalRuleRequest>,
) -> Result<Json<AutoApprovalRule>, AppError> {
    let rule = RemediationRepo::create_rule(
        &state.pool,
        claims.org_id,
        &create_req.name,
        create_req.enabled.unwrap_or(true),
        &create_req.conditions,
        Some(claims.sub),
    )
    .await?;
    Ok(Json(rule))
}

pub async fn update_rule(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
    Json(update_req): Json<UpdateAutoApprovalRuleRequest>,
) -> Result<Json<AutoApprovalRule>, AppError> {
    let rule = RemediationRepo::update_rule(
        &state.pool,
        claims.org_id,
        id,
        update_req.name.as_deref(),
        update_req.enabled,
        update_req.conditions.as_ref(),
    )
    .await?;
    Ok(Json(rule))
}

pub async fn delete_rule(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    RemediationRepo::delete_rule(&state.pool, claims.org_id, id).await?;
    Ok(Json(serde_json::json!({"message": "Rule deleted"})))
}
