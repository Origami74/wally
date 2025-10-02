//! HTTP server to expose wallet's Nostr public key
//!
//! This module provides a simple HTTP server that exposes the wallet's
//! Nostr public key in hex format on a dedicated port, allowing other services
//! on the same device to easily retrieve it.

use crate::TollGateState;
use axum::{
    extract::State,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Default port for the npub server
pub const DEFAULT_NPUB_PORT: u16 = 3737;

/// Start the npub HTTP server
pub async fn start_npub_server(
    state: Arc<Mutex<crate::tollgate::TollGateService>>,
    port: u16,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = Router::new()
        .route("/npub", get(get_npub))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    
    log::info!("Attempting to bind npub server to {}", addr);
    
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => {
            log::info!("Successfully bound npub server to {}", addr);
            l
        }
        Err(e) => {
            log::error!("Failed to bind npub server to {}: {}", addr, e);
            return Err(Box::new(e));
        }
    };
    
    log::info!("Npub server listening on http://{}/npub", addr);
    
    tokio::spawn(async move {
        log::info!("Npub server task started, beginning to serve requests");
        if let Err(e) = axum::serve(listener, app).await {
            log::error!("Npub server encountered an error: {}", e);
        }
        log::warn!("Npub server task ended");
    });

    Ok(())
}

/// Handler for GET /npub
async fn get_npub(State(state): State<TollGateState>) -> Response {
    log::info!("Received request to /npub endpoint");
    let service = state.lock().await;
    
    let pubkey_hex = service.get_pubkey_hex().await;
    log::info!("Successfully retrieved public key: {}", pubkey_hex);
    Json(json!({
        "pubkey": pubkey_hex,
        "success": true
    })).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_port() {
        assert_eq!(DEFAULT_NPUB_PORT, 3737);
    }
}

