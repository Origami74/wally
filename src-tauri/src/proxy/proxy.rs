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
    pub cost_sats: u64,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            target_url: "https://api.openai.com".to_string(),
            use_onion: false,
            payment_required: false,
            cost_sats: 0,
        }
    }
}

pub async fn forward_request_get(
    Path(path): Path<String>,
    headers: HeaderMap,
    _server_state: State<ConnectionServerState>,
) -> Response<Body> {
    forward_request_impl(path, None, headers, _server_state, false).await
}

pub async fn forward_request_post(
    Path(path): Path<String>,
    headers: HeaderMap,
    _server_state: State<ConnectionServerState>,
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

    forward_request_impl(path, body_data, headers, _server_state, false).await
}

async fn forward_request_impl(
    path: String,
    body: Option<serde_json::Value>,
    original_headers: HeaderMap,
    _server_state: State<ConnectionServerState>,
    is_streaming: bool,
) -> Response<Body> {
    let config = ProxyConfig::default();

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

    if config.payment_required && config.cost_sats > 0 {
        if let Ok(payment_token) = create_payment_token(config.cost_sats).await {
            req_builder = req_builder.header("X-Cashu", &payment_token);
        }
    }

    let start_time = start_onion_timing(&endpoint_url);

    match req_builder.send().await {
        Ok(resp) => {
            log_onion_timing(start_time, &endpoint_url, "proxy");
            let status = resp.status();
            let headers = resp.headers().clone();

            if let Some(change_token) = headers.get("X-Cashu") {
                if let Ok(token_str) = change_token.to_str() {
                    log::info!("Received change token: {}", token_str);
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

async fn create_payment_token(amount_sats: u64) -> Result<String, String> {
    log::info!("Creating payment token for {} sats", amount_sats);

    Ok(format!("dummy_token_{}", amount_sats))
}

pub async fn forward_routstr_proxy_request_get(
    Path(path): Path<String>,
    headers: HeaderMap,
    server_state: State<ConnectionServerState>,
) -> Response<Body> {
    let routstr_state = server_state
        .app_handle
        .state::<std::sync::Arc<tokio::sync::Mutex<RoutstrService>>>();
    forward_routstr_proxy_request_impl(path, None, headers, &routstr_state, false).await
}

pub async fn forward_routstr_proxy_request_post(
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

    let routstr_state = server_state
        .app_handle
        .state::<std::sync::Arc<tokio::sync::Mutex<RoutstrService>>>();
    forward_routstr_proxy_request_impl(path, body_data, headers, &routstr_state, false).await
}

async fn forward_routstr_proxy_request_impl(
    path: String,
    body: Option<serde_json::Value>,
    original_headers: HeaderMap,
    routstr_state: &std::sync::Arc<tokio::sync::Mutex<RoutstrService>>,
    is_streaming: bool,
) -> Response<Body> {
    let service = routstr_state.lock().await;

    if !service.use_proxy {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "message": "Proxy mode is not enabled",
                    "type": "configuration_error",
                    "code": "proxy_not_enabled"
                }
            })),
        )
            .into_response();
    }

    let target_url = match &service.target_service_url {
        Some(url) => url.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": {
                        "message": "No target service URL configured for proxy",
                        "type": "configuration_error",
                        "code": "no_target_service"
                    }
                })),
            )
                .into_response();
        }
    };

    let use_onion = service.use_onion;
    let payment_required = service.payment_required;
    let cost_sats = service.cost_per_request_sats;

    drop(service);

    let endpoint_url = construct_url_with_protocol(&target_url, &path);
    log::info!("Forwarding Routstr proxy request to: {}", endpoint_url);

    let timeout_secs = if is_streaming { 300 } else { 60 };
    let client = match create_onion_client(&endpoint_url, use_onion, Some(timeout_secs)) {
        Ok(client) => client,
        Err(e) => {
            log::error!("Failed to create HTTP client for Routstr proxy: {}", e);
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

    if payment_required && cost_sats > 0 {
        if let Ok(payment_token) = create_payment_token(cost_sats).await {
            req_builder = req_builder.header("X-Cashu", &payment_token);
        }
    }

    let start_time = start_onion_timing(&endpoint_url);

    match req_builder.send().await {
        Ok(resp) => {
            log_onion_timing(start_time, &endpoint_url, "routstr-proxy");
            let status = resp.status();
            let headers = resp.headers().clone();

            if let Some(change_token) = headers.get("X-Cashu") {
                if let Ok(token_str) = change_token.to_str() {
                    log::info!("Received change token from Routstr proxy: {}", token_str);
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
                    log::error!("Error creating Routstr proxy response: {}", e);
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
                    log::error!("Error reading Routstr proxy response body: {}", e);
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
            log::error!("Error forwarding Routstr proxy request: {}", error);

            let error_msg = get_onion_error_message(&error, &endpoint_url, "routstr-proxy");

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
