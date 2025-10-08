//! HTTP connection server for Nostr Wallet Connect
//!
//! This module provides a simple HTTP server that handles Nostr Wallet Connect
//! connection requests and exposes wallet information to connecting applications.

use axum::{
    extract::{Path, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tauri::Manager;
use tauri::{AppHandle, Emitter};
#[cfg(target_os = "macos")]
use tauri_nspanel::ManagerExt;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

/// Default port for the connection server
pub const DEFAULT_CONNECTION_PORT: u16 = 3737;

/// Request body for POST / endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectRequest {
    /// The Nostr Wallet Auth connection string
    pub nwa: String,
}

/// Nostr Wallet Auth request parsed from connection URI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrWalletAuthRequest {
    /// App's hex-encoded public key
    pub app_pubkey: String,
    /// Relay URLs where the app is listening
    pub relays: Vec<String>,
    /// Random secret identifier for this connection
    pub secret: String,
    /// Required commands that must be supported
    pub required_commands: Vec<String>,
    /// Optional commands
    pub optional_commands: Vec<String>,
    /// Budget in format "max_amount/period" (e.g., "10000/daily")
    pub budget: Option<String>,
    /// App's identity pubkey (optional)
    pub identity: Option<String>,
}

/// Pending connection request awaiting user approval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingConnectionRequest {
    /// Unique ID for this pending request
    pub request_id: String,
    /// The parsed NWA request (optional for standard NWC flow)
    pub nwa_request: Option<NostrWalletAuthRequest>,
    /// Timestamp when the request was received
    pub received_at: u64,
    /// The approved NWC connection URI (set after approval)
    pub nwc_uri: Option<String>,
    /// Whether this request has been approved
    pub approved: bool,
    /// Whether this request has been rejected
    pub rejected: bool,
}

/// State for managing pending connection requests
pub type PendingConnectionsState = Arc<Mutex<HashMap<String, PendingConnectionRequest>>>;

/// Server state that includes both TollGate and AppHandle
#[derive(Clone)]
pub struct ConnectionServerState {
    pub app_handle: AppHandle,
    pub pending_connections: PendingConnectionsState,
}

/// Response for approve/reject operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionResponse {
    pub success: bool,
    pub message: String,
}

/// Start the connection HTTP server
pub async fn start_connection_server(
    app_handle: AppHandle,
    pending_connections: PendingConnectionsState,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server_state = ConnectionServerState {
        app_handle,
        pending_connections,
    };

    // Configure CORS to allow any origin
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(tower_http::cors::Any);

    let app = Router::new()
        .route("/", get(get_wallet_info).post(post_wallet_connect))
        .route("/poll/:request_id", get(poll_connection_status))
        .route("/*path", get(crate::proxy::forward_request_get))
        .route("/*path", post(crate::proxy::forward_request_post))
        .route(
            "/routstr-proxy/*path",
            get(crate::proxy::forward_routstr_proxy_request_get),
        )
        .route(
            "/routstr-proxy/*path",
            post(crate::proxy::forward_routstr_proxy_request_post),
        )
        .layer(cors)
        .with_state(server_state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    log::info!("Attempting to bind connection server to {}", addr);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => {
            log::info!("Successfully bound connection server to {}", addr);
            l
        }
        Err(e) => {
            log::error!("Failed to bind connection server to {}: {}", addr, e);
            return Err(Box::new(e));
        }
    };

    log::info!("Connection server listening on http://{}", addr);
    log::info!("  GET  / - Create a new connection request (returns request_id)");
    log::info!("  GET  /poll/:request_id - Poll connection status and retrieve NWC URI");
    log::info!("  POST / - Connect via Nostr Wallet Auth (NWA)");

    tokio::spawn(async move {
        log::info!("Connection server task started, beginning to serve requests");
        if let Err(e) = axum::serve(listener, app).await {
            log::error!("Connection server encountered an error: {}", e);
        }
        log::warn!("Connection server task ended");
    });

    Ok(())
}

/// Handler for GET / - Creates a pending connection request
async fn get_wallet_info(State(state): State<ConnectionServerState>) -> Response {
    log::info!("Received GET request to create connection");

    // Generate a unique request ID
    let request_id = uuid::Uuid::new_v4().to_string();

    // Create pending connection request for standard NWC flow
    let pending_request = PendingConnectionRequest {
        request_id: request_id.clone(),
        nwa_request: None, // Standard NWC flow doesn't use NWA
        received_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        nwc_uri: None,
        approved: false,
        rejected: false,
    };

    // Store pending request
    {
        let mut pending_connections = state.pending_connections.lock().await;
        pending_connections.insert(request_id.clone(), pending_request.clone());
    }

    // Emit event to frontend to prompt user
    {
        #[cfg(target_os = "macos")]
        {
            let app_handle = state.app_handle.clone();
            let app_handle_for_closure = app_handle.clone();
            let _ = app_handle.run_on_main_thread(move || {
                if let Ok(panel) = app_handle_for_closure.get_webview_panel("main") {
                    if !panel.is_visible() {
                        panel.order_front_regardless();
                    }
                    panel.make_key_and_order_front(None);
                } else if let Some(window) = app_handle_for_closure.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            });
        }

        #[cfg(not(target_os = "macos"))]
        {
            if let Some(window) = state.app_handle.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    }

    if let Err(e) = state
        .app_handle
        .emit("nwc-connection-request", &pending_request)
    {
        log::error!("Failed to emit connection request event: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": "Failed to process connection request"
            })),
        )
            .into_response();
    }

    log::info!(
        "Created standard NWC connection request with ID: {}",
        request_id
    );

    Json(json!({
        "success": true,
        "request_id": request_id,
        "message": "Connection request created, awaiting user approval",
        "poll_url": format!("/poll/{}", request_id)
    }))
    .into_response()
}

/// Handler for POST /
async fn post_wallet_connect(
    State(state): State<ConnectionServerState>,
    Json(payload): Json<ConnectRequest>,
) -> Response {
    log::info!("Received POST connection request");
    log::debug!("Connection URI: {}", payload.nwa);

    // Parse the Nostr Wallet Auth URI
    match parse_nwa_uri(&payload.nwa) {
        Ok(nwa_request) => {
            log::info!("Successfully parsed connection request:");
            log::info!("  App pubkey: {}", nwa_request.app_pubkey);
            log::info!("  Relays: {:?}", nwa_request.relays);
            log::info!("  Secret: {}", nwa_request.secret);
            log::info!("  Required commands: {:?}", nwa_request.required_commands);
            log::info!("  Optional commands: {:?}", nwa_request.optional_commands);
            log::info!("  Budget: {:?}", nwa_request.budget);
            log::info!("  Identity: {:?}", nwa_request.identity);

            // Generate a unique request ID
            let request_id = uuid::Uuid::new_v4().to_string();

            // Create pending connection request
            let pending_request = PendingConnectionRequest {
                request_id: request_id.clone(),
                nwa_request: Some(nwa_request.clone()),
                received_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                nwc_uri: None,
                approved: false,
                rejected: false,
            };

            // Store pending request
            {
                let mut pending_connections = state.pending_connections.lock().await;
                pending_connections.insert(request_id.clone(), pending_request.clone());
            }

            // Emit event to frontend to prompt user
            // Ensure the app window is visible and focused so the user sees the prompt
            {
                #[cfg(target_os = "macos")]
                {
                    let app_handle = state.app_handle.clone();
                    let app_handle_for_closure = app_handle.clone();
                    let _ = app_handle.run_on_main_thread(move || {
                        if let Ok(panel) = app_handle_for_closure.get_webview_panel("main") {
                            if !panel.is_visible() {
                                panel.order_front_regardless();
                            }
                            panel.make_key_and_order_front(None);
                        } else if let Some(window) =
                            app_handle_for_closure.get_webview_window("main")
                        {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    });
                }

                #[cfg(not(target_os = "macos"))]
                {
                    if let Some(window) = state.app_handle.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
            if let Err(e) = state
                .app_handle
                .emit("nwc-connection-request", &pending_request)
            {
                log::error!("Failed to emit connection request event: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": "Failed to process connection request"
                    })),
                )
                    .into_response();
            }

            log::info!("Emitted connection request event with ID: {}", request_id);

            Json(json!({
                "success": true,
                "message": "Connection request received, awaiting user approval",
                "request_id": request_id
            }))
            .into_response()
        }
        Err(e) => {
            log::error!("Failed to parse connection URI: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "error": format!("Invalid connection URI: {}", e)
                })),
            )
                .into_response()
        }
    }
}

/// Handler for GET /poll/:request_id - Poll connection status
async fn poll_connection_status(
    State(state): State<ConnectionServerState>,
    Path(request_id): Path<String>,
) -> Response {
    log::debug!("Polling connection status for request: {}", request_id);

    let pending_connections = state.pending_connections.lock().await;

    if let Some(pending_request) = pending_connections.get(&request_id) {
        if pending_request.approved {
            if let Some(ref nwc_uri) = pending_request.nwc_uri {
                log::info!(
                    "Connection approved, returning NWC URI for request: {}",
                    request_id
                );
                Json(json!({
                    "status": "approved",
                    "nwc_uri": nwc_uri
                }))
                .into_response()
            } else {
                log::warn!(
                    "Connection approved but NWC URI not set for request: {}",
                    request_id
                );
                Json(json!({
                    "status": "approved",
                    "error": "NWC URI not available yet"
                }))
                .into_response()
            }
        } else if pending_request.rejected {
            log::info!("Connection rejected for request: {}", request_id);
            Json(json!({
                "status": "rejected",
                "message": "Connection request was rejected by user"
            }))
            .into_response()
        } else {
            log::debug!("Connection still pending for request: {}", request_id);
            Json(json!({
                "status": "pending",
                "message": "Waiting for user approval"
            }))
            .into_response()
        }
    } else {
        log::warn!("Connection request not found: {}", request_id);
        (
            StatusCode::NOT_FOUND,
            Json(json!({
                "status": "not_found",
                "error": "Connection request not found or expired"
            })),
        )
            .into_response()
    }
}

/// Parse a Nostr Wallet Auth URI
///
/// Format: nostr+walletauth://{pubkey}?relay={relay}&secret={secret}&request_methods={methods}&...
fn parse_nwa_uri(uri: &str) -> Result<NostrWalletAuthRequest, String> {
    // Check protocol
    if !uri.starts_with("nostr+walletauth://") {
        return Err("URI must start with 'nostr+walletauth://'".to_string());
    }

    // Remove protocol
    let uri = &uri["nostr+walletauth://".len()..];

    // Split pubkey and query string
    let parts: Vec<&str> = uri.splitn(2, '?').collect();
    if parts.len() != 2 {
        return Err("URI must contain query parameters".to_string());
    }

    let app_pubkey = parts[0].to_string();
    let query_string = parts[1];

    // Parse query parameters
    let params = parse_query_string(query_string);

    // Extract relays (can have multiple)
    let relays: Vec<String> = params
        .get("relay")
        .ok_or("Missing required parameter: relay")?
        .iter()
        .map(|s| urlencoding::decode(s).unwrap_or_default().to_string())
        .collect();

    // Extract secret (this is just an identifier from the app, not the actual connection secret)
    let secret = params
        .get("secret")
        .and_then(|v| v.first())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            // Generate a random identifier if not provided
            uuid::Uuid::new_v4().to_string()
        });

    // Extract request methods (handle both + and space separators)
    let required_commands: Vec<String> = params
        .get("request_methods")
        .and_then(|v| v.first())
        .map(|s| urlencoding::decode(s).unwrap_or_default().to_string())
        .map(|s| {
            // Split on both + and whitespace, then filter out empty strings
            s.split(&['+', ' ', '\t', '\n'][..])
                .filter(|c| !c.is_empty())
                .map(|c| c.to_string())
                .collect()
        })
        .unwrap_or_default();

    // Optional commands can still be parsed if present (for backwards compatibility)
    let optional_commands: Vec<String> = params
        .get("optional_commands")
        .and_then(|v| v.first())
        .map(|s| urlencoding::decode(s).unwrap_or_default().to_string())
        .map(|s| {
            // Split on both + and whitespace, then filter out empty strings
            s.split(&['+', ' ', '\t', '\n'][..])
                .filter(|c| !c.is_empty())
                .map(|c| c.to_string())
                .collect()
        })
        .unwrap_or_default();

    // Extract budget (optional)
    let budget = params
        .get("budget")
        .and_then(|v| v.first())
        .map(|s| urlencoding::decode(s).unwrap_or_default().to_string());

    // Extract identity (optional)
    let identity = params
        .get("identity")
        .and_then(|v| v.first())
        .map(|s| s.to_string());

    Ok(NostrWalletAuthRequest {
        app_pubkey,
        relays,
        secret,
        required_commands,
        optional_commands,
        budget,
        identity,
    })
}

/// Parse a query string into a map of key -> values
fn parse_query_string(query: &str) -> HashMap<String, Vec<String>> {
    let mut params: HashMap<String, Vec<String>> = HashMap::new();

    for pair in query.split('&') {
        let parts: Vec<&str> = pair.splitn(2, '=').collect();
        if parts.len() == 2 {
            let key = parts[0].to_string();
            let value = parts[1].to_string();
            params.entry(key).or_default().push(value);
        }
    }

    params
}

/// Tauri command to approve a pending connection request
#[tauri::command]
pub async fn nwc_approve_connection(
    request_id: String,
    pending_connections: tauri::State<'_, PendingConnectionsState>,
    nwc_state: tauri::State<'_, crate::NwcState>,
) -> Result<ConnectionResponse, String> {
    log::info!("Approving connection request: {}", request_id);

    let mut connections = pending_connections.lock().await;

    if let Some(mut pending_request) = connections.get(&request_id).cloned() {
        // Get the NWC service
        let nwc_lock = nwc_state.lock().await;
        let nwc = nwc_lock.as_ref().ok_or_else(|| {
            log::error!("NWC service not initialized");
            "NWC service not initialized".to_string()
        })?;

        // Check if this is NWA or standard NWC flow
        if let Some(nwa_request) = &pending_request.nwa_request {
            // NWA flow
            log::info!("  App pubkey: {}", nwa_request.app_pubkey);
            log::info!("  Required commands: {:?}", nwa_request.required_commands);
            log::info!("  Relays: {:?}", nwa_request.relays);

            // Parse budget if provided, or use default
            let budget = if let Some(budget_str) = &nwa_request.budget {
                parse_budget(budget_str).unwrap_or_else(|| {
                    log::warn!("Failed to parse budget '{}', using default", budget_str);
                    crate::nwc::ConnectionBudget::default()
                })
            } else {
                crate::nwc::ConnectionBudget::default()
            };

            // Create the NWA connection
            let connection = nwc
                .create_nwa_connection(
                    &nwa_request.app_pubkey,
                    nwa_request.secret.clone(), // App's secret for correlation
                    budget,
                )
                .await
                .map_err(|e| {
                    log::error!("Failed to create connection: {}", e);
                    format!("Failed to create connection: {}", e)
                })?;

            log::info!(
                "Created NWA connection: pubkey={}, budget={} msats",
                connection.keys.public_key(),
                connection.budget.total_budget_msats
            );

            // Broadcast the approval event to the app's relays
            nwc.broadcast_nwa_approval(
                &connection,
                nwa_request.relays.clone(),
                None, // TODO: Add lud16 support if needed
            )
            .await
            .map_err(|e| {
                log::error!("Failed to broadcast approval: {}", e);
                format!("Failed to broadcast approval: {}", e)
            })?;

            // Mark as approved and remove from pending
            connections.remove(&request_id);
            drop(connections);

            log::info!(
                "NWA connection approved successfully for request: {}",
                request_id
            );

            Ok(ConnectionResponse {
                success: true,
                message: "Connection approved and broadcasted successfully".to_string(),
            })
        } else {
            // Standard NWC flow - create a standard NWC connection and return URI
            log::info!("Creating standard NWC connection");

            // Create standard NWC connection
            let nwc_uri = nwc.create_standard_nwc_uri(false).await.map_err(|e| {
                log::error!("Failed to create NWC URI: {}", e);
                format!("Failed to create NWC URI: {}", e)
            })?;

            log::info!("Created standard NWC connection with URI");

            // Update pending request with the URI and mark as approved
            pending_request.nwc_uri = Some(nwc_uri.clone());
            pending_request.approved = true;
            connections.insert(request_id.clone(), pending_request);
            drop(connections);

            log::info!(
                "Standard NWC connection approved successfully for request: {}",
                request_id
            );

            Ok(ConnectionResponse {
                success: true,
                message: "Connection approved, NWC URI available for polling".to_string(),
            })
        }
    } else {
        drop(connections);
        log::warn!("Connection request not found: {}", request_id);
        Err(format!("Connection request not found: {}", request_id))
    }
}

/// Parse budget string in format "amount/period" (e.g., "10000/daily")
fn parse_budget(budget_str: &str) -> Option<crate::nwc::ConnectionBudget> {
    let parts: Vec<&str> = budget_str.split('/').collect();
    if parts.len() != 2 {
        return None;
    }

    let amount_sats = parts[0].parse::<u64>().ok()?;
    let amount_msats = amount_sats * 1000;

    let renewal_period = match parts[1].to_lowercase().as_str() {
        "daily" => crate::nwc::BudgetRenewalPeriod::Daily,
        "weekly" => crate::nwc::BudgetRenewalPeriod::Weekly,
        "monthly" => crate::nwc::BudgetRenewalPeriod::Monthly,
        "yearly" => crate::nwc::BudgetRenewalPeriod::Yearly,
        "never" => crate::nwc::BudgetRenewalPeriod::Never,
        _ => return None,
    };

    Some(crate::nwc::ConnectionBudget {
        renewal_period,
        renews_at: None,
        total_budget_msats: amount_msats,
        used_budget_msats: 0,
    })
}

/// Tauri command to reject a pending connection request
#[tauri::command]
pub async fn nwc_reject_connection(
    request_id: String,
    pending_connections: tauri::State<'_, PendingConnectionsState>,
) -> Result<ConnectionResponse, String> {
    log::info!("Rejecting connection request: {}", request_id);

    let mut connections = pending_connections.lock().await;

    if let Some(mut pending_request) = connections.get(&request_id).cloned() {
        // Mark as rejected instead of removing it
        pending_request.rejected = true;
        connections.insert(request_id.clone(), pending_request);
        drop(connections);

        log::info!("Connection request rejected: {}", request_id);
        Ok(ConnectionResponse {
            success: true,
            message: "Connection request rejected".to_string(),
        })
    } else {
        drop(connections);
        log::warn!("Connection request not found: {}", request_id);
        Err(format!("Connection request not found: {}", request_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_port() {
        assert_eq!(DEFAULT_CONNECTION_PORT, 3737);
    }

    #[test]
    fn test_parse_nwa_uri() {
        let uri = "nostr+walletauth://b889ff5b1513b641e2a139f661a661364979c5beee91842f8f0ef42ab558e9d4?relay=wss%3A%2F%2Frelay.damus.io&secret=b8a30fafa48d4795b6c0eec169a383de&request_methods=pay_invoice%2Bpay_keysend%2Bmake_invoice%2Blookup_invoice&optional_commands=list_transactions&budget=10000%2Fdaily";

        let result = parse_nwa_uri(uri).unwrap();

        assert_eq!(
            result.app_pubkey,
            "b889ff5b1513b641e2a139f661a661364979c5beee91842f8f0ef42ab558e9d4"
        );
        assert_eq!(result.relays, vec!["wss://relay.damus.io"]);
        assert_eq!(result.secret, "b8a30fafa48d4795b6c0eec169a383de");
        assert_eq!(
            result.required_commands,
            vec![
                "pay_invoice",
                "pay_keysend",
                "make_invoice",
                "lookup_invoice"
            ]
        );
        assert_eq!(result.optional_commands, vec!["list_transactions"]);
        assert_eq!(result.budget, Some("10000/daily".to_string()));
        assert_eq!(result.identity, None);
    }

    #[test]
    fn test_parse_nwa_uri_multiple_relays() {
        let uri = "nostr+walletauth://abc123?relay=wss%3A%2F%2Frelay1.com&relay=wss%3A%2F%2Frelay2.com&secret=test123&required_commands=pay_invoice";

        let result = parse_nwa_uri(uri).unwrap();

        assert_eq!(result.relays.len(), 2);
        assert_eq!(result.relays, vec!["wss://relay1.com", "wss://relay2.com"]);
    }

    #[test]
    fn test_parse_nwa_uri_invalid() {
        let uri = "invalid://test";
        assert!(parse_nwa_uri(uri).is_err());

        let uri = "nostr+walletauth://pubkey";
        assert!(parse_nwa_uri(uri).is_err());
    }
}
