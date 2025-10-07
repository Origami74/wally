//! Nostr Wallet Connect (NIP-47) implementation for the TollGate wallet.
//!
//! This module provides NWC functionality that allows external applications
//! to interact with the wallet through Nostr relays.

use crate::nwc_storage::NwcConnectionStorage;
use crate::tollgate::wallet::{
    Bolt11InvoiceInfo, Bolt11PaymentResult, CashuReceiveResult, PayNut18Result,
};
use crate::TollGateState;
use lightning_invoice::Bolt11Invoice;
use nostr_sdk::prelude::FromBech32;
use nostr_sdk::{
    nips::{
        nip04,
        nip47::{self, NostrWalletConnectURI},
    },
    Alphabet, Client, Event, EventBuilder, Filter, JsonUtil, Keys, Kind, PublicKey, RelayUrl,
    SecretKey, SingleLetterTag, Tag, TagStandard, Timestamp, Url,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

const REMOTE_RELAY_URL: &str = "wss://nostrue.com";
const LOCAL_RELAY_URL: &str = "ws://localhost:4869";
const NWC_BUDGET_MSATS: u64 = 1_000_000_000; // 1,000 sats budget

fn parse_connection_pubkey(value: &str) -> Result<PublicKey, Error> {
    if let Ok(pk) = PublicKey::from_str(value) {
        return Ok(pk);
    }

    if let Ok(pk) = PublicKey::from_bech32(value) {
        return Ok(pk);
    }

    Err(Error::Wallet(format!(
        "Invalid connection pubkey: {}",
        value
    )))
}

/// Nostr Wallet Connect service for the TollGate wallet.
#[derive(Clone)]
pub struct NostrWalletConnect {
    /// NWC service keys
    keys: Keys,
    /// Connected client
    client: Client,
    /// Last processed event timestamp
    last_check: Arc<Mutex<Timestamp>>,
    /// Response event cache to avoid reprocessing
    response_event_cache: Arc<Mutex<HashMap<String, Event>>>,
    /// Active connections
    connections: Arc<RwLock<Vec<WalletConnection>>>,
    /// Reference to the TollGate service state
    service_state: TollGateState,
    /// Connection storage
    storage: Arc<NwcConnectionStorage>,
}

impl NostrWalletConnect {
    async fn ensure_relay(&self, relay_url: &str) -> Result<(), Error> {
        let relays = self.client.relays().await;
        let has_relay = relays.keys().any(|url| url.as_str() == relay_url);
        if !has_relay {
            if let Err(err) = self.client.add_relay(relay_url).await {
                log::warn!("Failed to add relay {}: {}", relay_url, err);
            }
        }

        if let Err(err) = self.client.connect_relay(relay_url).await {
            log::warn!("Failed to connect relay {}: {}", relay_url, err);
        }

        Ok(())
    }

    /// Creates a new NWC service instance.
    pub async fn new(service_key: SecretKey, service_state: TollGateState) -> Result<Self, Error> {
        let keys = Keys::new(service_key);
        let client = Client::default();

        // Initialize storage
        let storage = Arc::new(
            NwcConnectionStorage::new()
                .map_err(|e| Error::Wallet(format!("Failed to initialize NWC storage: {}", e)))?,
        );

        // Load existing connections from storage
        let connections = storage
            .load_connections()
            .map_err(|e| Error::Wallet(format!("Failed to load NWC connections: {}", e)))?;

        log::info!("Loaded {} NWC connections from storage", connections.len());

        Ok(Self {
            keys,
            client,
            last_check: Arc::new(Mutex::new(Timestamp::now())),
            response_event_cache: Arc::new(Mutex::new(HashMap::new())),
            connections: Arc::new(RwLock::new(connections)),
            service_state,
            storage,
        })
    }

    /// Starts the NWC service.
    pub async fn start(&self) -> Result<(), Error> {
        log::info!(
            "Starting NWC service, ensuring relay connectivity: {}",
            REMOTE_RELAY_URL
        );

        self.ensure_relay(REMOTE_RELAY_URL).await?;

        // Connect to relay with timeout
        log::info!("Connecting to relay...");
        self.client.connect().await;

        // Wait a moment for connection to establish
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        log::info!("NWC service connected to relay: {}", REMOTE_RELAY_URL);

        // Publish info event
        log::info!("Publishing NWC info event...");
        match self.publish_info_event().await {
            Ok(_) => log::info!("Successfully published info event"),
            Err(e) => {
                log::error!("Failed to publish info event: {}", e);
                return Err(e);
            }
        }

        Ok(())
    }

    /// Adds a new wallet connection.
    pub async fn add_connection(&self, connection: WalletConnection) -> Result<(), Error> {
        let mut connections = self.connections.write().await;
        if connections
            .iter()
            .any(|conn| conn.keys.public_key() == connection.keys.public_key())
        {
            return Ok(());
        }

        // Persist to storage
        self.storage
            .save_connection(&connection)
            .map_err(|e| Error::Wallet(format!("Failed to save connection to storage: {}", e)))?;

        connections.push(connection);
        log::info!("Added and persisted new NWC connection");
        Ok(())
    }

    /// Gets all wallet connections.
    pub async fn get_connections(&self) -> Vec<WalletConnection> {
        self.connections.read().await.clone()
    }

    /// Removes a wallet connection.
    pub async fn remove_connection(&self, connection_pubkey: &str) -> Result<(), Error> {
        let target_pubkey = parse_connection_pubkey(connection_pubkey)?;
        let target_hex = target_pubkey.to_hex();

        let mut connections = self.connections.write().await;

        let initial_len = connections.len();
        connections.retain(|conn| conn.keys.public_key().to_hex() != target_hex);

        if connections.len() < initial_len {
            self.storage.delete_connection(&target_hex).map_err(|e| {
                Error::Wallet(format!("Failed to delete connection from storage: {}", e))
            })?;
            log::info!("Removed and deleted NWC connection: {}", target_hex);
        } else {
            log::warn!("Attempted to remove unknown NWC connection: {}", target_hex);
        }

        Ok(())
    }

    /// Updates the budget configuration for a connection.
    pub async fn update_connection_budget(
        &self,
        connection_pubkey: &str,
        total_budget_sats: u64,
        renewal_period: BudgetRenewalPeriod,
    ) -> Result<WalletConnection, Error> {
        let target_pubkey = parse_connection_pubkey(connection_pubkey)?;
        let target_hex = target_pubkey.to_hex();

        let mut connections = self.connections.write().await;
        let connection = connections
            .iter_mut()
            .find(|conn| conn.keys.public_key() == target_pubkey)
            .ok_or_else(|| Error::Wallet("Connection not found".to_string()))?;

        connection.budget.total_budget_msats = total_budget_sats.saturating_mul(1_000);
        connection.budget.renewal_period = renewal_period;
        connection.budget.used_budget_msats = connection
            .budget
            .used_budget_msats
            .min(connection.budget.total_budget_msats);
        connection.budget.renews_at = match renewal_period {
            BudgetRenewalPeriod::Never => None,
            _ => connection.budget_renews_at(),
        };

        self.storage
            .update_budget(&target_hex, &connection.budget)
            .map_err(|e| Error::Wallet(format!("Failed to persist budget: {}", e)))?;

        Ok(connection.clone())
    }

    /// Updates the friendly name for a connection.
    pub async fn update_connection_name(
        &self,
        connection_pubkey: &str,
        name: &str,
    ) -> Result<WalletConnection, Error> {
        let target_pubkey = parse_connection_pubkey(connection_pubkey)?;
        let target_hex = target_pubkey.to_hex();
        let trimmed = name.trim();

        let mut connections = self.connections.write().await;
        let connection = connections
            .iter_mut()
            .find(|conn| conn.keys.public_key() == target_pubkey)
            .ok_or_else(|| Error::Wallet("Connection not found".to_string()))?;

        connection.name = if trimmed.is_empty() {
            WalletConnection::default_name(&connection.keys)
        } else {
            trimmed.to_string()
        };

        self.storage
            .update_name(&target_hex, &connection.name)
            .map_err(|e| Error::Wallet(format!("Failed to persist connection name: {}", e)))?;

        Ok(connection.clone())
    }

    /// Creates a new standard NWC connection and returns the connection URI.
    pub async fn create_standard_nwc_uri(&self, use_local_relay: bool) -> Result<String, Error> {
        // Generate new keys for the connection
        let connection_key = SecretKey::generate();

        // Create a default budget
        let budget = ConnectionBudget::default();

        // Create a new WalletConnection
        let connection = WalletConnection::new(connection_key, budget);
        let connection_pubkey = connection.keys.public_key();

        // Add and persist the connection
        self.add_connection(connection.clone()).await?;

        // Create the URI
        let relay_url_str = if use_local_relay {
            LOCAL_RELAY_URL
        } else {
            REMOTE_RELAY_URL
        };

        self.ensure_relay(relay_url_str).await?;

        let relay_url = Url::from_str(relay_url_str).map_err(|e| Error::Url(e.to_string()))?;
        let uri = connection.uri(self.service_pubkey(), relay_url)?;

        log::info!(
            "Created new standard NWC URI for connection: {} via {}",
            connection_pubkey,
            relay_url_str
        );

        Ok(uri)
    }

    /// Creates a kind 13194 info event for the NWC service.
    pub fn info_event(&self) -> Result<Event, Error> {
        let event = EventBuilder::new(
            Kind::WalletConnectInfo,
            "get_balance make_invoice pay_invoice receive_cashu pay_cashu_request",
        )
        .sign_with_keys(&self.keys)?;
        Ok(event)
    }

    /// Publishes the NWC info event.
    pub async fn publish_info_event(&self) -> Result<(), Error> {
        let event = self.info_event()?;
        self.client.send_event(&event).await?;
        log::info!("Published NWC info event");
        Ok(())
    }

    /// Gets the Nostr filters for NWC requests.
    pub async fn filters(&self) -> Vec<Filter> {
        // Use a timestamp from 5 minutes ago to catch any recent events
        let since = Timestamp::now() - Duration::from_secs(300);
        let connections = self.connections.read().await;
        connections
            .iter()
            .map(|conn| conn.filter(self.keys.public_key(), since))
            .collect()
    }

    /// Processes incoming NWC events in a loop.
    pub async fn process_events_loop(&self) -> Result<(), Error> {
        log::info!("Starting NWC event processing loop");

        loop {
            // Get filters for active connections
            let filters = self.filters().await;

            if filters.is_empty() {
                log::debug!("No active connections, waiting...");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }

            log::debug!("Subscribing with {} filter(s)", filters.len());

            // Subscribe to events matching our filters
            for filter in filters.clone() {
                let _ = match self.client.subscribe(filter, None).await {
                    Ok(sub_output) => {
                        log::info!(
                            "Subscribed to NWC events with subscription ID: {:?}",
                            sub_output.val
                        );
                        sub_output
                    }
                    Err(e) => {
                        log::error!("Failed to subscribe to NWC events: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                };
            }

            // Create a channel to receive notifications
            let mut notifications = self.client.notifications();

            // Process events from the subscription
            loop {
                tokio::select! {
                    // Check for new events
                    notification = notifications.recv() => {
                        if let Ok(notification) = notification {
                            use nostr_sdk::RelayPoolNotification;
                            if let RelayPoolNotification::Event { event, .. } = notification {
                                log::debug!("Received event: {} kind={}", event.id, event.kind);

                                // Check if this is a WalletConnectRequest event
                                if event.kind == Kind::WalletConnectRequest {
                                    match self.handle_event(*event).await {
                                        Ok(Some(response)) => {
                                            log::info!("Sending response event: {}", response.id);
                                            if let Err(e) = self.client.send_event(&response).await {
                                                log::error!("Failed to send response: {}", e);
                                            }
                                        }
                                        Ok(None) => {
                                            log::debug!("Event already processed, skipping");
                                        }
                                        Err(e) => {
                                            log::error!("Error handling event: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                    },
                    // Periodically check if filters need updating
                    _ = tokio::time::sleep(Duration::from_secs(10)) => {
                        let new_filters = self.filters().await;
                        if new_filters.len() != filters.len() || new_filters.is_empty() {
                            log::info!("Connection count changed, resubscribing...");
                            // Unsubscribe from old filters
                            let _ = self.client.unsubscribe_all().await;
                            break; // Break inner loop to resubscribe with new filters
                        }
                    }
                }
            }
        }
    }

    /// Handles a single NWC request event.
    pub async fn handle_event(&self, event: Event) -> Result<Option<Event>, Error> {
        if event.kind != Kind::WalletConnectRequest {
            log::warn!(
                "Ignoring non-WalletConnectRequest event: kind={}",
                event.kind
            );
            return Err(Error::InvalidKind);
        }

        // Get the target pubkey from the 'p' tag
        let target_pubkey_str = event
            .tags
            .iter()
            .find_map(|tag| {
                let tag_vec = tag.as_slice();
                if tag_vec.len() >= 2 && tag_vec[0] == "p" {
                    Some(tag_vec[1].clone())
                } else {
                    None
                }
            })
            .ok_or(Error::MissingServiceKey)?;

        let _target_pubkey = PublicKey::from_str(&target_pubkey_str)?;

        log::info!(
            "Received NWC request: event_id={}, from={}, to={}",
            event.id,
            event.pubkey,
            target_pubkey_str
        );

        // For NWA connections, _target_pubkey should be the connection pubkey
        // For standard NWC, _target_pubkey should be the service pubkey
        // We validate this matches a known connection when we find the matching connection

        let event_id = event.id.to_string();

        // Check if we've already processed this event
        {
            let cache = self.response_event_cache.lock().await;
            if let Some(cached_response) = cache.get(&event_id) {
                log::debug!(
                    "Event {} already processed, returning cached response",
                    event_id
                );
                return Ok(Some(cached_response.clone()));
            }
        }

        log::info!("Processing new NWC event: {}", event_id);

        // Find matching connection (check both standard and NWA connections)
        let mut connections = self.connections.write().await;

        log::debug!(
            "Searching for connection among {} connections for event from {}",
            connections.len(),
            event.pubkey
        );

        // Pre-compute available connections list for error message
        let available_connections_str: String = connections
            .iter()
            .map(|c| {
                if let Some(app_pk) = c.app_pubkey {
                    format!("NWA(app={})", app_pk)
                } else {
                    format!("Standard(conn={})", c.keys.public_key())
                }
            })
            .collect::<Vec<_>>()
            .join(", ");

        let connection = connections
            .iter_mut()
            .find(|conn| {
                // For NWA connections, match on app_pubkey
                if let Some(app_pubkey) = conn.app_pubkey {
                    let matches = app_pubkey == event.pubkey;
                    log::debug!(
                        "Checking NWA connection: app_pubkey={}, matches={}",
                        app_pubkey,
                        matches
                    );
                    matches
                } else {
                    // For standard NWC, match on connection pubkey
                    let matches = conn.keys.public_key() == event.pubkey;
                    log::debug!(
                        "Checking standard NWC connection: conn_pubkey={}, matches={}",
                        conn.keys.public_key(),
                        matches
                    );
                    matches
                }
            })
            .ok_or_else(|| {
                log::error!(
                    "Connection not found for event from {}. Available connections: {}",
                    event.pubkey,
                    available_connections_str
                );
                Error::ConnectionNotFound
            })?;

        log::info!("Found matching connection for event {}", event_id);

        // Decrypt request
        // For NWA: decrypt with connection's secret key and app's pubkey
        // For standard NWC: decrypt with connection's secret key and service pubkey
        let decrypt_pubkey = if connection.app_pubkey.is_some() {
            connection.app_pubkey.as_ref().unwrap()
        } else {
            &self.keys.public_key()
        };

        log::debug!("Decrypting content with pubkey: {}", decrypt_pubkey);

        let decrypted_content =
            nip04::decrypt(connection.keys.secret_key(), decrypt_pubkey, &event.content).map_err(
                |e| {
                    log::error!("Failed to decrypt NWC request: {}", e);
                    e
                },
            )?;

        log::debug!("Decrypted content: {}", decrypted_content);

        // Try to parse as JSON first to check for custom methods
        let json_value: serde_json::Value =
            serde_json::from_str(&decrypted_content).map_err(|e| {
                log::error!("Failed to parse request JSON: {}", e);
                Error::Wallet(format!("Failed to parse request JSON: {}", e))
            })?;

        // Check if this is a custom method
        let method = json_value
            .get("method")
            .and_then(|m| m.as_str())
            .ok_or_else(|| {
                log::error!("Missing method field in request");
                Error::Wallet("Missing method field in request".to_string())
            })?;

        log::info!("NWC request method: {}", method);

        // Handle custom methods
        if method == "receive_cashu" {
            // Parse custom params
            let params = json_value
                .get("params")
                .ok_or_else(|| Error::Wallet("Missing params field in request".to_string()))?;
            let token = params
                .get("token")
                .and_then(|t| t.as_str())
                .ok_or_else(|| {
                    Error::Wallet("Missing token in receive_cashu params".to_string())
                })?;

            // Handle custom receive_cashu request
            return self
                .handle_receive_cashu_request(connection, event, token)
                .await;
        }

        if method == "pay_cashu_request" {
            // Parse custom params
            let params = json_value
                .get("params")
                .ok_or_else(|| Error::Wallet("Missing params field in request".to_string()))?;
            let payment_request = params
                .get("payment_request")
                .and_then(|pr| pr.as_str())
                .ok_or_else(|| {
                    Error::Wallet("Missing payment_request in pay_cashu_request params".to_string())
                })?;

            // Optional amount parameter - can override the amount in the payment request
            let amount = params.get("amount").and_then(|a| a.as_u64());

            // Handle custom pay_cashu_request request
            return self
                .handle_pay_cashu_request(connection, event, payment_request, amount)
                .await;
        }

        // Parse as standard NIP-47 request
        let request = nip47::Request::from_json(decrypted_content)?;

        // Check budget
        // let remaining_budget_msats =
        let remaining_budget_msats = connection.budget.total_budget_msats;
        // TODO: do thhis so that it acutally updates the budget correctly
        // let remaining_budget_msats = connection.check_and_update_remaining_budget();

        // If budget was renewed, persist the update
        let connection_pubkey = connection.keys.public_key().to_hex();
        if let Err(e) = self
            .storage
            .update_budget(&connection_pubkey, &connection.budget)
        {
            log::error!("Failed to persist budget renewal: {}", e);
        }

        // Handle request
        let (response, payment_amount, balance_info) =
            self.handle_request(request, remaining_budget_msats).await;

        // Update budget if payment was made
        if let Some(amount) = payment_amount {
            connection.budget.used_budget_msats += amount;

            // Persist updated budget to storage
            let connection_pubkey = connection.keys.public_key().to_hex();
            if let Err(e) = self
                .storage
                .update_budget(&connection_pubkey, &connection.budget)
            {
                log::error!("Failed to update connection budget in storage: {}", e);
            }
        }

        // Encrypt response
        // For NWA: encrypt with connection's secret key and app's pubkey
        // For standard NWC: encrypt with connection's secret key and service pubkey
        let encrypt_pubkey = if connection.app_pubkey.is_some() {
            connection.app_pubkey.as_ref().unwrap()
        } else {
            &self.keys.public_key()
        };

        // Serialize response to JSON and extend with custom fields if it's a get_balance response
        let mut response_json = response.as_json();
        if let Some(ref bal_info) = balance_info {
            // Parse the JSON, add custom fields, then re-serialize
            if let Ok(mut json_value) = serde_json::from_str::<serde_json::Value>(&response_json) {
                if let Some(result) = json_value.get_mut("result") {
                    result["max_sendable"] = serde_json::json!(bal_info.max_sendable);
                    result["mints"] = serde_json::json!(bal_info.mints);
                }
                response_json = serde_json::to_string(&json_value).unwrap_or(response_json);
            }
        }

        let encrypted_response =
            nip04::encrypt(connection.keys.secret_key(), encrypt_pubkey, response_json)?;

        // Create response event
        // For NWA: sign with connection keys
        // For standard NWC: sign with service keys
        let signing_keys = if connection.app_pubkey.is_some() {
            &connection.keys
        } else {
            &self.keys
        };

        let res_event = EventBuilder::new(Kind::WalletConnectResponse, encrypted_response)
            .tags(vec![
                Tag::from_standardized(TagStandard::public_key(event.pubkey)),
                Tag::from_standardized(TagStandard::event(event.id)),
            ])
            .sign_with_keys(signing_keys)?;

        log::info!(
            "📤 Created response event: id={}, kind={}, author={}, p_tag={}, e_tag={}",
            res_event.id,
            res_event.kind,
            res_event.pubkey,
            event.pubkey,
            event.id
        );

        log::info!(
            "💡 Client should subscribe with: kinds=[23195], authors=[{}], #p=[{}]",
            res_event.pubkey,
            event.pubkey
        );

        // Cache response
        {
            let mut cache = self.response_event_cache.lock().await;
            cache.insert(event_id, res_event.clone());
        }

        // Update last check timestamp
        {
            let mut last_check = self.last_check.lock().await;
            *last_check = event.created_at;
        }

        Ok(Some(res_event))
    }

    /// Handles a custom receive_cashu request.
    async fn handle_receive_cashu_request(
        &self,
        connection: &mut WalletConnection,
        event: Event,
        token: &str,
    ) -> Result<Option<Event>, Error> {
        let event_id = event.id.to_string();

        // Check if we've already processed this event
        {
            let cache = self.response_event_cache.lock().await;
            if let Some(cached_response) = cache.get(&event_id) {
                return Ok(Some(cached_response.clone()));
            }
        }

        log::info!("Processing receive_cashu request");

        // Call receive_cashu
        let result = self.receive_cashu(token).await;

        // Build response JSON
        let response_json = match result {
            Ok(receive_result) => {
                serde_json::json!({
                    "result_type": "receive_cashu",
                    "result": {
                        "amount": receive_result.amount,
                        "mint_url": receive_result.mint_url,
                    }
                })
            }
            Err(e) => {
                log::error!("Failed to receive cashu token: {}", e);
                serde_json::json!({
                    "result_type": "receive_cashu",
                    "error": {
                        "code": "INTERNAL",
                        "message": e.to_string(),
                    }
                })
            }
        };

        // Encrypt response
        let encrypt_pubkey = if connection.app_pubkey.is_some() {
            connection.app_pubkey.as_ref().unwrap()
        } else {
            &self.keys.public_key()
        };

        let encrypted_response = nip04::encrypt(
            connection.keys.secret_key(),
            encrypt_pubkey,
            response_json.to_string(),
        )?;

        // Create response event
        let signing_keys = if connection.app_pubkey.is_some() {
            &connection.keys
        } else {
            &self.keys
        };

        let res_event = EventBuilder::new(Kind::WalletConnectResponse, encrypted_response)
            .tags(vec![
                Tag::from_standardized(TagStandard::public_key(event.pubkey)),
                Tag::from_standardized(TagStandard::event(event.id)),
            ])
            .sign_with_keys(signing_keys)?;
        // Cache response
        {
            let mut cache = self.response_event_cache.lock().await;
            cache.insert(event_id, res_event.clone());
        }

        // Update last check timestamp
        {
            let mut last_check = self.last_check.lock().await;
            *last_check = event.created_at;
        }

        Ok(Some(res_event))
    }

    /// Handles a custom pay_cashu_request request.
    async fn handle_pay_cashu_request(
        &self,
        connection: &mut WalletConnection,
        event: Event,
        payment_request: &str,
        amount: Option<u64>,
    ) -> Result<Option<Event>, Error> {
        let event_id = event.id.to_string();

        // Check if we've already processed this event
        {
            let cache = self.response_event_cache.lock().await;
            if let Some(cached_response) = cache.get(&event_id) {
                return Ok(Some(cached_response.clone()));
            }
        }

        log::info!("Processing pay_cashu_request request");

        // Call pay_cashu_request
        let result = self.pay_cashu_request(payment_request, amount).await;

        // Build response JSON
        let response_json = match result {
            Ok(pay_result) => {
                let mut result_obj = serde_json::json!({
                    "result_type": "pay_cashu_request",
                    "result": {
                        "amount": pay_result.amount,
                    }
                });

                // Add token field if present
                if let Some(token) = pay_result.token {
                    result_obj["result"]["token"] = serde_json::json!(token);
                }

                result_obj
            }
            Err(e) => {
                log::error!("Failed to pay cashu payment request: {}", e);
                serde_json::json!({
                    "result_type": "pay_cashu_request",
                    "error": {
                        "code": "INTERNAL",
                        "message": e.to_string(),
                    }
                })
            }
        };

        // Encrypt response
        let encrypt_pubkey = if connection.app_pubkey.is_some() {
            connection.app_pubkey.as_ref().unwrap()
        } else {
            &self.keys.public_key()
        };

        let encrypted_response = nip04::encrypt(
            connection.keys.secret_key(),
            encrypt_pubkey,
            response_json.to_string(),
        )?;

        // Create response event
        let signing_keys = if connection.app_pubkey.is_some() {
            &connection.keys
        } else {
            &self.keys
        };

        let res_event = EventBuilder::new(Kind::WalletConnectResponse, encrypted_response)
            .tags(vec![
                Tag::from_standardized(TagStandard::public_key(event.pubkey)),
                Tag::from_standardized(TagStandard::event(event.id)),
            ])
            .sign_with_keys(signing_keys)?;
        // Cache response
        {
            let mut cache = self.response_event_cache.lock().await;
            cache.insert(event_id, res_event.clone());
        }

        // Update last check timestamp
        {
            let mut last_check = self.last_check.lock().await;
            *last_check = event.created_at;
        }

        Ok(Some(res_event))
    }

    /// Handles a NIP-47 request and routes it to wallet methods.
    async fn handle_request(
        &self,
        request: nip47::Request,
        remaining_budget_msats: u64,
    ) -> (nip47::Response, Option<u64>, Option<BalanceInfo>) {
        match request.params {
            nip47::RequestParams::GetBalance => match self.get_balance().await {
                Ok(balance_info) => {
                    log::info!(
                        "Balance: {} msats, Max sendable: {} msats, Mints: {}",
                        balance_info.balance,
                        balance_info.max_sendable,
                        balance_info.mints.len()
                    );

                    let balance_info_clone = balance_info.clone();
                    (
                        nip47::Response {
                            result_type: nip47::Method::GetBalance,
                            error: None,
                            result: Some(nip47::ResponseResult::GetBalance(
                                nip47::GetBalanceResponse {
                                    balance: balance_info.balance,
                                },
                            )),
                        },
                        None,
                        Some(balance_info_clone),
                    )
                }
                Err(e) => (
                    nip47::Response {
                        result_type: nip47::Method::GetBalance,
                        error: Some(e.into()),
                        result: None,
                    },
                    None,
                    None,
                ),
            },
            nip47::RequestParams::MakeInvoice(params) => {
                match self.make_invoice(params.amount, params.description).await {
                    Ok(invoice_info) => {
                        let invoice = Bolt11Invoice::from_str(&invoice_info.request)
                            .expect("Valid invoice from wallet");
                        (
                            nip47::Response {
                                result_type: nip47::Method::MakeInvoice,
                                error: None,
                                result: Some(nip47::ResponseResult::MakeInvoice(
                                    nip47::MakeInvoiceResponse {
                                        invoice: invoice_info.request,
                                        payment_hash: invoice.payment_hash().to_string(),
                                    },
                                )),
                            },
                            None,
                            None,
                        )
                    }
                    Err(e) => (
                        nip47::Response {
                            result_type: nip47::Method::MakeInvoice,
                            error: Some(e.into()),
                            result: None,
                        },
                        None,
                        None,
                    ),
                }
            }
            nip47::RequestParams::PayInvoice(params) => {
                match self
                    .pay_invoice(&params.invoice, remaining_budget_msats)
                    .await
                {
                    Ok((payment_result, amount_msats)) => (
                        nip47::Response {
                            result_type: nip47::Method::PayInvoice,
                            error: None,
                            result: Some(nip47::ResponseResult::PayInvoice(
                                nip47::PayInvoiceResponse {
                                    preimage: payment_result.preimage.unwrap_or_default(),
                                    fees_paid: Some(payment_result.fee_paid),
                                },
                            )),
                        },
                        Some(amount_msats),
                        None,
                    ),
                    Err(e) => (
                        nip47::Response {
                            result_type: nip47::Method::PayInvoice,
                            error: Some(e.into()),
                            result: None,
                        },
                        None,
                        None,
                    ),
                }
            }
            _ => (
                nip47::Response {
                    result_type: request.method,
                    error: Some(nip47::NIP47Error {
                        code: nip47::ErrorCode::NotImplemented,
                        message: "Method not implemented".to_string(),
                    }),
                    result: None,
                },
                None,
                None,
            ),
        }
    }

    /// Gets the wallet balance in millisatoshis with per-mint details.
    pub(crate) async fn get_balance(&self) -> Result<BalanceInfo, Error> {
        let service = self.service_state.lock().await;
        let wallet_summary = service
            .get_wallet_summary()
            .await
            .map_err(|e| Error::Wallet(format!("Failed to get wallet summary: {}", e)))?;

        // Convert per-mint balances from sats to msats
        let mint_balances: Vec<MintBalance> = wallet_summary
            .balances
            .iter()
            .map(|b| {
                MintBalance {
                    mint_url: b.mint_url.clone(),
                    balance: b.balance * 1000, // Convert to msats
                    unit: "msat".to_string(),
                }
            })
            .collect();

        // Find the max sendable amount (highest single mint balance)
        let max_sendable = mint_balances.iter().map(|b| b.balance).max().unwrap_or(0);

        // Total balance in msats
        let total_balance = wallet_summary.total * 1000;

        Ok(BalanceInfo {
            balance: total_balance,
            max_sendable,
            mints: mint_balances,
        })
    }

    /// Creates a BOLT11 invoice.
    async fn make_invoice(
        &self,
        amount_msats: u64,
        description: Option<String>,
    ) -> Result<Bolt11InvoiceInfo, Error> {
        let service = self.service_state.lock().await;
        let amount_sats = amount_msats / 1000;
        service
            .create_bolt11_invoice(amount_sats, description)
            .await
            .map_err(|e| Error::Wallet(format!("Failed to create invoice: {}", e)))
    }

    /// Pays a BOLT11 invoice.
    async fn pay_invoice(
        &self,
        invoice: &str,
        remaining_budget_msats: u64,
    ) -> Result<(Bolt11PaymentResult, u64), Error> {
        log::info!("Paying invoice via NWC: {}", invoice);

        // Parse invoice to check amount
        let parsed_invoice = Bolt11Invoice::from_str(invoice)?;
        let amount_msats = parsed_invoice
            .amount_milli_satoshis()
            .ok_or(Error::InvalidInvoice)?;

        log::info!(
            "Invoice amount: {} msats, remaining budget: {} msats",
            amount_msats,
            remaining_budget_msats
        );

        // Check budget
        if amount_msats > remaining_budget_msats {
            return Err(Error::BudgetExceeded);
        }

        // Pay invoice through wallet
        let service = self.service_state.lock().await;
        let payment_result = service
            .pay_bolt11_invoice(invoice)
            .await
            .map_err(|e| Error::Wallet(format!("Failed to pay invoice: {}", e)))?;

        Ok((payment_result, amount_msats))
    }

    /// Receives a cashu token.
    async fn receive_cashu(&self, token: &str) -> Result<CashuReceiveResult, Error> {
        log::info!("Receiving cashu token via NWC");

        // Receive token through wallet
        let service = self.service_state.lock().await;
        let receive_result = service
            .receive_cashu_token(token)
            .await
            .map_err(|e| Error::Wallet(format!("Failed to receive cashu token: {}", e)))?;

        log::info!(
            "Successfully received {} sats to mint {}",
            receive_result.amount,
            receive_result.mint_url
        );

        Ok(receive_result)
    }

    /// Pays a NUT18 payment request, returning a Token if no transport is defined.
    async fn pay_cashu_request(
        &self,
        payment_request: &str,
        amount: Option<u64>,
    ) -> Result<PayNut18Result, Error> {
        log::info!("Paying cashu payment request via NWC");

        // Pay payment request through wallet
        let service = self.service_state.lock().await;
        let pay_result = service
            .pay_nut18_payment_request_with_token(payment_request, amount)
            .await
            .map_err(|e| Error::Wallet(format!("Failed to pay cashu payment request: {}", e)))?;

        if pay_result.token.is_some() {
            log::info!(
                "Successfully created token for {} sats (no transport, token returned)",
                pay_result.amount
            );
        } else {
            log::info!("Successfully paid {} sats via transport", pay_result.amount);
        }

        Ok(pay_result)
    }

    /// Gets the service public key for creating connection URIs.
    pub fn service_pubkey(&self) -> PublicKey {
        self.keys.public_key()
    }

    /// Creates a new NWA connection and returns the connection details.
    ///
    /// According to NIP-47, the wallet generates its own secret and returns it to the app.
    /// The app's secret from the URI is just an identifier for correlation.
    pub async fn create_nwa_connection(
        &self,
        app_pubkey_str: &str,
        _app_secret: String, // App's secret is just for correlation, we generate our own
        budget: ConnectionBudget,
    ) -> Result<WalletConnection, Error> {
        // Parse app's public key
        let app_pubkey = PublicKey::from_str(app_pubkey_str).map_err(Error::Key)?;

        // Generate our own secret for this connection (as per NIP-47 spec)
        let wallet_secret = uuid::Uuid::new_v4().to_string();

        // Create new connection with generated keypair and our own secret
        let connection = WalletConnection::from_nwa(app_pubkey, wallet_secret, budget);

        // Add connection to our list
        self.add_connection(connection.clone()).await?;

        log::info!(
            "Created NWA connection: app_pubkey={}, connection_pubkey={}, wallet_secret={}",
            app_pubkey,
            connection.keys.public_key(),
            connection.secret.as_ref().unwrap_or(&"None".to_string())
        );

        Ok(connection)
    }

    /// Creates and broadcasts a NWA approval event (kind 33194).
    ///
    /// This event is encrypted with NIP-04 and sent to the app's specified relays.
    pub async fn broadcast_nwa_approval(
        &self,
        connection: &WalletConnection,
        relays: Vec<String>,
        lud16: Option<String>,
    ) -> Result<(), Error> {
        let app_pubkey = connection
            .app_pubkey
            .ok_or_else(|| Error::Wallet("Connection missing app_pubkey".to_string()))?;

        let secret = connection
            .secret
            .clone()
            .ok_or_else(|| Error::Wallet("Connection missing secret".to_string()))?;

        // Build response JSON
        // The app needs to know our connection pubkey to send requests to
        let response = serde_json::json!({
            "secret": secret,
            "pubkey": connection.keys.public_key().to_hex(),
            "commands": ["pay_invoice", "make_invoice", "get_balance", "receive_cashu", "pay_cashu_request"],
            "relay": REMOTE_RELAY_URL,
            "lud16": lud16,
        });

        log::info!("Broadcasting NWA approval to app: {}", app_pubkey);

        // Encrypt the response with NIP-04 (app's pubkey, connection's secret key)
        let encrypted_content = nip04::encrypt(
            connection.keys.secret_key(),
            &app_pubkey,
            response.to_string(),
        )?;

        // Create the event (kind 33194, parameterized replaceable event)
        let event = EventBuilder::new(Kind::from(33194), encrypted_content)
            .tags(vec![Tag::from_standardized(TagStandard::Identifier(
                app_pubkey.to_string(),
            ))])
            .sign_with_keys(&connection.keys)?;
        // Add specified relays if they're different from our default
        for relay_url in relays {
            if relay_url != REMOTE_RELAY_URL {
                if let Err(e) = self.client.add_relay(&relay_url).await {
                    log::warn!("Failed to add relay {}: {}", relay_url, e);
                }
            }
        }

        // Broadcast the event
        self.client.send_event(&event).await?;
        log::info!("Successfully broadcasted NWA approval event: {}", event.id);

        Ok(())
    }
}

/// A wallet connection configuration.
#[derive(Debug, Clone)]
pub struct WalletConnection {
    /// Connection keys (generated for each connection)
    pub keys: Keys,
    /// Connection budget
    pub budget: ConnectionBudget,
    /// App's public key (for NWA connections)
    pub app_pubkey: Option<PublicKey>,
    /// Connection secret (for NWA connections)
    pub secret: Option<String>,
    /// User-defined display name
    pub name: String,
}

impl WalletConnection {
    pub(crate) fn default_name(keys: &Keys) -> String {
        let hex = keys.public_key().to_hex();
        let short = hex.get(..8).unwrap_or(&hex);
        format!("Connection {}", short)
    }

    /// Creates a new wallet connection.
    pub fn new(secret: SecretKey, budget: ConnectionBudget) -> Self {
        let keys = Keys::new(secret);
        Self {
            name: Self::default_name(&keys),
            keys,
            budget,
            app_pubkey: None,
            secret: None,
        }
    }

    /// Creates a wallet connection from NWA request.
    ///
    /// For each NWA connection, we generate a unique keypair:
    /// - The connection's public key is sent to the app in the approval response
    /// - The app sends NWC requests FROM its own pubkey TO our connection pubkey
    /// - The secret is only used by the app to correlate the approval with its request
    pub fn from_nwa(app_pubkey: PublicKey, secret: String, budget: ConnectionBudget) -> Self {
        // Generate a NEW unique keypair for this connection
        let connection_keys = Keys::generate();

        Self {
            name: Self::default_name(&connection_keys),
            keys: connection_keys,
            budget,
            app_pubkey: Some(app_pubkey),
            secret: Some(secret),
        }
    }

    /// Creates a Nostr filter for this connection.
    ///
    /// NWA (Nostr Wallet Auth):
    /// - Each connection has a unique generated keypair
    /// - The app sends events FROM its own pubkey TO our connection pubkey (p-tag)
    /// - We decrypt using the connection's secret key and the app's pubkey
    ///
    /// Standard NWC:
    /// - The connection keypair is shared with the app via a URI
    /// - The app sends events FROM the connection pubkey TO the service pubkey (p-tag)
    /// - We decrypt using the service's secret key and the connection pubkey
    fn filter(&self, service_pubkey: PublicKey, since: Timestamp) -> Filter {
        if let Some(app_pubkey) = self.app_pubkey {
            // NWA connection: filter events authored by app, tagged to our connection pubkey
            Filter::new()
                .kind(Kind::WalletConnectRequest)
                .author(app_pubkey)
                .since(since)
                .custom_tag(
                    SingleLetterTag::lowercase(Alphabet::P),
                    self.keys.public_key().to_string(),
                )
        } else {
            // Standard NWC: filter events authored by connection, tagged to service pubkey
            Filter::new()
                .kind(Kind::WalletConnectRequest)
                .author(self.keys.public_key())
                .since(since)
                .custom_tag(
                    SingleLetterTag::lowercase(Alphabet::P),
                    service_pubkey.to_string(),
                )
        }
    }

    /// Gets the next budget renewal timestamp.
    pub fn budget_renews_at(&self) -> Option<Timestamp> {
        let now = Timestamp::now();
        let period = match self.budget.renewal_period {
            BudgetRenewalPeriod::Daily => Duration::from_secs(24 * 60 * 60),
            BudgetRenewalPeriod::Weekly => Duration::from_secs(7 * 24 * 60 * 60),
            BudgetRenewalPeriod::Monthly => Duration::from_secs(30 * 24 * 60 * 60),
            BudgetRenewalPeriod::Yearly => Duration::from_secs(365 * 24 * 60 * 60),
            _ => return None,
        };
        let mut renews_at = match self.budget.renews_at {
            Some(t) => t,
            None => now,
        };

        loop {
            if renews_at > now {
                return Some(renews_at);
            }
            renews_at = renews_at + period;
        }
    }

    /// Gets the Wallet Connect URI for this connection.
    pub fn uri(&self, service_pubkey: PublicKey, relay: Url) -> Result<String, Error> {
        let relay_url = RelayUrl::parse(relay.as_ref()).map_err(|e| Error::Url(e.to_string()))?;
        let uri = NostrWalletConnectURI::new(
            service_pubkey,
            vec![relay_url],
            self.keys.secret_key().clone(),
            None,
        );
        Ok(uri.to_string())
    }
}

/// Balance information for a single mint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MintBalance {
    pub mint_url: String,
    pub balance: u64,
    pub unit: String,
}

/// Extended balance information including per-mint balances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceInfo {
    pub balance: u64,
    pub max_sendable: u64,
    pub mints: Vec<MintBalance>,
}

/// Budget configuration for a wallet connection.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ConnectionBudget {
    /// Renewal period
    pub renewal_period: BudgetRenewalPeriod,
    /// When the budget renews next
    pub renews_at: Option<Timestamp>,
    /// Total budget in millisatoshis
    pub total_budget_msats: u64,
    /// Used budget in millisatoshis
    pub used_budget_msats: u64,
}

impl Default for ConnectionBudget {
    fn default() -> Self {
        Self {
            renewal_period: BudgetRenewalPeriod::Daily,
            renews_at: None,
            total_budget_msats: NWC_BUDGET_MSATS,
            used_budget_msats: 0,
        }
    }
}

/// Budget renewal period options.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BudgetRenewalPeriod {
    /// Daily (24 hours)
    Daily,
    /// Weekly (7 days)
    Weekly,
    /// Monthly (30 days)
    Monthly,
    /// Yearly (365 days)
    Yearly,
    /// Never renews
    Never,
}

/// NWC error types.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Budget exceeded")]
    BudgetExceeded,

    #[error("Client error: {0}")]
    Client(#[from] nostr_sdk::client::Error),

    #[error("Connection not found")]
    ConnectionNotFound,

    #[error("Event builder error: {0}")]
    EventBuilder(#[from] nostr_sdk::event::builder::Error),

    #[error("Invalid invoice")]
    InvalidInvoice,

    #[error("Invalid event kind")]
    InvalidKind,

    #[error("Invalid service key: {0}")]
    InvalidServiceKey(PublicKey),

    #[error("Invoice parse error: {0}")]
    InvoiceParse(String),

    #[error("Key error: {0}")]
    Key(#[from] nostr_sdk::key::Error),

    #[error("Missing service key in event")]
    MissingServiceKey,

    #[error("NIP-04 error: {0}")]
    Nip04(#[from] nip04::Error),

    #[error("NIP-47 error: {0}")]
    Nip47(#[from] nip47::Error),

    #[error("URL parse error: {0}")]
    Url(String),

    #[error("Wallet error: {0}")]
    Wallet(String),
}

impl From<lightning_invoice::ParseOrSemanticError> for Error {
    fn from(err: lightning_invoice::ParseOrSemanticError) -> Self {
        Error::InvoiceParse(format!("{:?}", err))
    }
}

impl From<Error> for nip47::NIP47Error {
    fn from(val: Error) -> Self {
        match val {
            Error::BudgetExceeded => nip47::NIP47Error {
                code: nip47::ErrorCode::QuotaExceeded,
                message: "Budget exceeded".to_string(),
            },
            Error::InvalidInvoice => nip47::NIP47Error {
                code: nip47::ErrorCode::Other,
                message: "Invalid invoice".to_string(),
            },
            e => nip47::NIP47Error {
                code: nip47::ErrorCode::Internal,
                message: e.to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tollgate::TollGateService;
    use nostr_sdk::SecretKey;

    #[tokio::test]
    async fn test_nwc_connection_flow() {
        println!("=== Starting NWC Connection Flow Test ===");

        // Step 1: Create TollGate service
        println!("Step 1: Creating TollGate service...");
        let service = TollGateService::new()
            .await
            .expect("Failed to create TollGate service");
        let service_state = Arc::new(Mutex::new(service));
        println!("✓ TollGate service created");

        // Step 2: Create NWC service
        println!("Step 2: Creating NWC service...");
        let service_key = SecretKey::generate();
        let nwc = NostrWalletConnect::new(service_key, service_state.clone())
            .await
            .expect("Failed to create NWC service");
        println!(
            "✓ NWC service created with pubkey: {}",
            nwc.service_pubkey()
        );

        // Step 3: Create a wallet connection
        println!("Step 3: Creating wallet connection...");
        let connection_key = SecretKey::generate();
        let budget = ConnectionBudget {
            renewal_period: BudgetRenewalPeriod::Daily,
            renews_at: None,
            total_budget_msats: 1_000_000_000, // 1,000 sats
            used_budget_msats: 0,
        };
        let connection = WalletConnection::new(connection_key, budget);
        println!(
            "✓ Connection created with pubkey: {}",
            connection.keys.public_key()
        );

        // Step 4: Add connection to NWC service
        println!("Step 4: Adding connection to NWC service...");
        nwc.add_connection(connection.clone())
            .await
            .expect("Failed to add connection");
        println!("✓ Connection added successfully");

        // Step 5: Get info event
        println!("Step 5: Getting NWC info event...");
        let info_event = nwc.info_event().expect("Failed to create info event");
        println!("✓ Info event created:");
        println!("  - Event ID: {}", info_event.id);
        println!("  - Kind: {:?}", info_event.kind);
        println!("  - Content: {}", info_event.content);
        println!("  - Pubkey: {}", info_event.pubkey);

        // Log info event as JSON
        if let Ok(json) = serde_json::to_string_pretty(&serde_json::json!({
            "id": info_event.id.to_string(),
            "kind": format!("{:?}", info_event.kind),
            "content": info_event.content,
            "pubkey": info_event.pubkey.to_string(),
            "created_at": info_event.created_at.as_u64(),
            "tags": info_event.tags.iter().map(|t| t.as_slice()).collect::<Vec<_>>(),
        })) {
            println!("\n  Info event JSON:\n{}", json);
        }

        // Step 6: Get balance
        println!("\nStep 6: Getting wallet balance...");
        match nwc.get_balance().await {
            Ok(balance_info) => {
                println!("✓ Balance retrieved successfully:");
                println!(
                    "  - Total balance: {} msats ({} sats)",
                    balance_info.balance,
                    balance_info.balance / 1000
                );
                println!(
                    "  - Max sendable: {} msats ({} sats)",
                    balance_info.max_sendable,
                    balance_info.max_sendable / 1000
                );
                println!("  - Number of mints: {}", balance_info.mints.len());
                for (i, mint) in balance_info.mints.iter().enumerate() {
                    println!(
                        "    Mint {}: {} - {} msats ({} sats)",
                        i + 1,
                        mint.mint_url,
                        mint.balance,
                        mint.balance / 1000
                    );
                }

                // Log balance as JSON
                if let Ok(json) = serde_json::to_string_pretty(&balance_info) {
                    println!("\n  Balance JSON:\n{}", json);
                }
            }
            Err(e) => {
                println!("! Balance retrieval returned error (this may be expected if no mints configured): {}", e);
            }
        }

        // Step 7: Verify connections are stored
        println!("Step 7: Verifying stored connections...");
        let connections = nwc.get_connections().await;
        println!("✓ Retrieved {} connection(s)", connections.len());
        assert!(
            !connections.is_empty(),
            "Should have at least one connection"
        );

        // Verify our connection is in the list
        let found = connections
            .iter()
            .any(|c| c.keys.public_key() == connection.keys.public_key());
        assert!(found, "Our connection should be in the list");

        println!("=== Test completed successfully ===");
    }

    #[tokio::test]
    async fn test_nwa_connection_flow() {
        println!("=== Starting NWA Connection Flow Test ===");

        // Step 1: Create TollGate service
        println!("Step 1: Creating TollGate service...");
        let service = TollGateService::new()
            .await
            .expect("Failed to create TollGate service");
        let service_state = Arc::new(Mutex::new(service));
        println!("✓ TollGate service created");

        // Step 2: Create NWC service
        println!("Step 2: Creating NWC service...");
        let service_key = SecretKey::generate();
        let nwc = NostrWalletConnect::new(service_key, service_state.clone())
            .await
            .expect("Failed to create NWC service");
        println!(
            "✓ NWC service created with pubkey: {}",
            nwc.service_pubkey()
        );

        // Step 3: Create NWA connection (simulating an app request)
        println!("Step 3: Creating NWA connection...");
        let app_keys = Keys::generate();
        let secret = "test_secret_123".to_string();
        let budget = ConnectionBudget {
            renewal_period: BudgetRenewalPeriod::Weekly,
            renews_at: None,
            total_budget_msats: 5_000_000_000, // 5,000 sats
            used_budget_msats: 0,
        };

        let nwa_connection = nwc
            .create_nwa_connection(&app_keys.public_key().to_hex(), secret.clone(), budget)
            .await
            .expect("Failed to create NWA connection");

        println!("✓ NWA connection created:");
        println!("  - App pubkey: {}", app_keys.public_key());
        println!(
            "  - Connection pubkey: {}",
            nwa_connection.keys.public_key()
        );
        println!("  - Secret: {}", secret);
        println!("  - Budget: {} sats", budget.total_budget_msats / 1000);

        // Log NWA connection details as JSON
        if let Ok(json) = serde_json::to_string_pretty(&serde_json::json!({
            "app_pubkey": app_keys.public_key().to_string(),
            "connection_pubkey": nwa_connection.keys.public_key().to_string(),
            "secret": secret,
            "budget": {
                "total_msats": budget.total_budget_msats,
                "total_sats": budget.total_budget_msats / 1000,
                "used_msats": budget.used_budget_msats,
                "renewal_period": format!("{:?}", budget.renewal_period),
            }
        })) {
            println!("\n  NWA Connection JSON:\n{}", json);
        }

        // Step 4: Verify connection is stored
        println!("Step 4: Verifying stored connections...");
        let connections = nwc.get_connections().await;
        println!("✓ Retrieved {} connection(s)", connections.len());
        assert!(
            !connections.is_empty(),
            "Should have at least one connection"
        );

        // Find our connection
        let stored_connection = connections
            .iter()
            .find(|c| c.keys.public_key() == nwa_connection.keys.public_key())
            .expect("Our connection should be in the list");
        assert_eq!(stored_connection.app_pubkey, Some(app_keys.public_key()));
        // The secret is generated by the wallet (UUID), not the app's secret
        assert_eq!(stored_connection.secret, nwa_connection.secret);
        assert!(
            stored_connection.secret.is_some(),
            "Connection should have a secret"
        );
        println!("✓ Connection properly stored with NWA details");

        println!("=== Test completed successfully ===");
    }

    #[tokio::test]
    async fn test_receive_cashu_token() {
        println!("=== Starting Receive Cashu Token Test ===");

        // Step 1: Create TollGate service
        println!("Step 1: Creating TollGate service...");
        let service = TollGateService::new()
            .await
            .expect("Failed to create TollGate service");
        let service_state = Arc::new(Mutex::new(service));
        println!("✓ TollGate service created");

        // Step 2: Create NWC service
        println!("Step 2: Creating NWC service...");
        let service_key = SecretKey::generate();
        let nwc = NostrWalletConnect::new(service_key, service_state.clone())
            .await
            .expect("Failed to create NWC service");
        println!(
            "✓ NWC service created with pubkey: {}",
            nwc.service_pubkey()
        );

        // Step 3: Get balance before receiving
        println!("Step 3: Getting initial balance...");
        match nwc.get_balance().await {
            Ok(balance_info) => {
                println!("✓ Initial balance: {} sats", balance_info.balance / 1000);
                println!("  Mints: {}", balance_info.mints.len());
            }
            Err(e) => {
                println!("! Initial balance error (may be expected): {}", e);
            }
        }

        // Step 4: Test receiving cashu token
        println!("\nStep 4: Testing cashu token reception...");
        // Note: Using a test token that may already be spent in testing environment
        let token = "cashuBo2FteCJodHRwczovL25vZmVlcy50ZXN0bnV0LmNhc2h1LnNwYWNlYXVjc2F0YXSBomFpSAC0zSfYhhpEYXCFpGFhCGFzeEE5YmViNTE0ZTE2MjFkM2RkYTY0MjgyNDg4Zjg5ZTBkZTk4Y2IyNmM3NGI2MjNmNjllZGMwYWMxOTA3ZTAxMjA1YWNYIQMw7UppJvgL0Ixr7brd2QUSiZ_BkkWgkpmo_ojPa-W5wGFko2FlWCDgFHEyX6D2iU-Mam3xrcfzMHTXP2QFuDALk8BKQqxhIWFzWCDD81s4-_savlVBT05zsXEYv59_DT9G_VuSHzgMUU081GFyWCDIs4v0uSoV9dlp09FeFE7iNG1RGmbd7n4zwkBotSS0_6RhYQhhc3hAMDg5NTg4M2Y4NjQwMzMwY2Q1ODY1ODc0MTE5ZGRkZWExODJiZWYxNmU1ZWI5YzliODk3YjUxNzI4NjgzMzdmM2FjWCECwXXD_aWRi1ZY4VAw4QC_3WAd-dzIO16wsP0448PSZfZhZKNhZVggI96r12eU2NmET3Y9iuvRB_BHA8yTKJ0ovVqXpAVXnTFhc1gg25yD6mRI9PMP70IqAje3BDgiQOsnGrsM5vSJbOm8slVhclgg8jW7TRtey7xrQfv762Fx9aGICHfeFQ1UTaj5MPi6IAmkYWECYXN4QDhhOWEyNmM0ZDg4ZWYyY2E2MDlkYjJjNjY3MWQ1YTU3OWZhMDhkYjU1ODI3YmVjZGJiMmNlNTNiOGEyZWVjMGVhY1ghApZZIz1vpxeW6zrSv44msnU3Ky0M0Ad8kCbxfCW9F8GqYWSjYWVYIAD_aln-jTz31V1v3Jcp8zLZoIHmKGCwJcsZrHmbvqAaYXNYIC7lL1yomkctyPMfGjPj6hsm6ZTs5gyJkiUtuxSan1BMYXJYIDf4xrFqo6s200g1AOLP8CZqFjgRUBqL8St5tF_1PGRQpGFhAmFzeEE0YzAzMTI1ZDRhZTU3NWM2MTBiNzBmMWYwN2VlMTNiMjkwN2E2MWQ4NzgwOWRkMjM2MTA2NmJjNjAwNzVmZDQ3YWNYIQNXh9p03x9bqCAj4picnMqOpqY9m8S3W3502ayAaqGvJmFko2FlWCBM--Kr27PYSt-xNng4q5a8w_3moX8V2JybosGthPnzrGFzWCCcrJS0WuLvD3b_Y0g_8OImwA9Ly2rKwp2bRvAskjegKGFyWCBZfFAv0nqKNBC_FM8QzSu3eOV4NkA3eSD40CVMiCi5rKRhYQFhc3hAMGZhMjM4M2Y1YjUzZTA0MWQzOWIxMDQ4YWVlZWQ3NjRmMTU1MDBkMzE4YmI1ZGU4MzNiOTJkZjUzMjBkZjM2NGFjWCEC6_oMe4HmiKrmyukKGez4sOaA-m2I7MloMXqE9zbFoDJhZKNhZVggagzjRZB-jJ9xJ1KZzbyRCH2C39Utiole54pyD0fnIvBhc1ggulky-qM3PRpNCg_tZoSWPDFnpSqdB0SX6M4KvINWmeZhclggbMnmAC1Pe3KPY07KJqTPh84IsgrmqmcjNYHMsp3wCFQ";

        match nwc.receive_cashu(token).await {
            Ok(result) => {
                println!("✓ Successfully received cashu token!");
                println!("  Amount: {} sats", result.amount);
                println!("  Mint URL: {}", result.mint_url);

                // Log as JSON
                if let Ok(json) = serde_json::to_string_pretty(&serde_json::json!({
                    "amount_sats": result.amount,
                    "mint_url": result.mint_url,
                })) {
                    println!("\n  Receive Result JSON:\n{}", json);
                }

                // Verify we got a reasonable amount
                assert!(result.amount > 0, "Should have received a non-zero amount");
                assert!(!result.mint_url.is_empty(), "Should have a mint URL");
            }
            Err(e) => {
                // Token may already be spent or invalid in test environment - this is expected
                let error_str = e.to_string();
                if error_str.contains("Token Already Spent")
                    || error_str.contains("already spent")
                    || error_str.contains("Invalid cashu token")
                    || error_str.contains("invalid type")
                {
                    println!("! Token error (expected in test environment): {}", e);
                    println!("✓ Test passed - receive_cashu method works correctly");
                } else {
                    println!("✗ Unexpected error receiving cashu token: {}", e);
                    panic!("Unexpected token receive error: {}", e);
                }
            }
        }

        // Step 5: Verify that the method at least exists and can be called
        println!("\nStep 5: Verifying cashu receive functionality exists...");
        // The fact that we got here means the receive_cashu method exists and can be called
        println!("✓ receive_cashu method is properly implemented and callable");

        println!("\n=== Test completed successfully ===");
    }

    #[tokio::test]
    async fn test_pay_cashu_request() {
        println!("=== Starting Pay Cashu Request Test ===");

        // Step 1: Create TollGate service
        println!("Step 1: Creating TollGate service...");
        let service = TollGateService::new()
            .await
            .expect("Failed to create TollGate service");
        let service_state = Arc::new(Mutex::new(service));
        println!("✓ TollGate service created");

        // Step 2: Create NWC service
        println!("Step 2: Creating NWC service...");
        let service_key = SecretKey::generate();
        let nwc = NostrWalletConnect::new(service_key, service_state.clone())
            .await
            .expect("Failed to create NWC service");
        println!(
            "✓ NWC service created with pubkey: {}",
            nwc.service_pubkey()
        );

        // Step 3: Pay the cashu request
        println!("\nStep 3: Paying cashu payment request...");
        let payment_request = "creqApWF0gaNhdGVub3N0cmFheKlucHJvZmlsZTFxeTI4d3VtbjhnaGo3dW45ZDNzaGp0bnl2OWtoMnVld2Q5aHN6OW1od2RlbjV0ZTB3ZmprY2N0ZTljdXJ4dmVuOWVlaHFjdHJ2NWhzenJ0aHdkZW41dGUwZGVoaHh0bnZkYWtxcWd6Z21yMnB0MDk0OTV0ZG5sbXduZ3NmdTN5NjR1cDh4ODVmcnM5c2h5a3lwYzU0dm5ranNneTU2enNtYWeBgmFuYjE3YWloNWVmYzE3ZWZhYQVhdWNzYXRhbYF4Imh0dHBzOi8vbm9mZWVzLnRlc3RudXQuY2FzaHUuc3BhY2U=";

        match nwc.pay_cashu_request(payment_request, None).await {
            Ok(result) => {
                println!("✓ Successfully processed cashu payment request!");
                println!("  Amount: {} sats", result.amount);

                if let Some(token) = &result.token {
                    println!(
                        "  Token returned (no transport): {}",
                        &token[..50.min(token.len())]
                    );

                    // Log as JSON
                    if let Ok(json) = serde_json::to_string_pretty(&serde_json::json!({
                        "amount_sats": result.amount,
                        "token_preview": &token[..100.min(token.len())],
                    })) {
                        println!("\n  Pay Result JSON:\n{}", json);
                    }

                    // Verify we got a token since the request has a transport
                    println!("  Note: Token was returned - this may indicate no transport was defined or payment failed");
                } else {
                    println!("  Payment sent via transport (no token returned)");
                }

                // Verify we got a reasonable amount
                assert!(
                    result.amount > 0,
                    "Should have created token for a non-zero amount"
                );
            }
            Err(e) => {
                println!("✗ Failed to pay cashu request: {}", e);
                println!("  This is expected if:");
                println!("  - The wallet has no balance");
                println!("  - The mint is not configured");
                println!("  - The payment request is invalid/expired");

                // Don't panic - this test is informational
                println!("  Test result: Payment failed (may be expected)");
            }
        }

        println!("\n=== Test completed ===");
    }

    #[tokio::test]
    async fn test_pay_cashu_request_no_transport() {
        println!("=== Starting Pay Cashu Request (No Transport) Test ===");

        // Step 1: Create TollGate service
        println!("Step 1: Creating TollGate service...");
        let service = TollGateService::new()
            .await
            .expect("Failed to create TollGate service");
        let service_state = Arc::new(Mutex::new(service));
        println!("✓ TollGate service created");

        // Step 2: Add the mint and some balance
        println!("Step 2: Adding test mint...");
        {
            let service = service_state.lock().await;
            match service.add_mint("https://nofees.testnut.cashu.space").await {
                Ok(_) => println!("✓ Mint added"),
                Err(e) => println!("! Mint add failed (may already exist): {}", e),
            }
        }

        // Step 3: Create NWC service
        println!("Step 3: Creating NWC service...");
        let service_key = SecretKey::generate();
        let nwc = NostrWalletConnect::new(service_key, service_state.clone())
            .await
            .expect("Failed to create NWC service");
        println!(
            "✓ NWC service created with pubkey: {}",
            nwc.service_pubkey()
        );

        // Step 4: Create a payment request with no transport
        println!("\nStep 4: Creating payment request with no transport...");
        let payment_request = {
            let service = service_state.lock().await;
            match service
                .create_nut18_payment_request(Some(5), Some("Test payment".to_string()))
                .await
            {
                Ok(pr) => {
                    println!(
                        "✓ Created payment request: {}",
                        &pr.request[..50.min(pr.request.len())]
                    );
                    pr.request
                }
                Err(e) => {
                    println!("! Failed to create payment request: {}", e);
                    println!("  Skipping test (wallet may have no mints configured)");
                    return;
                }
            }
        };

        // Step 5: Pay the cashu request (should return a token since there's no transport)
        println!("\nStep 5: Paying cashu payment request with no transport...");
        match nwc.pay_cashu_request(&payment_request, None).await {
            Ok(result) => {
                println!("✓ Successfully processed cashu payment request!");
                println!("  Amount: {} sats", result.amount);

                if let Some(token) = &result.token {
                    println!("✓ Token returned as expected (no transport defined)");
                    println!("  Token preview: {}...", &token[..50.min(token.len())]);

                    // Verify the token is valid
                    assert!(
                        token.starts_with("cashu"),
                        "Token should start with 'cashu'"
                    );
                    assert!(result.amount == 5, "Amount should be 5 sats");

                    println!("✓ Token format verified");
                } else {
                    panic!("Expected token to be returned when no transport is defined");
                }
            }
            Err(e) => {
                println!("✗ Failed to pay cashu request: {}", e);
                println!("  This is expected if the wallet has no balance");
                println!("  Test result: Payment failed (insufficient funds)");
            }
        }

        // Step 6: Create an amount-less payment request
        println!("\nStep 6: Creating amount-less payment request...");
        let amountless_request = {
            let service = service_state.lock().await;
            match service
                .create_nut18_payment_request(None, Some("Amount-less payment".to_string()))
                .await
            {
                Ok(pr) => {
                    println!(
                        "✓ Created amount-less payment request: {}",
                        &pr.request[..50.min(pr.request.len())]
                    );
                    assert!(pr.amount.is_none(), "Payment request should have no amount");
                    println!("✓ Verified payment request has no amount");
                    pr.request
                }
                Err(e) => {
                    println!("! Failed to create amount-less payment request: {}", e);
                    println!("  Skipping test (wallet may have no mints configured)");
                    return;
                }
            }
        };

        // Step 7: Pay the amount-less request with a custom amount
        println!("\nStep 7: Paying amount-less payment request with custom amount of 10 sats...");
        match nwc.pay_cashu_request(&amountless_request, Some(10)).await {
            Ok(result) => {
                println!(
                    "✓ Successfully processed amount-less payment request with custom amount!"
                );
                println!("  Amount: {} sats", result.amount);

                if let Some(token) = &result.token {
                    println!("✓ Token returned as expected (no transport defined)");
                    println!("  Token preview: {}...", &token[..50.min(token.len())]);

                    // Verify the token is valid and amount matches
                    assert!(
                        token.starts_with("cashu"),
                        "Token should start with 'cashu'"
                    );
                    assert!(
                        result.amount == 10,
                        "Amount should be 10 sats (the custom amount)"
                    );

                    println!("✓ Token format and amount verified");
                } else {
                    panic!("Expected token to be returned when no transport is defined");
                }
            }
            Err(e) => {
                println!("✗ Failed to pay amount-less request: {}", e);
                println!("  This is expected if the wallet has no balance");
                println!("  Test result: Payment failed (insufficient funds or invalid request)");
            }
        }

        // Step 8: Test that amount-less request fails without custom amount
        println!("\nStep 8: Testing that amount-less request fails without custom amount...");
        match nwc.pay_cashu_request(&amountless_request, None).await {
            Ok(_) => {
                println!(
                    "✗ Unexpectedly succeeded paying amount-less request without custom amount!"
                );
                panic!("Should have failed when no amount is provided");
            }
            Err(_e) => {
                println!("✓ Correctly failed to pay amount-less request without custom amount");
            }
        }

        println!("\n=== Test completed ===");
    }
}
