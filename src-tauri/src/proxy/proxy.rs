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
            "https://api.openai.com".to_string() // fallback
        };

        let max_cost_msats = if let Some(body_data) = &body {
            if let Some(model_name) = body_data.get("model").and_then(|m| m.as_str()) {
                service
                    .models
                    .iter()
                    .find(|m| m.id == model_name)
                    .and_then(|m| m.sats_pricing.as_ref())
                    .map(|p| (p.max_cost * 1000.0) as u64)
                    .unwrap_or(service.cost_per_request_sats * 1000)
            } else {
                service.cost_per_request_sats * 1000 // Convert sats to msats
            }
        } else {
            service.cost_per_request_sats * 1000 // Convert sats to msats
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

    let mut req_builder = if body.is_some() {
        client.post(&endpoint_url)
    } else {
        client.get(&endpoint_url)
    };

    if let Some(body_data) = &body {
        req_builder = req_builder.json(body_data);
    }

    req_builder = req_builder.header("content-type", "application/json");

    if let Some(accept) = original_headers.get(header::ACCEPT) {
        if let Ok(accept_str) = accept.to_str() {
            req_builder = req_builder.header("accept", accept_str);
        }
    }

    if let Some(auth) = original_headers.get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth.to_str() {
            req_builder = req_builder.header("authorization", auth_str);
        }
    }

    if max_cost_msats > 0 {
        if let Ok(payment_token) =
            create_payment_token(max_cost_msats, selected_mint, &server_state.app_handle).await
        {
            req_builder = req_builder.header("X-Cashu", &payment_token);
        }
    }

    let start_time = start_onion_timing(&endpoint_url);

    match req_builder.send().await {
        Ok(resp) => {
            log_onion_timing(start_time, &endpoint_url, "proxy");
            let status = resp.status();
            let headers = resp.headers().clone();

            println!("{:?}", headers);
            if let Some(change_token) = headers.get("X-Cashu") {
                if let Ok(token_str) = change_token.to_str() {
                    let app_handle_clone = server_state.app_handle.clone();
                    let token_str_owned = token_str.to_string();
                    if let Err(e) = redeem_change_token(&token_str_owned, &app_handle_clone).await {
                        log::error!("Failed to redeem change token in background: {}", e);
                    }
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

            match resp.bytes().await {
                Ok(bytes) => response.body(Body::from(bytes)).unwrap_or_else(|e| {
                    log::error!("Error creating response: {}", e);
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
                }),
                Err(e) => {
                    log::error!("Error reading response body: {}", e);
                    Response::builder()
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
                        .unwrap()
                }
            }
        }
        Err(error) => {
            log::error!("Error forwarding request: {}", error);

            let error_msg = get_onion_error_message(&error, &endpoint_url, "proxy");

            (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": {
                        "message": error_msg,
                        "type": "gateway_error",
                        "code": "request_forwarding_failed"
                    }
                })),
            )
                .into_response()
        }
    }
}

async fn create_payment_token(
    amount_msats: u64,
    selected_mint_url: Option<String>,
    app_handle: &tauri::AppHandle,
) -> Result<String, String> {
    log::info!(
        "Creating payment token for {} sats using mint: {:?}",
        amount_msats,
        selected_mint_url
    );

    let tollgate_state = app_handle.state::<crate::TollGateState>();
    let service = tollgate_state.lock().await;

    match service
        .create_external_token(amount_msats, selected_mint_url)
        .await
    {
        Ok(token) => {
            log::info!(
                "Successfully created payment token for {} sats",
                amount_msats
            );
            Ok(token)
        }
        Err(e) => {
            log::error!("Failed to create payment token: {}", e);
            Err(e.to_string())
        }
    }
}

async fn redeem_change_token(
    change_token: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    log::info!("Redeeming change token: {}", change_token);

    let tollgate_state = app_handle.state::<crate::TollGateState>();
    let service = tollgate_state.lock().await;

    match service.receive_cashu_token(change_token).await {
        Ok(result) => {
            log::info!(
                "Successfully redeemed change token: {} sats from mint {}",
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
