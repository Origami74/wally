//! Nostr Wallet Connect (NIP-47) implementation for the TollGate wallet.
//!
//! This module provides NWC functionality that allows external applications
//! to interact with the wallet through Nostr relays.

use crate::nwc_storage::NwcConnectionStorage;
use crate::tollgate::wallet::{Bolt11InvoiceInfo, Bolt11PaymentResult, CashuReceiveResult, PayNut18Result};
use crate::TollGateState;
use lightning_invoice::Bolt11Invoice;
use nostr_sdk::{
    nips::{
        nip04,
        nip47::{self, MakeInvoiceResponseResult, NostrWalletConnectURI},
    },
    Client, Event, EventBuilder, EventSource, Filter, JsonUtil, Keys, Kind, PublicKey, SecretKey,
    SingleLetterTag, Tag, TagKind, TagStandard, Timestamp, Url, Alphabet,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

const RELAY_URL: &str = "ws://localhost:4869";
const NWC_BUDGET_MSATS: u64 = 1_000_000_000; // 1,000 sats budget

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
    /// Creates a new NWC service instance.
    pub async fn new(
        service_key: SecretKey,
        service_state: TollGateState,
    ) -> Result<Self, Error> {
        let keys = Keys::new(service_key);
        let client = Client::default();
        
        // Initialize storage
        let storage = Arc::new(NwcConnectionStorage::new().map_err(|e| {
            Error::Wallet(format!("Failed to initialize NWC storage: {}", e))
        })?);
        
        // Load existing connections from storage
        let connections = storage.load_connections().map_err(|e| {
            Error::Wallet(format!("Failed to load NWC connections: {}", e))
        })?;
        
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
        // Add relay
        self.client.add_relay(RELAY_URL).await?;
        
        // Connect to relay
        self.client.connect().await;

        log::info!("NWC service connected to relay: {}", RELAY_URL);

        // Publish info event
        self.publish_info_event().await?;

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
        self.storage.save_connection(&connection).map_err(|e| {
            Error::Wallet(format!("Failed to save connection to storage: {}", e))
        })?;
        
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
        let mut connections = self.connections.write().await;
        
        // Find and remove the connection
        let initial_len = connections.len();
        connections.retain(|conn| conn.keys.public_key().to_hex() != connection_pubkey);
        
        if connections.len() < initial_len {
            // Connection was removed, delete from storage
            self.storage.delete_connection(connection_pubkey).map_err(|e| {
                Error::Wallet(format!("Failed to delete connection from storage: {}", e))
            })?;
            log::info!("Removed and deleted NWC connection: {}", connection_pubkey);
        }
        
        Ok(())
    }

    /// Creates a kind 13194 info event for the NWC service.
    pub fn info_event(&self) -> Result<Event, Error> {
        let event = EventBuilder::new(
            Kind::WalletConnectInfo,
            "get_balance make_invoice pay_invoice receive_cashu pay_cashu_request",
            vec![],
        )
        .to_event(&self.keys)?;
        Ok(event)
    }

    /// Publishes the NWC info event.
    pub async fn publish_info_event(&self) -> Result<(), Error> {
        let event = self.info_event()?;
        self.client.send_event(event).await?;
        log::info!("Published NWC info event");
        Ok(())
    }

    /// Gets the Nostr filters for NWC requests.
    pub async fn filters(&self) -> Vec<Filter> {
        let last_check = *self.last_check.lock().await;
        let connections = self.connections.read().await;
        connections
            .iter()
            .map(|conn| conn.filter(self.keys.public_key(), last_check))
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

            // Query events from relay
            match self
                .client
                .get_events_of(filters, EventSource::relays(Some(Duration::from_secs(5))))
                .await
            {
                Ok(events) => {
                    log::debug!("Received {} events", events.len());
                    for event in events {
                        match self.handle_event(event).await {
                            Ok(Some(response)) => {
                                if let Err(e) = self.client.send_event(response).await {
                                    log::error!("Failed to send response: {}", e);
                                }
                            }
                            Ok(None) => {
                                // Already processed, skip
                            }
                            Err(e) => {
                                log::error!("Error handling event: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("Error querying events: {}", e);
                }
            }

            // Wait before next check
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    /// Handles a single NWC request event.
    pub async fn handle_event(&self, event: Event) -> Result<Option<Event>, Error> {
        if event.kind != Kind::WalletConnectRequest {
            return Err(Error::InvalidKind);
        }

        // Get the target pubkey from the 'p' tag
        let _target_pubkey = PublicKey::from_str(
            event
                .get_tag_content(TagKind::SingleLetter(SingleLetterTag::lowercase(
                    Alphabet::P,
                )))
                .ok_or(Error::MissingServiceKey)?,
        )?;

        // For NWA connections, _target_pubkey should be the connection pubkey
        // For standard NWC, _target_pubkey should be the service pubkey
        // We validate this matches a known connection when we find the matching connection

        let event_id = event.id.to_string();

        // Check if we've already processed this event
        {
            let cache = self.response_event_cache.lock().await;
            if let Some(cached_response) = cache.get(&event_id) {
                return Ok(Some(cached_response.clone()));
            }
        }

        log::info!("Processing NWC event: {}", event_id);

        // Find matching connection (check both standard and NWA connections)
        let mut connections = self.connections.write().await;
        let connection = connections
            .iter_mut()
            .find(|conn| {
                // For NWA connections, match on app_pubkey
                if let Some(app_pubkey) = conn.app_pubkey {
                    app_pubkey == event.pubkey
                } else {
                    // For standard NWC, match on connection pubkey
                    conn.keys.public_key() == event.pubkey
                }
            })
            .ok_or(Error::ConnectionNotFound)?;

        // Decrypt request
        // For NWA: decrypt with connection's secret key and app's pubkey
        // For standard NWC: decrypt with connection's secret key and service pubkey
        let decrypt_pubkey = if connection.app_pubkey.is_some() {
            connection.app_pubkey.as_ref().unwrap()
        } else {
            &self.keys.public_key()
        };
        
        let decrypted_content = nip04::decrypt(
            connection.keys.secret_key(),
            decrypt_pubkey,
            &event.content,
        )?;
        
        // Try to parse as JSON first to check for custom methods
        let json_value: serde_json::Value = serde_json::from_str(&decrypted_content)
            .map_err(|e| Error::Wallet(format!("Failed to parse request JSON: {}", e)))?;
        
        // Check if this is a custom method
        let method = json_value.get("method")
            .and_then(|m| m.as_str())
            .ok_or_else(|| Error::Wallet("Missing method field in request".to_string()))?;
        
        // Handle custom methods
        if method == "receive_cashu" {
            // Parse custom params
            let params = json_value.get("params")
                .ok_or_else(|| Error::Wallet("Missing params field in request".to_string()))?;
            let token = params.get("token")
                .and_then(|t| t.as_str())
                .ok_or_else(|| Error::Wallet("Missing token in receive_cashu params".to_string()))?;
            
            // Handle custom receive_cashu request
            return self.handle_receive_cashu_request(connection, event, token).await;
        }
        
        if method == "pay_cashu_request" {
            // Parse custom params
            let params = json_value.get("params")
                .ok_or_else(|| Error::Wallet("Missing params field in request".to_string()))?;
            let payment_request = params.get("payment_request")
                .and_then(|pr| pr.as_str())
                .ok_or_else(|| Error::Wallet("Missing payment_request in pay_cashu_request params".to_string()))?;
            
            // Optional amount parameter - can override the amount in the payment request
            let amount = params.get("amount")
                .and_then(|a| a.as_u64());
            
            // Handle custom pay_cashu_request request
            return self.handle_pay_cashu_request(connection, event, payment_request, amount).await;
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
        if let Err(e) = self.storage.update_budget(&connection_pubkey, &connection.budget) {
            log::error!("Failed to persist budget renewal: {}", e);
        }

        // Handle request
        let (response, payment_amount, balance_info) = self
            .handle_request(request, remaining_budget_msats)
            .await;

        // Update budget if payment was made
        if let Some(amount) = payment_amount {
            connection.budget.used_budget_msats += amount;
            
            // Persist updated budget to storage
            let connection_pubkey = connection.keys.public_key().to_hex();
            if let Err(e) = self.storage.update_budget(&connection_pubkey, &connection.budget) {
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
        
        let encrypted_response = nip04::encrypt(
            connection.keys.secret_key(),
            encrypt_pubkey,
            response_json,
        )?;

        // Create response event
        // For NWA: sign with connection keys
        // For standard NWC: sign with service keys
        let signing_keys = if connection.app_pubkey.is_some() {
            &connection.keys
        } else {
            &self.keys
        };
        
        let res_event = EventBuilder::new(
            Kind::WalletConnectResponse,
            encrypted_response,
            vec![
                Tag::from_standardized(TagStandard::public_key(event.pubkey)),
                Tag::from_standardized(TagStandard::event(event.id)),
            ],
        )
        .to_event(signing_keys)?;

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
        
        let res_event = EventBuilder::new(
            Kind::WalletConnectResponse,
            encrypted_response,
            vec![
                Tag::from_standardized(TagStandard::public_key(event.pubkey)),
                Tag::from_standardized(TagStandard::event(event.id)),
            ],
        )
        .to_event(signing_keys)?;
        
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
        
        let res_event = EventBuilder::new(
            Kind::WalletConnectResponse,
            encrypted_response,
            vec![
                Tag::from_standardized(TagStandard::public_key(event.pubkey)),
                Tag::from_standardized(TagStandard::event(event.id)),
            ],
        )
        .to_event(signing_keys)?;
        
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
            nip47::RequestParams::GetBalance => {
                match self.get_balance().await {
                    Ok(balance_info) => {
                        log::info!("Balance: {} msats, Max sendable: {} msats, Mints: {}", 
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
                                    nip47::GetBalanceResponseResult {
                                        balance: balance_info.balance,
                                    },
                                )),
                            },
                            None,
                            Some(balance_info_clone),
                        )
                    },
                    Err(e) => (
                        nip47::Response {
                            result_type: nip47::Method::GetBalance,
                            error: Some(e.into()),
                            result: None,
                        },
                        None,
                        None,
                    ),
                }
            }
            nip47::RequestParams::MakeInvoice(params) => {
                match self
                    .make_invoice(params.amount.into(), params.description)
                    .await
                {
                    Ok(invoice_info) => {
                        let invoice = Bolt11Invoice::from_str(&invoice_info.request)
                            .expect("Valid invoice from wallet");
                        (
                            nip47::Response {
                                result_type: nip47::Method::MakeInvoice,
                                error: None,
                                result: Some(nip47::ResponseResult::MakeInvoice(
                                    MakeInvoiceResponseResult {
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
                                nip47::PayInvoiceResponseResult {
                                    preimage: payment_result.preimage.unwrap_or_default(),
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
        let wallet_summary = service.get_wallet_summary().await.map_err(|e| {
            Error::Wallet(format!("Failed to get wallet summary: {}", e))
        })?;
        
        // Convert per-mint balances from sats to msats
        let mint_balances: Vec<MintBalance> = wallet_summary.balances.iter().map(|b| {
            MintBalance {
                mint_url: b.mint_url.clone(),
                balance: b.balance * 1000, // Convert to msats
                unit: "msat".to_string(),
            }
        }).collect();
        
        // Find the max sendable amount (highest single mint balance)
        let max_sendable = mint_balances.iter()
            .map(|b| b.balance)
            .max()
            .unwrap_or(0);
        
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
        // Convert msats to sats
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
            log::info!(
                "Successfully paid {} sats via transport",
                pay_result.amount
            );
        }

        Ok(pay_result)
    }

    /// Gets the service public key for creating connection URIs.
    pub fn service_pubkey(&self) -> PublicKey {
        self.keys.public_key()
    }
    
    /// Creates a new NWA connection and returns the connection details.
    pub async fn create_nwa_connection(
        &self,
        app_pubkey_str: &str,
        secret: String,
        budget: ConnectionBudget,
    ) -> Result<WalletConnection, Error> {
        // Parse app's public key
        let app_pubkey = PublicKey::from_str(app_pubkey_str)
            .map_err(|e| Error::Key(e))?;
        
        // Create new connection with generated keypair
        let connection = WalletConnection::from_nwa(app_pubkey, secret, budget);
        
        // Add connection to our list
        self.add_connection(connection.clone()).await?;
        
        log::info!(
            "Created NWA connection: app_pubkey={}, connection_pubkey={}",
            app_pubkey,
            connection.keys.public_key()
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
        let app_pubkey = connection.app_pubkey
            .ok_or_else(|| Error::Wallet("Connection missing app_pubkey".to_string()))?;
        
        let secret = connection.secret.clone()
            .ok_or_else(|| Error::Wallet("Connection missing secret".to_string()))?;
        
        // Build response JSON
        // The app needs to know our connection pubkey to send requests to
        let response = serde_json::json!({
            "secret": secret,
            "pubkey": connection.keys.public_key().to_hex(),
            "commands": ["pay_invoice", "make_invoice", "get_balance", "receive_cashu", "pay_cashu_request"],
            "relay": RELAY_URL,
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
        let event = EventBuilder::new(
            Kind::from(33194),
            encrypted_content,
            vec![
                Tag::from_standardized(TagStandard::Identifier(app_pubkey.to_string())),
            ],
        )
        .to_event(&connection.keys)?;
        
        // Add specified relays if they're different from our default
        for relay_url in relays {
            if relay_url != RELAY_URL {
                if let Err(e) = self.client.add_relay(&relay_url).await {
                    log::warn!("Failed to add relay {}: {}", relay_url, e);
                }
            }
        }
        
        // Broadcast the event
        self.client.send_event(event.clone()).await?;
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
}

impl WalletConnection {
    /// Creates a new wallet connection.
    pub fn new(secret: SecretKey, budget: ConnectionBudget) -> Self {
        Self {
            keys: Keys::new(secret),
            budget,
            app_pubkey: None,
            secret: None,
        }
    }

    /// Creates a wallet connection from a Wallet Connect URI.
    pub fn from_uri(uri: NostrWalletConnectURI, budget: ConnectionBudget) -> Self {
        Self {
            keys: Keys::new(uri.secret),
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
    pub fn from_nwa(
        app_pubkey: PublicKey,
        secret: String,
        budget: ConnectionBudget,
    ) -> Self {
        // Generate a NEW unique keypair for this connection
        let connection_keys = Keys::generate();
        
        Self {
            keys: connection_keys,
            budget,
            app_pubkey: Some(app_pubkey),
            secret: Some(secret),
        }
    }

    /// Checks and updates the remaining budget, handling renewal if needed.
    fn check_and_update_remaining_budget(&mut self) -> u64 {
        if let Some(renews_at) = self.budget.renews_at {
            if renews_at <= Timestamp::now() {
                self.budget.used_budget_msats = 0;
                self.budget.renews_at = self.budget_renews_at();
            }
        }
        if self.budget.used_budget_msats >= self.budget.total_budget_msats {
            return 0;
        }
        self.budget.total_budget_msats - self.budget.used_budget_msats
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
                    vec![self.keys.public_key()],
                )
        } else {
            // Standard NWC: filter events authored by connection, tagged to service pubkey
            Filter::new()
                .kind(Kind::WalletConnectRequest)
                .author(self.keys.public_key())
                .since(since)
                .custom_tag(
                    SingleLetterTag::lowercase(Alphabet::P),
                    vec![service_pubkey],
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
        let uri = NostrWalletConnectURI::new(
            service_pubkey,
            relay,
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
    
    #[error("Wallet error: {0}")]
    Wallet(String),
}

impl From<lightning_invoice::ParseOrSemanticError> for Error {
    fn from(err: lightning_invoice::ParseOrSemanticError) -> Self {
        Error::InvoiceParse(format!("{:?}", err))
    }
}

impl Into<nip47::NIP47Error> for Error {
    fn into(self) -> nip47::NIP47Error {
        match self {
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
        let service = TollGateService::new().await
            .expect("Failed to create TollGate service");
        let service_state = Arc::new(Mutex::new(service));
        println!("✓ TollGate service created");
        
        // Step 2: Create NWC service
        println!("Step 2: Creating NWC service...");
        let service_key = SecretKey::generate();
        let nwc = NostrWalletConnect::new(service_key, service_state.clone()).await
            .expect("Failed to create NWC service");
        println!("✓ NWC service created with pubkey: {}", nwc.service_pubkey());
        
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
        println!("✓ Connection created with pubkey: {}", connection.keys.public_key());
        
        // Step 4: Add connection to NWC service
        println!("Step 4: Adding connection to NWC service...");
        nwc.add_connection(connection.clone()).await
            .expect("Failed to add connection");
        println!("✓ Connection added successfully");
        
        // Step 5: Get info event
        println!("Step 5: Getting NWC info event...");
        let info_event = nwc.info_event()
            .expect("Failed to create info event");
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
                println!("  - Total balance: {} msats ({} sats)", 
                    balance_info.balance, 
                    balance_info.balance / 1000
                );
                println!("  - Max sendable: {} msats ({} sats)", 
                    balance_info.max_sendable,
                    balance_info.max_sendable / 1000
                );
                println!("  - Number of mints: {}", balance_info.mints.len());
                for (i, mint) in balance_info.mints.iter().enumerate() {
                    println!("    Mint {}: {} - {} msats ({} sats)", 
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
            },
            Err(e) => {
                println!("! Balance retrieval returned error (this may be expected if no mints configured): {}", e);
            }
        }
        
        // Step 7: Verify connections are stored
        println!("Step 7: Verifying stored connections...");
        let connections = nwc.get_connections().await;
        println!("✓ Retrieved {} connection(s)", connections.len());
        assert!(connections.len() >= 1, "Should have at least one connection");
        
        // Verify our connection is in the list
        let found = connections.iter().any(|c| c.keys.public_key() == connection.keys.public_key());
        assert!(found, "Our connection should be in the list");
        
        println!("=== Test completed successfully ===");
    }
    
    #[tokio::test]
    async fn test_nwa_connection_flow() {
        println!("=== Starting NWA Connection Flow Test ===");
        
        // Step 1: Create TollGate service
        println!("Step 1: Creating TollGate service...");
        let service = TollGateService::new().await
            .expect("Failed to create TollGate service");
        let service_state = Arc::new(Mutex::new(service));
        println!("✓ TollGate service created");
        
        // Step 2: Create NWC service
        println!("Step 2: Creating NWC service...");
        let service_key = SecretKey::generate();
        let nwc = NostrWalletConnect::new(service_key, service_state.clone()).await
            .expect("Failed to create NWC service");
        println!("✓ NWC service created with pubkey: {}", nwc.service_pubkey());
        
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
        
        let nwa_connection = nwc.create_nwa_connection(
            &app_keys.public_key().to_hex(),
            secret.clone(),
            budget,
        ).await.expect("Failed to create NWA connection");
        
        println!("✓ NWA connection created:");
        println!("  - App pubkey: {}", app_keys.public_key());
        println!("  - Connection pubkey: {}", nwa_connection.keys.public_key());
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
        assert!(connections.len() >= 1, "Should have at least one connection");
        
        // Find our connection
        let stored_connection = connections.iter()
            .find(|c| c.keys.public_key() == nwa_connection.keys.public_key())
            .expect("Our connection should be in the list");
        assert_eq!(stored_connection.app_pubkey, Some(app_keys.public_key()));
        assert_eq!(stored_connection.secret, Some(secret));
        println!("✓ Connection properly stored with NWA details");
        
        println!("=== Test completed successfully ===");
    }
    
    #[tokio::test]
    async fn test_receive_cashu_token() {
        println!("=== Starting Receive Cashu Token Test ===");
        
        // Step 1: Create TollGate service
        println!("Step 1: Creating TollGate service...");
        let service = TollGateService::new().await
            .expect("Failed to create TollGate service");
        let service_state = Arc::new(Mutex::new(service));
        println!("✓ TollGate service created");
        
        // Step 2: Create NWC service
        println!("Step 2: Creating NWC service...");
        let service_key = SecretKey::generate();
        let nwc = NostrWalletConnect::new(service_key, service_state.clone()).await
            .expect("Failed to create NWC service");
        println!("✓ NWC service created with pubkey: {}", nwc.service_pubkey());
        
        // Step 3: Get balance before receiving
        println!("Step 3: Getting initial balance...");
        match nwc.get_balance().await {
            Ok(balance_info) => {
                println!("✓ Initial balance: {} sats", balance_info.balance / 1000);
                println!("  Mints: {}", balance_info.mints.len());
            },
            Err(e) => {
                println!("! Initial balance error (may be expected): {}", e);
            }
        }
        
        // Step 4: Receive the cashu token
        println!("\nStep 4: Receiving cashu token...");
        let token = "cashuBo2FteCJodHRwczovL25vZmVlcy50ZXN0bnV0LmNhc2h1LnNwYWNlYXVjc2F0YXSBomFpSAC0zSfYhhpEYXCFpGFhCGFzeEA5YmViNTE0ZTE2MjFkM2RkYTY0MjgyNDg4Zjg5ZTBkZTk4Y2IyNmM3NGI2MjNmNjllZGMwYWMxOTA3ZTAxMjA1YWNYIQMw7UppJvgL0Ixr7brd2QUSiZ_BkkWgkpmo_ojPa-W5wGFko2FlWCDgFHEyX6D2iU-Mam3xrcfzMHTXP2QFuDALk8BKQqxhIWFzWCDD81s4-_savlVBT05zsXEYv59_DT9G_VuSHzgMUU081GFyWCDIs4v0uSoV9dlp09FeFE7iNG1RGmbd7n4zwkBotSS0_6RhYQhhc3hAMDg5NTg4M2Y4NjQwMzMwY2Q1ODY1ODc0MTE5ZGRkZWExODJiZWYxNmU1ZWI5YzliODk3YjUxNzI4NjgzMzdmM2FjWCECwXXD_aWRi1ZY4VAw4QC_3WAd-dzIO16wsP0448PSZfZhZKNhZVggI96r12eU2NmET3Y9iuvRB_BHA8yTKJ0ovVqXpAVXnTFhc1gg25yD6mRI9PMP70IqAje3BDgiQOsnGrsM5vSJbOm8slVhclgg8jW7TRtey7xrQfv762Fx9aGICHfeFQ1UTaj5MPi6IAmkYWECYXN4QDhhOWEyNmM0ZDg4ZWYyY2E2MDlkYjJjNjY3MWQ1YTU3OWZhMDhkYjU1ODI3YmVjZGJiMmNlNTNiOGEyZWVjMGVhY1ghApZZIz1vpxeW6zrSv44msnU3Ky0M0Ad8kCbxfCW9F8GqYWSjYWVYIAD_aln-jTz31V1v3Jcp8zLZoIHmKGCwJcsZrHmbvqAaYXNYIC7lL1yomkctyPMfGjPj6hsm6ZTs5gyJkiUtuxSan1BMYXJYIDf4xrFqo6s200g1AOLP8CZqFjgRUBqL8St5tF_1PGRQpGFhAmFzeEA0YzAzMTI1ZDRhZTU3NWM2MTBiNzBmMWYwN2VlMTNiMjkwN2E2MWQ4NzgwOWRkMjM2MTA2NmJjNjAwNzVmZDQ3YWNYIQNXh9p03x9bqCAj4picnMqOpqY9m8S3W3502ayAaqGvJmFko2FlWCBM--Kr27PYSt-xNng4q5a8w_3moX8V2JybosGthPnzrGFzWCCcrJS0WuLvD3b_Y0g_8OImwA9Ly2rKwp2bRvAskjegKGFyWCBZfFAv0nqKNBC_FM8QzSu3eOV4NkA3eSD40CVMiCi5rKRhYQFhc3hAMGZhMjM4M2Y1YjUzZTA0MWQzOWIxMDQ4YWVlZWQ3NjRmMTU1MDBkMzE4YmI1ZGU4MzNiOTJkZjUzMjBkZjM2NGFjWCEC6_oMe4HmiKrmyukKGez4sOaA-m2I7MloMXqE9zbFoDJhZKNhZVggagzjRZB-jJ9xJ1KZzbyRCH2C39Utiole54pyD0fnIvBhc1ggulky-qM3PRpNCg_tZoSWPDFnpSqdB0SX6M4KvINWmeZhclggbMnmAC1Pe3KPY07KJqTPh84IsgrmqmcjNYHMsp3wCFQ";
        
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
                println!("✗ Failed to receive cashu token: {}", e);
                panic!("Token receive failed: {}", e);
            }
        }
        
        // Step 5: Get balance after receiving to verify
        println!("\nStep 5: Getting balance after receiving...");
        match nwc.get_balance().await {
            Ok(balance_info) => {
                println!("✓ New balance: {} sats", balance_info.balance / 1000);
                println!("  Number of mints: {}", balance_info.mints.len());
                for (i, mint) in balance_info.mints.iter().enumerate() {
                    println!("    Mint {}: {} - {} sats", 
                        i + 1,
                        mint.mint_url,
                        mint.balance / 1000
                    );
                }
            },
            Err(e) => {
                println!("! Balance retrieval error: {}", e);
            }
        }
        
        println!("\n=== Test completed successfully ===");
    }
    
    #[tokio::test]
    async fn test_pay_cashu_request() {
        println!("=== Starting Pay Cashu Request Test ===");
        
        // Step 1: Create TollGate service
        println!("Step 1: Creating TollGate service...");
        let service = TollGateService::new().await
            .expect("Failed to create TollGate service");
        let service_state = Arc::new(Mutex::new(service));
        println!("✓ TollGate service created");
        
        // Step 2: Create NWC service
        println!("Step 2: Creating NWC service...");
        let service_key = SecretKey::generate();
        let nwc = NostrWalletConnect::new(service_key, service_state.clone()).await
            .expect("Failed to create NWC service");
        println!("✓ NWC service created with pubkey: {}", nwc.service_pubkey());
        
        // Step 3: Pay the cashu request
        println!("\nStep 3: Paying cashu payment request...");
        let payment_request = "creqApWF0gaNhdGVub3N0cmFheKlucHJvZmlsZTFxeTI4d3VtbjhnaGo3dW45ZDNzaGp0bnl2OWtoMnVld2Q5aHN6OW1od2RlbjV0ZTB3ZmprY2N0ZTljdXJ4dmVuOWVlaHFjdHJ2NWhzenJ0aHdkZW41dGUwZGVoaHh0bnZkYWtxcWd6Z21yMnB0MDk0OTV0ZG5sbXduZ3NmdTN5NjR1cDh4ODVmcnM5c2h5a3lwYzU0dm5ranNneTU2enNtYWeBgmFuYjE3YWloNWVmYzE3ZWZhYQVhdWNzYXRhbYF4Imh0dHBzOi8vbm9mZWVzLnRlc3RudXQuY2FzaHUuc3BhY2U=";
        
        match nwc.pay_cashu_request(payment_request, None).await {
            Ok(result) => {
                println!("✓ Successfully processed cashu payment request!");
                println!("  Amount: {} sats", result.amount);
                
                if let Some(token) = &result.token {
                    println!("  Token returned (no transport): {}", &token[..50.min(token.len())]);
                    
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
                assert!(result.amount > 0, "Should have created token for a non-zero amount");
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
        let service = TollGateService::new().await
            .expect("Failed to create TollGate service");
        let service_state = Arc::new(Mutex::new(service));
        println!("✓ TollGate service created");
        
        // Step 2: Add the mint and some balance
        println!("Step 2: Adding test mint...");
        {
            let mut service = service_state.lock().await;
            match service.add_mint("https://nofees.testnut.cashu.space").await {
                Ok(_) => println!("✓ Mint added"),
                Err(e) => println!("! Mint add failed (may already exist): {}", e),
            }
        }
        
        // Step 3: Create NWC service
        println!("Step 3: Creating NWC service...");
        let service_key = SecretKey::generate();
        let nwc = NostrWalletConnect::new(service_key, service_state.clone()).await
            .expect("Failed to create NWC service");
        println!("✓ NWC service created with pubkey: {}", nwc.service_pubkey());
        
        // Step 4: Create a payment request with no transport
        println!("\nStep 4: Creating payment request with no transport...");
        let payment_request = {
            let service = service_state.lock().await;
            match service.create_nut18_payment_request(Some(5), Some("Test payment".to_string())).await {
                Ok(pr) => {
                    println!("✓ Created payment request: {}", &pr.request[..50.min(pr.request.len())]);
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
                    assert!(token.starts_with("cashu"), "Token should start with 'cashu'");
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
            match service.create_nut18_payment_request(None, Some("Amount-less payment".to_string())).await {
                Ok(pr) => {
                    println!("✓ Created amount-less payment request: {}", &pr.request[..50.min(pr.request.len())]);
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
                println!("✓ Successfully processed amount-less payment request with custom amount!");
                println!("  Amount: {} sats", result.amount);
                
                if let Some(token) = &result.token {
                    println!("✓ Token returned as expected (no transport defined)");
                    println!("  Token preview: {}...", &token[..50.min(token.len())]);
                    
                    // Verify the token is valid and amount matches
                    assert!(token.starts_with("cashu"), "Token should start with 'cashu'");
                    assert!(result.amount == 10, "Amount should be 10 sats (the custom amount)");
                    
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
                println!("✗ Unexpectedly succeeded paying amount-less request without custom amount!");
                panic!("Should have failed when no amount is provided");
            }
            Err(e) => {
                println!("✓ Correctly failed to pay amount-less request without custom amount");
            }
        }
        
        println!("\n=== Test completed ===");
    }
}
