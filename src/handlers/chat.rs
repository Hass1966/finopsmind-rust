use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use chrono::{Duration, Utc};
use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::auth::Claims;
use crate::config::LlmConfig;
use crate::db::{
    AnomalyRepo, BudgetRepo, ChatMessageRepo, CostRepo, ForecastRepo, RecommendationRepo,
};
use crate::errors::AppError;
use crate::handlers::AppState;
use crate::llm::LlmMessage;
use crate::models::{ChatHistoryParams, ChatMessage, PaginatedResponse, Pagination};

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub context: Option<serde_json::Value>,
    #[serde(default)]
    pub stream: bool,
}

#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub response: String,
    pub intent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Gathered org data used to build the LLM system prompt.
struct OrgContext {
    cost_summary: String,
    top_services: String,
    cost_trend: String,
    anomalies: String,
    recommendations: String,
    budgets: String,
    forecast: String,
}

pub async fn chat(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(chat_req): Json<ChatRequest>,
) -> Result<Response, AppError> {
    let intent = detect_intent(&chat_req.message);

    // Fetch conversation history (last 10 messages)
    let history_msgs = ChatMessageRepo::get_recent(&state.pool, claims.sub, 10)
        .await
        .unwrap_or_default();
    let llm_history: Vec<LlmMessage> = history_msgs
        .iter()
        .map(|m| LlmMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        })
        .collect();

    // Gather real data from the database based on intent
    let ctx = gather_context(&state, claims.org_id, &intent).await;
    let system_prompt = build_system_prompt(&ctx, &intent);

    // Save user message
    let _ = ChatMessageRepo::create(
        &state.pool,
        claims.org_id,
        claims.sub,
        "user",
        &chat_req.message,
        None,
    )
    .await;

    let is_anthropic = state.llm_config.provider == "anthropic";

    if chat_req.stream && is_anthropic {
        // Streaming path (Anthropic only)
        let text_stream = crate::llm::call_llm_stream(
            &state.llm_config,
            &system_prompt,
            &llm_history,
            &chat_req.message,
        )
        .await
        .map_err(|e| AppError::service_unavailable(&e.to_string()))?;

        let pool = state.pool.clone();
        let org_id = claims.org_id;
        let user_id = claims.sub;
        let intent_clone = intent.clone();
        let full_response = Arc::new(Mutex::new(String::new()));
        let full_response_for_done = full_response.clone();

        let chunk_stream = text_stream.map(move |chunk_result| {
            let fr = full_response.clone();
            match chunk_result {
                Ok(text) => {
                    // Accumulate full text for DB save
                    if let Ok(mut guard) = fr.try_lock() {
                        guard.push_str(&text);
                    }
                    Ok::<_, Infallible>(
                        Event::default().data(
                            serde_json::json!({"type": "chunk", "text": text}).to_string(),
                        ),
                    )
                }
                Err(e) => Ok(Event::default().data(
                    serde_json::json!({"type": "error", "message": e.to_string()}).to_string(),
                )),
            }
        });

        // Chain a final "done" event that also persists the assistant message
        let done_event = stream::once(async move {
            let full_text = full_response_for_done.lock().await.clone();
            let _ = ChatMessageRepo::create(
                &pool,
                org_id,
                user_id,
                "assistant",
                &full_text,
                Some(&intent_clone),
            )
            .await;
            Ok::<_, Infallible>(Event::default().data(
                serde_json::json!({
                    "type": "done",
                    "response": full_text,
                    "intent": intent_clone
                })
                .to_string(),
            ))
        });

        let sse_stream = chunk_stream.chain(done_event);

        Ok(Sse::new(sse_stream)
            .keep_alive(KeepAlive::default())
            .into_response())
    } else {
        // Non-streaming path (Ollama or explicit non-stream)
        let response = crate::llm::call_llm_with_history(
            &state.llm_config,
            &system_prompt,
            &llm_history,
            &chat_req.message,
        )
        .await
        .map_err(|e| AppError::service_unavailable(&e.to_string()))?;

        // Save assistant message
        let _ = ChatMessageRepo::create(
            &state.pool,
            claims.org_id,
            claims.sub,
            "assistant",
            &response,
            Some(&intent),
        )
        .await;

        Ok(Json(ChatResponse {
            response,
            intent,
            data: None,
        })
        .into_response())
    }
}

pub async fn get_history(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Query(params): Query<ChatHistoryParams>,
) -> Result<Json<PaginatedResponse<ChatMessage>>, AppError> {
    let page = params.page.unwrap_or(1);
    let page_size = params.page_size.unwrap_or(20);
    let offset = (page - 1) * page_size;

    let (messages, total) =
        ChatMessageRepo::list(&state.pool, claims.sub, page_size, offset).await?;

    Ok(Json(PaginatedResponse {
        data: messages,
        pagination: Pagination::new(page, page_size, total),
    }))
}

pub async fn clear_history(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<serde_json::Value>, AppError> {
    ChatMessageRepo::delete_all_for_user(&state.pool, claims.sub).await?;
    Ok(Json(serde_json::json!({"message": "Chat history cleared"})))
}

fn detect_intent(message: &str) -> String {
    let msg = message.to_lowercase();
    if msg.contains("execut")
        || msg.contains("summary")
        || msg.contains("overview")
        || msg.contains("report")
    {
        "executive_summary".into()
    } else if msg.contains("anomal")
        || msg.contains("spike")
        || msg.contains("unusual")
        || msg.contains("surge")
        || msg.contains("jump")
    {
        "anomaly_query".into()
    } else if msg.contains("sav")
        || msg.contains("optim")
        || msg.contains("recommend")
        || msg.contains("reduc")
        || msg.contains("cut")
    {
        "savings_query".into()
    } else if msg.contains("forecast")
        || msg.contains("predict")
        || msg.contains("project")
        || msg.contains("next month")
    {
        "forecast_query".into()
    } else if msg.contains("budget")
        || msg.contains("threshold")
        || msg.contains("limit")
    {
        "budget_query".into()
    } else if msg.contains("cost")
        || msg.contains("spend")
        || msg.contains("bill")
        || msg.contains("expens")
        || msg.contains("charg")
    {
        "cost_query".into()
    } else {
        "general".into()
    }
}

async fn gather_context(state: &AppState, org_id: uuid::Uuid, _intent: &str) -> OrgContext {
    let now = Utc::now().date_naive();
    let days_30_ago = now - Duration::days(30);
    let days_7_ago = now - Duration::days(7);

    let cost_summary = match CostRepo::get_summary(&state.pool, org_id, days_30_ago, now).await {
        Ok(s) => format!(
            "Total cost (last 30 days): ${:.2}. Previous period: ${:.2}. Change: {:.1}%.",
            s.total_cost,
            s.previous_period_cost.unwrap_or(0.0),
            s.change_pct.unwrap_or(0.0),
        ),
        Err(_) => "Cost data unavailable.".into(),
    };

    let top_services =
        match CostRepo::get_breakdown(&state.pool, org_id, days_30_ago, now, "service").await {
            Ok(b) => {
                let lines: Vec<String> = b
                    .items
                    .iter()
                    .take(8)
                    .map(|i| format!("  - {}: ${:.2} ({:.1}%)", i.name, i.amount, i.percentage))
                    .collect();
                if lines.is_empty() {
                    "No service breakdown available.".into()
                } else {
                    format!("Top services by cost:\n{}", lines.join("\n"))
                }
            }
            Err(_) => "Service breakdown unavailable.".into(),
        };

    let cost_trend =
        match CostRepo::get_trend(&state.pool, org_id, days_7_ago, now, "daily").await {
            Ok(t) => {
                let lines: Vec<String> = t
                    .data_points
                    .iter()
                    .map(|p| format!("  {}: ${:.2}", p.date, p.amount))
                    .collect();
                if lines.is_empty() {
                    "No recent cost trend data.".into()
                } else {
                    format!("Daily cost trend (last 7 days):\n{}", lines.join("\n"))
                }
            }
            Err(_) => "Cost trend unavailable.".into(),
        };

    let anomalies = match AnomalyRepo::list(&state.pool, org_id, None, None, 10, 0).await {
        Ok((list, total)) => {
            if list.is_empty() {
                "No anomalies detected.".into()
            } else {
                let lines: Vec<String> = list
                    .iter()
                    .map(|a| {
                        format!(
                        "  - [{}] {}: actual ${}, expected ${}, deviation {:.1}%, severity: {}, status: {}{}",
                        a.date,
                        if a.service.is_empty() { "overall" } else { &a.service },
                        a.actual_amount,
                        a.expected_amount,
                        a.deviation_pct,
                        a.severity,
                        a.status,
                        a.root_cause
                            .as_ref()
                            .map(|r| format!(", root cause: {r}"))
                            .unwrap_or_default(),
                    )
                    })
                    .collect();
                format!("{total} total anomalies. Recent:\n{}", lines.join("\n"))
            }
        }
        Err(_) => "Anomaly data unavailable.".into(),
    };

    let recommendations = match RecommendationRepo::get_summary(&state.pool, org_id).await {
        Ok(s) => {
            let mut text = format!(
                "{} recommendations ({} pending, {} implemented, {} dismissed). Potential savings: ${:.2}. Realized savings: ${:.2}.",
                s.total_count, s.pending_count, s.implemented_count, s.dismissed_count, s.total_savings, s.implemented_savings
            );
            if let Ok((recs, _)) = RecommendationRepo::list(
                &state.pool,
                org_id,
                Some("pending"),
                None,
                None,
                5,
                0,
            )
            .await
            {
                if !recs.is_empty() {
                    let lines: Vec<String> = recs
                        .iter()
                        .map(|r| {
                            format!(
                            "  - {}: {} {} in {}, save ${}/mo ({:.0}%), risk: {}, effort: {}",
                            r.rec_type,
                            r.provider,
                            if r.resource_type.is_empty() {
                                "resource"
                            } else {
                                &r.resource_type
                            },
                            if r.region.is_empty() {
                                "unknown"
                            } else {
                                &r.region
                            },
                            r.estimated_savings,
                            r.estimated_savings_pct,
                            r.risk,
                            r.effort
                        )
                        })
                        .collect();
                    text.push_str(&format!(
                        "\nTop pending recommendations:\n{}",
                        lines.join("\n")
                    ));
                }
            }
            text
        }
        Err(_) => "Recommendation data unavailable.".into(),
    };

    let budgets = match BudgetRepo::list(&state.pool, org_id).await {
        Ok(list) => {
            if list.is_empty() {
                "No budgets configured.".into()
            } else {
                let lines: Vec<String> = list
                    .iter()
                    .map(|b| {
                        let pct = if b.amount > rust_decimal::Decimal::ZERO {
                            let spend_f: f64 = b.current_spend.try_into().unwrap_or(0.0);
                            let amount_f: f64 = b.amount.try_into().unwrap_or(1.0);
                            (spend_f / amount_f) * 100.0
                        } else {
                            0.0
                        };
                        format!(
                            "  - {}: ${} of ${} ({:.1}% used), period: {}, status: {}",
                            b.name, b.current_spend, b.amount, pct, b.period, b.status
                        )
                    })
                    .collect();
                format!("Budgets:\n{}", lines.join("\n"))
            }
        }
        Err(_) => "Budget data unavailable.".into(),
    };

    let forecast = match ForecastRepo::get_latest(&state.pool, org_id).await {
        Ok(Some(f)) => {
            format!(
                "Forecast (model {}, confidence {:.0}%): total projected spend ${} over next period. Generated {}.",
                f.model_version,
                f.confidence_level,
                f.total_forecasted,
                f.generated_at.format("%Y-%m-%d")
            )
        }
        Ok(None) => "No forecast available yet.".into(),
        Err(_) => "Forecast data unavailable.".into(),
    };

    OrgContext {
        cost_summary,
        top_services,
        cost_trend,
        anomalies,
        recommendations,
        budgets,
        forecast,
    }
}

fn build_system_prompt(ctx: &OrgContext, intent: &str) -> String {
    let base_context = format!(
        "=== ORGANIZATION FINANCIAL DATA ===\n\
         {}\n\n\
         {}\n\n\
         {}\n\n\
         {}\n\n\
         {}\n\n\
         {}\n\n\
         {}\n\
         =================================",
        ctx.cost_summary,
        ctx.top_services,
        ctx.cost_trend,
        ctx.anomalies,
        ctx.recommendations,
        ctx.budgets,
        ctx.forecast
    );

    let persona = "You are FinOpsMind AI, an expert FinOps cloud cost management assistant. \
                    You have access to the organisation's real-time cloud financial data shown below. \
                    Always ground your answers in the actual data provided. \
                    Use specific numbers, dates, and service names from the data. \
                    Be concise and actionable. Use bullet points for clarity. \
                    Format currency as USD with commas. \
                    If the data doesn't contain enough information to fully answer, say so honestly.";

    let intent_instruction = match intent {
        "cost_query" => {
            "The user is asking about costs or spending. \
             Analyse the cost summary, service breakdown, and daily trend data. \
             Identify the biggest cost drivers and any notable changes. \
             Compare current vs. previous period spending where relevant."
        }
        "anomaly_query" => {
            "The user is asking about anomalies, spikes, or unusual spending. \
             Focus on the anomaly data. Explain what the anomalies mean in business terms. \
             For each significant anomaly, explain: when it happened, how much it deviated from expected, \
             which service/provider was affected, and what might have caused it. \
             Suggest investigation steps if root cause is unknown."
        }
        "savings_query" => {
            "The user is asking about optimization or savings opportunities. \
             Focus on the recommendation data. For each recommendation, explain: \
             what the opportunity is, how much could be saved, the effort/risk involved, \
             and a prioritised action plan. Group by category (rightsizing, unused resources, \
             reserved instances, etc.) when there are multiple recommendations."
        }
        "forecast_query" => {
            "The user is asking about cost forecasts or projections. \
             Use the forecast data to explain projected spending. \
             Discuss the confidence level, compare to current spending rates, \
             and highlight any trends that suggest costs will increase or decrease. \
             Suggest actions to influence the forecast positively."
        }
        "budget_query" => {
            "The user is asking about budgets. \
             Analyse the budget data. For each budget, explain utilisation, \
             whether it's on track, and what the risk of exceeding is. \
             Flag any budgets in warning or exceeded status with urgency. \
             Suggest corrective actions for at-risk budgets."
        }
        "executive_summary" => {
            "The user wants a high-level executive summary. Provide a structured overview: \
             1) Total spend and trend (up/down/flat vs prior period) \
             2) Key anomalies requiring attention \
             3) Top savings opportunities with total potential \
             4) Budget health status \
             5) Forecast outlook \
             Keep it concise enough for a C-level audience. Lead with the most important insight."
        }
        _ => {
            "Answer the user's question using the available data. \
             If their question doesn't relate to cloud costs, politely redirect them \
             to cloud cost management topics you can help with."
        }
    };

    format!("{persona}\n\n{intent_instruction}\n\n{base_context}")
}
