use crate::connection_server::ConnectionServerState;
use crate::proxy::onion::{
    construct_url_with_protocol, create_onion_client, get_onion_error_message, log_onion_timing,
    start_onion_timing,
};
use crate::routstr::RoutstrService;
use axum::{
    body::Body,
    extract::{Path, Request, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tauri::Manager;

#[derive(serde::Deserialize)]
struct OpenAIRequest {
    #[allow(dead_code)]
    model: Option<String>,
    #[serde(flatten)]
    _other: serde_json::Value,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ProxyConfig {
    pub target_url: String,
    pub use_onion: bool,
    pub payment_required: bool,
    pub cost_msats: u64,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            target_url: "https://api.openai.com".to_string(),
            use_onion: false,
            payment_required: true,
            cost_msats: 65548,
        }
    }
}

pub async fn forward_request_get(
    Path(path): Path<String>,
    headers: HeaderMap,
    server_state: State<ConnectionServerState>,
) -> Response<Body> {
    forward_request_impl(path, None, headers, server_state, false).await
}

pub async fn forward_request_post(
    Path(path): Path<String>,
    headers: HeaderMap,
    server_state: State<ConnectionServerState>,
    request: Request,
) -> Response<Body> {
    let (_, body) = request.into_parts();

    let body_bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(_) => return (StatusCode::BAD_REQUEST, "Failed to read request body").into_response(),
    };

    let body_data: Option<serde_json::Value> = if body_bytes.is_empty() {
        None
    } else {
        match serde_json::from_slice(&body_bytes) {
            Ok(data) => Some(data),
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": {
                            "message": "Invalid JSON in request body",
                            "type": "parse_error",
                            "code": "invalid_json"
                        }
                    })),
                )
                    .into_response()
            }
        }
    };

    forward_request_impl(path, body_data, headers, server_state, false).await
}

async fn forward_request_impl(
    path: String,
    body: Option<serde_json::Value>,
    original_headers: HeaderMap,
    server_state: State<ConnectionServerState>,
    is_streaming: bool,
) -> Response<Body> {
    // Get routstr config from the app state
    let routstr_state = server_state
        .app_handle
        .state::<std::sync::Arc<tokio::sync::Mutex<RoutstrService>>>();

    let (config, max_cost_msats, selected_mint) = {
        let service = routstr_state.lock().await;

        let target_url = if let Some(url) = &service.target_service_url {
            url.clone()
        } else if let Some(url) = &service.base_url {
            url.clone()
        } else {
            "https://api.openai.com".to_string()
        };

        let max_cost_msats = if let Some(body_data) = &body {
            let model = if let Ok(openai_request) = serde_json::from_value::<OpenAIRequest>(
                serde_json::to_value(body_data).unwrap_or_default(),
            ) {
                openai_request.model
            } else {
                None
            };

            if let Some(model) = model {
                service
                    .models
                    .iter()
                    .find(|m| m.id == model)
                    .and_then(|m| m.sats_pricing.as_ref())
                    .map(|p| (p.max_cost * 1000.0) as u64)
                    .unwrap_or(service.cost_per_request_sats * 1000)
            } else {
                service.cost_per_request_sats * 1000
            }
        } else {
            service.cost_per_request_sats * 1000
        };

        let config = ProxyConfig {
            target_url,
            use_onion: service.use_onion,
            payment_required: service.payment_required,
            cost_msats: max_cost_msats,
        };

        let selected_mint = service.selected_mint_url.clone();

        (config, max_cost_msats, selected_mint)
    };

    let endpoint_url = construct_url_with_protocol(&config.target_url, &path);
    log::info!("Forwarding request to: {}", endpoint_url);

    let timeout_secs = if is_streaming { 300 } else { 60 };
    let client = match create_onion_client(&endpoint_url, config.use_onion, Some(timeout_secs)) {
        Ok(client) => client,
        Err(e) => {
            log::error!("Failed to create HTTP client: {}", e);
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": {
                        "message": "Failed to configure HTTP client",
                        "type": "proxy_error",
                        "code": "client_configuration_failed"
                    }
                })),
            )
                .into_response();
        }
    };

    let mut excluded_mints = Vec::new();
    let mut payment_token = if config.payment_required && max_cost_msats > 0 {
        match create_payment_token(
            max_cost_msats,
            selected_mint.clone(),
            &excluded_mints,
            &server_state.app_handle,
        )
        .await
        {
            Ok(payment) => Some(payment),
            Err(error) => {
                log::error!(
                    "[wallet] Failed to create required payment token: {}",
                    error
                );
                return payment_error_response(&error);
            }
        }
    } else {
        None
    };

    loop {
        let req_builder = build_provider_request(
            &client,
            &endpoint_url,
            &body,
            &original_headers,
            payment_token.as_ref(),
        );
        let start_time = start_onion_timing(&endpoint_url);

        let resp = match req_builder.send().await {
            Ok(resp) => resp,
            Err(error) => {
                log::error!("Error forwarding request: {}", error);
                if let Some(payment) = payment_token.as_ref() {
                    reclaim_payment_token(payment, &server_state.app_handle).await;
                }

                let error_msg = get_onion_error_message(&error, &endpoint_url, "proxy");
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({
                        "error": {
                            "message": error_msg,
                            "type": "gateway_error",
                            "code": "request_forwarding_failed"
                        }
                    })),
                )
                    .into_response();
            }
        };

        log_onion_timing(start_time, &endpoint_url, "proxy");
        let status = resp.status();
        let headers = resp.headers().clone();
        let response_bytes = match resp.bytes().await {
            Ok(bytes) => bytes,
            Err(error) => {
                if let Some(payment) = payment_token.as_ref() {
                    reclaim_payment_token(payment, &server_state.app_handle).await;
                }
                log::error!("Error reading response body: {}", error);
                return Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Body::from(
                        json!({
                            "error": {
                                "message": "Error reading response from provider",
                                "type": "gateway_error",
                                "code": "response_read_failed"
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap();
            }
        };

        if is_mint_unreachable_response(status, &response_bytes) {
            if let Some(failed_payment) = payment_token.take() {
                log::warn!(
                    "[wallet] Provider rejected payment because mint {} is unreachable",
                    failed_payment.mint_url
                );
                reclaim_payment_token(&failed_payment, &server_state.app_handle).await;
                excluded_mints.push(failed_payment.mint_url);

                match create_payment_token(
                    max_cost_msats,
                    selected_mint.clone(),
                    &excluded_mints,
                    &server_state.app_handle,
                )
                .await
                {
                    Ok(fallback_payment) => {
                        log::info!(
                            "[wallet] Retrying provider with fallback mint {} ({})",
                            fallback_payment.mint_url,
                            excluded_mints.len() + 1
                        );
                        payment_token = Some(fallback_payment);
                        continue;
                    }
                    Err(error) => {
                        log::warn!(
                            "[wallet] No fallback mint could create a replacement payment token: {}",
                            error
                        );
                    }
                }
            }
        }

        let change_token = headers
            .get("X-Cashu")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        if let Some(change_token) = change_token {
            if let Err(error) = redeem_change_token(&change_token, &server_state.app_handle).await {
                log::error!("[wallet] Failed to redeem provider change token: {}", error);
            }
        } else if !status.is_success() {
            if let Some(payment) = payment_token.as_ref() {
                reclaim_payment_token(payment, &server_state.app_handle).await;
            }
        }

        let mut response = Response::builder().status(
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        );

        if is_streaming && !headers.contains_key("content-type") {
            response = response.header("content-type", "text/event-stream");
        }

        if let Some(content_type) = headers.get("content-type") {
            if let Ok(ct_str) = content_type.to_str() {
                response = response.header("content-type", ct_str);
            }
        }

        return response
            .body(Body::from(response_bytes))
            .unwrap_or_else(|error| {
                log::error!("Error creating response: {}", error);
                Response::builder()
                    .status(StatusCode::BAD_GATEWAY)
                    .body(Body::from(
                        json!({
                            "error": {
                                "message": "Error processing provider response",
                                "type": "gateway_error",
                                "code": "response_processing_failed"
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap()
            });
    }
}

#[derive(Clone)]
struct PaymentToken {
    token: String,
    mint_url: String,
}

fn build_provider_request(
    client: &reqwest::Client,
    endpoint_url: &str,
    body: &Option<serde_json::Value>,
    original_headers: &HeaderMap,
    payment_token: Option<&PaymentToken>,
) -> reqwest::RequestBuilder {
    let mut request = if body.is_some() {
        client.post(endpoint_url)
    } else {
        client.get(endpoint_url)
    };

    if let Some(body_data) = body {
        request = request.json(body_data);
    }

    request = request.header("content-type", "application/json");
    if let Some(accept) = original_headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
    {
        request = request.header("accept", accept);
    }
    if let Some(auth) = original_headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
    {
        request = request.header("authorization", auth);
    }
    if let Some(payment) = payment_token {
        request = request.header("X-Cashu", &payment.token);
    }
    request
}

fn msats_to_sats_rounded_up(amount_msats: u64) -> u64 {
    amount_msats / 1_000 + u64::from(amount_msats % 1_000 != 0)
}

fn value_contains_mint_unreachable(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::String(text) => text_contains_mint_unreachable(text),
        serde_json::Value::Array(values) => values.iter().any(value_contains_mint_unreachable),
        serde_json::Value::Object(values) => values.values().any(value_contains_mint_unreachable),
        _ => false,
    }
}

fn text_contains_mint_unreachable(text: &str) -> bool {
    let normalized = text.trim().to_ascii_lowercase();
    let words: Vec<&str> = normalized
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    if words.windows(2).any(|pair| pair == ["mint", "unreachable"]) {
        return true;
    }

    serde_json::from_str::<serde_json::Value>(&normalized)
        .map(|value| value_contains_mint_unreachable(&value))
        .unwrap_or(false)
}

fn is_mint_unreachable_response(status: reqwest::StatusCode, body: &[u8]) -> bool {
    status.is_server_error()
        && std::str::from_utf8(body)
            .map(text_contains_mint_unreachable)
            .unwrap_or(false)
}

fn payment_error_response(error: &str) -> Response<Body> {
    (
        StatusCode::PAYMENT_REQUIRED,
        Json(json!({
            "error": {
                "message": error,
                "type": "payment_error",
                "code": "payment_token_creation_failed"
            }
        })),
    )
        .into_response()
}

async fn create_payment_token(
    amount_msats: u64,
    selected_mint_url: Option<String>,
    excluded_mints: &[String],
    app_handle: &tauri::AppHandle,
) -> Result<PaymentToken, String> {
    let amount_sats = msats_to_sats_rounded_up(amount_msats);
    log::info!(
        "[wallet] Creating payment token for {} sats; preferred mint: {:?}",
        amount_sats,
        selected_mint_url
    );

    let wallet_state = app_handle.state::<crate::WalletState>();
    let service = wallet_state.lock().await;

    match service
        .create_external_token_with_fallback(amount_sats, selected_mint_url, excluded_mints)
        .await
    {
        Ok(payment) => {
            log::info!(
                "[wallet] Created payment token for {} sats from mint {}",
                amount_sats,
                payment.mint_url
            );
            Ok(PaymentToken {
                token: payment.token,
                mint_url: payment.mint_url,
            })
        }
        Err(error) => Err(error.to_string()),
    }
}

async fn reclaim_payment_token(payment: &PaymentToken, app_handle: &tauri::AppHandle) {
    if let Err(error) = redeem_change_token(&payment.token, app_handle).await {
        log::error!(
            "[wallet] Failed to reclaim payment token from mint {}: {}",
            payment.mint_url,
            error
        );
    }
}

async fn redeem_change_token(
    change_token: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    log::info!("[wallet] Redeeming Cashu change token");

    let wallet_state = app_handle.state::<crate::WalletState>();
    let service = wallet_state.lock().await;

    match service.receive_cashu_token(change_token).await {
        Ok(result) => {
            log::info!(
                "[wallet] Redeemed change token: {} wallet units from mint {}",
                result.amount,
                result.mint_url
            );
            Ok(())
        }
        Err(e) => {
            log::error!("Failed to redeem change token: {}", e);
            Err(e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_msats_up_at_the_wallet_boundary() {
        assert_eq!(msats_to_sats_rounded_up(0), 0);
        assert_eq!(msats_to_sats_rounded_up(1), 1);
        assert_eq!(msats_to_sats_rounded_up(999), 1);
        assert_eq!(msats_to_sats_rounded_up(1_000), 1);
        assert_eq!(msats_to_sats_rounded_up(1_001), 2);
    }

    #[test]
    fn detects_nested_mint_unreachable_server_errors() {
        let body = br#"{"detail":{"error":{"type":"mint_unreachable"}}}"#;
        assert!(is_mint_unreachable_response(
            reqwest::StatusCode::SERVICE_UNAVAILABLE,
            body
        ));
        assert!(!is_mint_unreachable_response(
            reqwest::StatusCode::BAD_REQUEST,
            body
        ));
    }

    #[test]
    fn ignores_unrelated_provider_errors() {
        assert!(!is_mint_unreachable_response(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            br#"{"error":"insufficient balance"}"#
        ));
        assert!(!is_mint_unreachable_response(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            br#"{"error":"mint list loaded but provider is unreachable"}"#
        ));
    }
}
