//! Simple local Nostr relay for NWC communication
//! 
//! This module provides a lightweight, in-process Nostr relay that allows
//! the NWC service to communicate with external applications.

use nostr::{Event, Filter};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
// Note: AsyncReadExt and AsyncWriteExt are not used directly but are required for tokio-tungstenite
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, broadcast};

/// Default port for the local relay
pub const DEFAULT_RELAY_PORT: u16 = 4869;

/// Simple in-memory event store with broadcast support
#[derive(Clone)]
struct EventStore {
    events: Arc<RwLock<Vec<Event>>>,
    broadcast_tx: broadcast::Sender<Event>,
}

impl EventStore {
    fn new() -> Self {
        let (tx, _) = broadcast::channel(1000);
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
            broadcast_tx: tx,
        }
    }

    async fn add_event(&self, event: Event) {
        let mut events = self.events.write().await;
        // Check if event already exists
        if !events.iter().any(|e| e.id == event.id) {
            events.push(event.clone());
            // Broadcast to all subscribers
            let _ = self.broadcast_tx.send(event);
        }
    }

    async fn query_events(&self, filters: &[Filter]) -> Vec<Event> {
        let events = self.events.read().await;
        
        if filters.is_empty() {
            return events.clone();
        }

        events
            .iter()
            .filter(|event| filters.iter().any(|filter| filter.match_event(event)))
            .cloned()
            .collect()
    }
    
    fn subscribe_to_events(&self) -> broadcast::Receiver<Event> {
        self.broadcast_tx.subscribe()
    }
}

/// Start the local Nostr relay server
pub async fn start_relay_server(port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    
    log::info!("Attempting to bind local Nostr relay to {}", addr);
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => {
            log::info!("Successfully bound to {}", addr);
            l
        }
        Err(e) => {
            log::error!("Failed to bind relay server to {}: {}", addr, e);
            return Err(Box::new(e));
        }
    };
    
    log::info!("✓ Local Nostr relay listening on ws://{}", addr);
    
    let event_store = EventStore::new();
    
    tokio::spawn(async move {
        log::info!("Relay server task started, waiting for connections...");
        let mut connection_count = 0;
        loop {
            match listener.accept().await {
                Ok((stream, peer_addr)) => {
                    connection_count += 1;
                    log::info!("✓ New relay connection #{} from: {}", connection_count, peer_addr);
                    let store = event_store.clone();
                    let conn_id = connection_count;
                    tokio::spawn(async move {
                        log::debug!("[Conn #{}] Starting WebSocket handshake", conn_id);
                        if let Err(e) = handle_connection(stream, store).await {
                            log::error!("[Conn #{}] Relay connection error: {}", conn_id, e);
                        } else {
                            log::info!("[Conn #{}] Connection closed gracefully", conn_id);
                        }
                    });
                }
                Err(e) => {
                    log::error!("Failed to accept relay connection: {}", e);
                }
            }
        }
    });
    
    Ok(())
}

/// Handle a single WebSocket connection
async fn handle_connection(
    stream: TcpStream,
    event_store: EventStore,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tokio_tungstenite::{accept_async, tungstenite::Message};
    use futures_util::{SinkExt, StreamExt};
    
    log::debug!("Accepting WebSocket connection...");
    let ws_stream = match accept_async(stream).await {
        Ok(ws) => {
            log::info!("✓ WebSocket handshake successful");
            ws
        }
        Err(e) => {
            log::error!("✗ WebSocket handshake failed: {}", e);
            return Err(Box::new(e));
        }
    };
    
    let (mut write, mut read) = ws_stream.split();
    log::debug!("WebSocket connection established, ready to receive messages");
    
    // Track active subscriptions for this connection
    let subscriptions: Arc<RwLock<HashMap<String, Vec<Filter>>>> = 
        Arc::new(RwLock::new(HashMap::new()));
    
    // Subscribe to broadcast events
    let mut event_rx = event_store.subscribe_to_events();
    
    loop {
        tokio::select! {
            // Handle incoming WebSocket messages
            msg = read.next() => {
                let msg = match msg {
                    Some(Ok(m)) => m,
                    Some(Err(e)) => {
                        log::error!("WebSocket error: {}", e);
                        break;
                    }
                    None => break,
                };
                
                if let Err(e) = handle_message(msg, &mut write, &event_store, &subscriptions).await {
                    log::error!("Error handling message: {}", e);
                    break;
                }
            }
            // Handle broadcasted events
            event = event_rx.recv() => {
                match event {
                    Ok(event) => {
                        // Check if this event matches any active subscriptions
                        let subs = subscriptions.read().await;
                        for (sub_id, filters) in subs.iter() {
                            for filter in filters.iter() {
                                if filter.match_event(&event) {
                                    log::info!("✅ Broadcasting event {} to subscription '{}'", event.id, sub_id);
                                    let event_msg = serde_json::json!([
                                        "EVENT",
                                        sub_id,
                                        event
                                    ]);
                                    if let Ok(event_str) = serde_json::to_string(&event_msg) {
                                        let _ = write.send(Message::Text(event_str)).await;
                                    }
                                    break; // Only send once per subscription
                                }
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        log::warn!("Broadcast receiver lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        log::warn!("Broadcast channel closed");
                        break;
                    }
                }
            }
        }
    }
    
    Ok(())
}

/// Handle a single WebSocket message
async fn handle_message(
    msg: tokio_tungstenite::tungstenite::Message,
    write: &mut futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<TcpStream>, tokio_tungstenite::tungstenite::Message>,
    event_store: &EventStore,
    subscriptions: &Arc<RwLock<HashMap<String, Vec<Filter>>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tokio_tungstenite::tungstenite::Message;
    use futures_util::SinkExt;
    
    match msg {
        Message::Text(text) => {
            log::debug!("Received relay message: {}", text);
            
            // Parse Nostr message
            if let Ok(message) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(msg_type) = message.get(0).and_then(|v| v.as_str()) {
                    match msg_type {
                        "EVENT" => {
                            // ["EVENT", <event JSON>]
                            if let Some(event_json) = message.get(1) {
                                match serde_json::from_value::<Event>(event_json.clone()) {
                                    Ok(event) => {
                                        // Store event (will broadcast to all connections)
                                        event_store.add_event(event.clone()).await;
                                        
                                        // Send OK response
                                        let ok_msg = serde_json::json!([
                                            "OK",
                                            event.id.to_string(),
                                            true,
                                            ""
                                        ]);
                                        if let Ok(ok_str) = serde_json::to_string(&ok_msg) {
                                            let _ = write.send(Message::Text(ok_str)).await;
                                        }
                                        
                                        log::info!(
                                            "📦 Stored event: id={}, kind={}, author={}, p_tags={:?}",
                                            event.id,
                                            event.kind,
                                            event.pubkey,
                                            event.tags.iter()
                                                .filter(|t| t.as_slice().first().map(|s| s == "p").unwrap_or(false))
                                                .filter_map(|t| t.as_slice().get(1))
                                                .collect::<Vec<_>>()
                                        );
                                    }
                                    Err(e) => {
                                        log::warn!("Failed to parse event: {}", e);
                                    }
                                }
                            }
                        }
                        "REQ" => {
                            // ["REQ", <subscription_id>, <filter1>, <filter2>, ...]
                            if let Some(sub_id) = message.get(1).and_then(|v| v.as_str()) {
                                let mut filters = Vec::new();
                                
                                // Parse all filters
                                for i in 2..message.as_array().map(|a| a.len()).unwrap_or(0) {
                                    if let Some(filter_json) = message.get(i) {
                                        match serde_json::from_value::<Filter>(filter_json.clone()) {
                                            Ok(filter) => {
                                                // Log detailed filter info
                                                log::info!(
                                                    "Filter #{}: kinds={:?}, authors={:?}, #p tags={:?}",
                                                    i - 1,
                                                    filter.kinds,
                                                    filter.authors.as_ref().map(|a| a.iter().map(|pk| pk.to_string()).collect::<Vec<_>>()),
                                                    filter.generic_tags.iter()
                                                        .find(|(tag_name, _)| tag_name.to_string() == "p")
                                                        .map(|(_, values)| values.clone())
                                                );
                                                filters.push(filter);
                                            },
                                            Err(e) => log::warn!("Failed to parse filter: {}", e),
                                        }
                                    }
                                }
                                
                                log::info!("📥 New subscription: {} with {} filters", sub_id, filters.len());
                                
                                // Store subscription
                                {
                                    let mut subs = subscriptions.write().await;
                                    subs.insert(sub_id.to_string(), filters.clone());
                                }
                                
                                // Send matching events
                                let events = event_store.query_events(&filters).await;
                                log::info!("Found {} matching events for subscription {}", events.len(), sub_id);
                                
                                for event in events {
                                    let event_msg = serde_json::json!([
                                        "EVENT",
                                        sub_id,
                                        event
                                    ]);
                                    if let Ok(event_str) = serde_json::to_string(&event_msg) {
                                        let _ = write.send(Message::Text(event_str)).await;
                                    }
                                }
                                
                                // Send EOSE
                                let eose_msg = serde_json::json!(["EOSE", sub_id]);
                                if let Ok(eose_str) = serde_json::to_string(&eose_msg) {
                                    let _ = write.send(Message::Text(eose_str)).await;
                                }
                            }
                        }
                        "CLOSE" => {
                            // ["CLOSE", <subscription_id>]
                            if let Some(sub_id) = message.get(1).and_then(|v| v.as_str()) {
                                let mut subs = subscriptions.write().await;
                                subs.remove(sub_id);
                                log::debug!("Closed subscription: {}", sub_id);
                            }
                        }
                        _ => {
                            log::debug!("Unknown message type: {}", msg_type);
                        }
                    }
                }
            }
        }
        Message::Close(_) => {
            log::debug!("Client closed connection");
            return Err("Connection closed".into());
        }
        Message::Ping(data) => {
            let _ = write.send(Message::Pong(data)).await;
        }
        _ => {}
    }
    
    Ok(())
}

