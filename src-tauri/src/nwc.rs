//! Nostr Wallet Connect (NIP-47) implementation for the TollGate wallet.
//!
//! This module provides NWC functionality that allows external applications
//! to interact with the wallet through Nostr relays.

use crate::nwc_storage::NwcConnectionStorage;
use crate::tollgate::wallet::{Bolt11InvoiceInfo, Bolt11PaymentResult};
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
            "get_balance make_invoice pay_invoice",
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
        
        let request = nip47::Request::from_json(nip04::decrypt(
            connection.keys.secret_key(),
            decrypt_pubkey,
            &event.content,
        )?)?;

        // Check budget
        let remaining_budget_msats = connection.check_and_update_remaining_budget();
        
        // If budget was renewed, persist the update
        let connection_pubkey = connection.keys.public_key().to_hex();
        if let Err(e) = self.storage.update_budget(&connection_pubkey, &connection.budget) {
            log::error!("Failed to persist budget renewal: {}", e);
        }

        // Handle request
        let (response, payment_amount) = self
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
        
        let encrypted_response = nip04::encrypt(
            connection.keys.secret_key(),
            encrypt_pubkey,
            response.as_json(),
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

    /// Handles a NIP-47 request and routes it to wallet methods.
    async fn handle_request(
        &self,
        request: nip47::Request,
        remaining_budget_msats: u64,
    ) -> (nip47::Response, Option<u64>) {
        match request.params {
            nip47::RequestParams::GetBalance => {
                match self.get_balance().await {
                    Ok(balance_msats) => (
                        nip47::Response {
                            result_type: nip47::Method::GetBalance,
                            error: None,
                            result: Some(nip47::ResponseResult::GetBalance(
                                nip47::GetBalanceResponseResult {
                                    balance: balance_msats,
                                },
                            )),
                        },
                        None,
                    ),
                    Err(e) => (
                        nip47::Response {
                            result_type: nip47::Method::GetBalance,
                            error: Some(e.into()),
                            result: None,
                        },
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
                        )
                    }
                    Err(e) => (
                        nip47::Response {
                            result_type: nip47::Method::MakeInvoice,
                            error: Some(e.into()),
                            result: None,
                        },
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
                    ),
                    Err(e) => (
                        nip47::Response {
                            result_type: nip47::Method::PayInvoice,
                            error: Some(e.into()),
                            result: None,
                        },
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
            ),
        }
    }

    /// Gets the wallet balance in millisatoshis.
    async fn get_balance(&self) -> Result<u64, Error> {
        let service = self.service_state.lock().await;
        let balance_sats = service.get_wallet_balance().await.map_err(|e| {
            Error::Wallet(format!("Failed to get wallet balance: {}", e))
        })?;
        // Convert sats to msats
        Ok(balance_sats * 1000)
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
        let app_pubkey = PublicKey::from_str(app_pubkey_str)
            .map_err(|e| Error::Key(e))?;
        
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
        let app_pubkey = connection.app_pubkey
            .ok_or_else(|| Error::Wallet("Connection missing app_pubkey".to_string()))?;
        
        let secret = connection.secret.clone()
            .ok_or_else(|| Error::Wallet("Connection missing secret".to_string()))?;
        
        // Build response JSON
        // The app needs to know our connection pubkey to send requests to
        let response = serde_json::json!({
            "secret": secret,
            "pubkey": connection.keys.public_key().to_hex(),
            "commands": ["pay_invoice", "make_invoice", "get_balance"],
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

