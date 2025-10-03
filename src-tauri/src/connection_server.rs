//! HTTP connection server for Nostr Wallet Connect
//!
//! This module provides a simple HTTP server that handles Nostr Wallet Connect
//! connection requests and exposes wallet information to connecting applications.

use crate::TollGateState;
use axum::{
    extract::State,
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tauri::Manager;
#[cfg(target_os = "macos")]
use tauri_nspanel::ManagerExt;
use tokio::sync::Mutex;
use tower_http::cors::CorsLayer;

/// Default port for the connection server
pub const DEFAULT_CONNECTION_PORT: u16 = 3737;

/// NWC relay URL configuration
pub const NWC_RELAY_URL: &str = "ws://localhost:4869";

/// Supported NWC commands
pub const SUPPORTED_NWC_COMMANDS: &[&str] = &["get_balance", "make_invoice", "pay_invoice"];

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
    /// The parsed NWA request
    pub nwa_request: NostrWalletAuthRequest,
    /// Timestamp when the request was received
    pub received_at: u64,
}

/// State for managing pending connection requests
pub type PendingConnectionsState = Arc<Mutex<HashMap<String, PendingConnectionRequest>>>;

/// Server state that includes both TollGate and AppHandle
#[derive(Clone)]
pub struct ConnectionServerState {
    #[allow(dead_code)]
    pub tollgate_state: TollGateState,
    pub app_handle: AppHandle,
    pub pending_connections: PendingConnectionsState,
}

/// Response for approve/reject operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionResponse {
    pub success: bool,
    pub message: String,
}

/// Response for wallet info endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletInfoResponse {
    /// Relay URLs where the wallet is available
    pub relays: Vec<String>,
    /// Supported NWC commands
    pub supported_commands: Vec<String>,
}

/// Start the connection HTTP server
pub async fn start_connection_server(
    tollgate_state: Arc<Mutex<crate::tollgate::TollGateService>>,
    app_handle: AppHandle,
    pending_connections: PendingConnectionsState,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server_state = ConnectionServerState {
        tollgate_state,
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
    log::info!("  GET  / - Get wallet relays and supported commands");
    log::info!("  POST / - Connect via Nostr Wallet Auth");
    
    tokio::spawn(async move {
        log::info!("Connection server task started, beginning to serve requests");
        if let Err(e) = axum::serve(listener, app).await {
            log::error!("Connection server encountered an error: {}", e);
        }
        log::warn!("Connection server task ended");
    });

    Ok(())
}

/// Handler for GET /
async fn get_wallet_info(State(_state): State<ConnectionServerState>) -> Response {
    log::info!("Received GET request to / endpoint");
    
    let response = WalletInfoResponse {
        relays: vec![NWC_RELAY_URL.to_string()],
        supported_commands: SUPPORTED_NWC_COMMANDS.iter().map(|s| s.to_string()).collect(),
    };
    
    log::info!("Returning wallet info: relays={:?}, commands={:?}", 
               response.relays, response.supported_commands);
    
    Json(response).into_response()
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
                nwa_request: nwa_request.clone(),
                received_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
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
            if let Err(e) = state.app_handle.emit("nwc-connection-request", &pending_request) {
                log::error!("Failed to emit connection request event: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": "Failed to process connection request"
                    }))
                ).into_response();
            }
            
            log::info!("Emitted connection request event with ID: {}", request_id);
            
            Json(json!({
                "success": true,
                "message": "Connection request received, awaiting user approval",
                "request_id": request_id
            })).into_response()
        }
        Err(e) => {
            log::error!("Failed to parse connection URI: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "error": format!("Invalid connection URI: {}", e)
                }))
            ).into_response()
        }
    }
}

/// Parse a Nostr Wallet Auth URI
/// 
/// Format: nostr+walletauth://{pubkey}?relay={relay}&secret={secret}&required_commands={commands}&...
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
    let relays: Vec<String> = params.get("relay")
        .ok_or("Missing required parameter: relay")?
        .iter()
        .map(|s| urlencoding::decode(s).unwrap_or_default().to_string())
        .collect();
    
    // Extract secret
    let secret = params.get("secret")
        .and_then(|v| v.first())
        .ok_or("Missing required parameter: secret")?
        .to_string();
    
    // Extract required commands
    let required_commands: Vec<String> = params.get("required_commands")
        .and_then(|v| v.first())
        .map(|s| urlencoding::decode(s).unwrap_or_default().to_string())
        .map(|s| s.split_whitespace().map(|c| c.to_string()).collect())
        .unwrap_or_default();
    
    // Extract optional commands
    let optional_commands: Vec<String> = params.get("optional_commands")
        .and_then(|v| v.first())
        .map(|s| urlencoding::decode(s).unwrap_or_default().to_string())
        .map(|s| s.split_whitespace().map(|c| c.to_string()).collect())
        .unwrap_or_default();
    
    // Extract budget (optional)
    let budget = params.get("budget")
        .and_then(|v| v.first())
        .map(|s| urlencoding::decode(s).unwrap_or_default().to_string());
    
    // Extract identity (optional)
    let identity = params.get("identity")
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
            params.entry(key).or_insert_with(Vec::new).push(value);
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
    
    if let Some(pending_request) = connections.get(&request_id).cloned() {
        log::info!("  App pubkey: {}", pending_request.nwa_request.app_pubkey);
        log::info!("  Required commands: {:?}", pending_request.nwa_request.required_commands);
        log::info!("  Relays: {:?}", pending_request.nwa_request.relays);
        
        // Remove from pending connections immediately
        connections.remove(&request_id);
        drop(connections); // Release lock
        
        // Get the NWC service
        let nwc_lock = nwc_state.lock().await;
        let nwc = nwc_lock.as_ref()
            .ok_or_else(|| {
                log::error!("NWC service not initialized");
                "NWC service not initialized".to_string()
            })?;
        
        // Parse budget if provided, or use default
        let budget = if let Some(budget_str) = &pending_request.nwa_request.budget {
            parse_budget(budget_str).unwrap_or_else(|| {
                log::warn!("Failed to parse budget '{}', using default", budget_str);
                crate::nwc::ConnectionBudget::default()
            })
        } else {
            crate::nwc::ConnectionBudget::default()
        };
        
        // Create the NWA connection
        let connection = nwc.create_nwa_connection(
            &pending_request.nwa_request.app_pubkey,
            pending_request.nwa_request.secret.clone(),
            budget,
        ).await.map_err(|e| {
            log::error!("Failed to create connection: {}", e);
            format!("Failed to create connection: {}", e)
        })?;
        
        log::info!("Created connection: pubkey={}, budget={} msats", 
            connection.keys.public_key(), connection.budget.total_budget_msats);
        
        // Broadcast the approval event to the app's relays
        nwc.broadcast_nwa_approval(
            &connection,
            pending_request.nwa_request.relays.clone(),
            None, // TODO: Add lud16 support if needed
        ).await.map_err(|e| {
            log::error!("Failed to broadcast approval: {}", e);
            format!("Failed to broadcast approval: {}", e)
        })?;
        
        log::info!("Connection approved successfully for request: {}", request_id);
        
        Ok(ConnectionResponse {
            success: true,
            message: "Connection approved and broadcasted successfully".to_string(),
        })
    } else {
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
    
    if let Some(_) = connections.remove(&request_id) {
        log::info!("Connection request rejected and removed: {}", request_id);
        Ok(ConnectionResponse {
            success: true,
            message: "Connection request rejected".to_string(),
        })
    } else {
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
        let uri = "nostr+walletauth://b889ff5b1513b641e2a139f661a661364979c5beee91842f8f0ef42ab558e9d4?relay=wss%3A%2F%2Frelay.damus.io&secret=b8a30fafa48d4795b6c0eec169a383de&required_commands=pay_invoice%20pay_keysend%20make_invoice%20lookup_invoice&optional_commands=list_transactions&budget=10000%2Fdaily";
        
        let result = parse_nwa_uri(uri).unwrap();
        
        assert_eq!(result.app_pubkey, "b889ff5b1513b641e2a139f661a661364979c5beee91842f8f0ef42ab558e9d4");
        assert_eq!(result.relays, vec!["wss://relay.damus.io"]);
        assert_eq!(result.secret, "b8a30fafa48d4795b6c0eec169a383de");
        assert_eq!(result.required_commands, vec!["pay_invoice", "pay_keysend", "make_invoice", "lookup_invoice"]);
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
