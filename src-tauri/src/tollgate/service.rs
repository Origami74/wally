//! Main TollGate service coordinator
//!
//! This is the central service that manages all TollGate operations:
//! - Background monitoring and purchasing
//! - Session management and persistence
//! - Network detection and auto-connection
//! - Wallet integration and payments

use crate::tollgate::errors::{TollGateError, TollGateResult};
use crate::tollgate::network::{NetworkDetector, NetworkInfo};
use crate::tollgate::protocol::{PaymentEvent, TollGateProtocol};
use crate::tollgate::session::{Session, SessionManager, SessionStatus};
use crate::tollgate::wallet::{
    Bolt11InvoiceInfo, Bolt11PaymentResult, CashuReceiveResult, Nut18PaymentRequestInfo,
    PayNut18Result, TollGateWallet, WalletSummary, WalletTransactionEntry,
};
use cdk::amount::SplitTarget;
use chrono::{DateTime, Utc};
use nostr::Keys;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::interval;

/// Service status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub auto_tollgate_enabled: bool,
    pub current_network: Option<NetworkInfo>,
    pub active_sessions: Vec<SessionInfo>,
    pub wallet_balance: u64,
    pub last_check: DateTime<Utc>,
}

/// Session information for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub tollgate_pubkey: String,
    pub gateway_ip: String,
    pub status: SessionStatus,
    pub usage_percentage: f64,
    pub remaining_time_seconds: Option<i64>,
    pub remaining_data_bytes: Option<u64>,
    pub total_spent: u64,
}

/// Main TollGate service
pub struct TollGateService {
    /// Global enable/disable state
    auto_tollgate_enabled: Arc<RwLock<bool>>,
    /// Session manager
    session_manager: Arc<Mutex<SessionManager>>,
    /// Cashu wallet
    wallet: Arc<Mutex<TollGateWallet>>,
    /// Network detector
    network_detector: NetworkDetector,
    /// Protocol handler
    protocol: TollGateProtocol,
    /// Current network information
    current_network: Arc<RwLock<Option<NetworkInfo>>>,
    /// Background task handle
    background_task: Option<tokio::task::JoinHandle<()>>,
}

impl TollGateService {
    /// Create a new TollGate service
    pub async fn new() -> TollGateResult<Self> {
        let mut wallet = TollGateWallet::new()?;

        // Load existing mints from previous sessions
        wallet.load_existing_mints().await?;

        let service = Self {
            auto_tollgate_enabled: Arc::new(RwLock::new(false)),
            session_manager: Arc::new(Mutex::new(SessionManager::new())),
            wallet: Arc::new(Mutex::new(wallet)),
            network_detector: NetworkDetector::new(),
            protocol: TollGateProtocol::new(),
            current_network: Arc::new(RwLock::new(None)),
            background_task: None,
        };

        // Load persisted state
        service.load_persisted_state().await?;

        Ok(service)
    }

    /// Start the background monitoring service
    pub async fn start_background_service(&mut self) -> TollGateResult<()> {
        if self.background_task.is_some() {
            return Ok(()); // Already running
        }

        let auto_enabled = self.auto_tollgate_enabled.clone();
        let session_manager = self.session_manager.clone();
        let wallet = self.wallet.clone();
        let current_network = self.current_network.clone();
        let protocol = self.protocol.clone();

        let task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));

            loop {
                interval.tick().await;

                // Only run if auto-tollgate is enabled
                if !*auto_enabled.read().await {
                    continue;
                }

                // Check all active sessions for renewal needs
                if let Err(e) = Self::check_sessions_for_renewal(
                    &session_manager,
                    &wallet,
                    &current_network,
                    &protocol,
                )
                .await
                {
                    log::error!("Error checking sessions for renewal: {}", e);
                }

                // Update time-based usage for all sessions
                {
                    let mut manager = session_manager.lock().await;
                    manager.update_time_based_usage();
                    manager.cleanup_expired_sessions();
                }

                // Persist state periodically
                if let Err(e) = Self::persist_state(&session_manager).await {
                    log::error!("Error persisting state: {}", e);
                }
            }
        });

        self.background_task = Some(task);
        log::info!("Background service started");
        Ok(())
    }

    /// Stop the background service
    #[allow(dead_code)]
    pub async fn stop_background_service(&mut self) {
        if let Some(task) = self.background_task.take() {
            task.abort();
            log::info!("Background service stopped");
        }
    }

    /// Enable or disable auto-tollgate functionality
    pub async fn set_auto_tollgate_enabled(&self, enabled: bool) -> TollGateResult<()> {
        *self.auto_tollgate_enabled.write().await = enabled;
        log::info!(
            "Auto-tollgate {}",
            if enabled { "enabled" } else { "disabled" }
        );
        Ok(())
    }

    /// Get current service status
    pub async fn get_status(&self) -> TollGateResult<ServiceStatus> {
        let auto_enabled = *self.auto_tollgate_enabled.read().await;
        let current_network = self.current_network.read().await.clone();
        let session_manager = self.session_manager.lock().await;
        let wallet = self.wallet.lock().await;

        let active_sessions: Vec<SessionInfo> = session_manager
            .get_active_sessions()
            .iter()
            .map(|session| SessionInfo {
                id: session.id.clone(),
                tollgate_pubkey: session.tollgate_pubkey.clone(),
                gateway_ip: session.gateway_ip.clone(),
                status: session.status.clone(),
                usage_percentage: session.usage_percentage(),
                remaining_time_seconds: session.remaining_time_seconds(),
                remaining_data_bytes: session.remaining_data_bytes(),
                total_spent: session.total_spent,
            })
            .collect();

        let wallet_balance = wallet
            .get_all_balances()
            .await
            .unwrap_or_default()
            .iter()
            .map(|b| b.balance)
            .sum();

        Ok(ServiceStatus {
            auto_tollgate_enabled: auto_enabled,
            current_network,
            active_sessions,
            wallet_balance,
            last_check: Utc::now(),
        })
    }

    /// Handle network connection event
    pub async fn handle_network_connected(
        &self,
        gateway_ip: String,
        mac_address: String,
    ) -> TollGateResult<()> {
        log::info!(
            "Network connected: gateway={}, mac={}",
            gateway_ip,
            mac_address
        );

        // Detect if this is a TollGate network
        let network_info = self
            .network_detector
            .detect_tollgate(&gateway_ip, &mac_address)
            .await?;

        // Update current network
        *self.current_network.write().await = Some(network_info.clone());

        if !network_info.is_tollgate {
            log::debug!("Network {} is not a TollGate", gateway_ip);
            return Ok(());
        }

        log::info!("TollGate detected on network {}", gateway_ip);

        // If auto-tollgate is enabled, start a session
        if *self.auto_tollgate_enabled.read().await {
            if let Some(advertisement) = &network_info.advertisement.clone() {
                self.start_tollgate_session(network_info, advertisement.clone())
                    .await?;
            }
        }

        Ok(())
    }

    /// Handle network disconnection event
    pub async fn handle_network_disconnected(&self) -> TollGateResult<()> {
        log::info!("Network disconnected");

        // Clear current network
        *self.current_network.write().await = None;

        // Mark all sessions as expired (they'll be cleaned up by background service)
        let mut session_manager = self.session_manager.lock().await;
        for session in session_manager.get_all_sessions_mut() {
            if session.is_active() {
                session.status = SessionStatus::Expired;
                log::info!(
                    "Marked session {} as expired due to network disconnect",
                    session.id
                );
            }
        }

        Ok(())
    }

    /// Start a new TollGate session
    async fn start_tollgate_session(
        &self,
        network_info: NetworkInfo,
        advertisement: crate::tollgate::protocol::TollGateAdvertisement,
    ) -> TollGateResult<()> {
        let mut session_manager = self.session_manager.lock().await;
        let wallet = self.wallet.lock().await;

        // Check if we already have an active session for this TollGate
        if let Some(existing_session) = session_manager.get_session(&advertisement.tollgate_pubkey)
        {
            if existing_session.is_active() {
                log::info!(
                    "Already have active session for TollGate {}",
                    advertisement.tollgate_pubkey
                );
                return Ok(());
            }
        }

        // Calculate initial purchase (minimum steps or 5 minutes, whichever is larger)
        let min_steps = advertisement
            .pricing_options
            .iter()
            .map(|opt| opt.min_steps)
            .min()
            .unwrap_or(1);

        let five_minutes_steps = if advertisement.metric == "milliseconds" {
            300000 / advertisement.step_size // 5 minutes in milliseconds
        } else {
            1024 * 1024 * 10 / advertisement.step_size // 10MB for data-based
        };

        let initial_steps = min_steps.max(five_minutes_steps);

        // Select best pricing option
        let pricing_option = wallet
            .select_best_pricing_option(&advertisement.pricing_options, initial_steps)
            .await?;

        // Create payment token
        let payment_token = wallet
            .create_payment_token(&pricing_option, initial_steps)
            .await?;

        // Get device identifier from TollGate
        let (device_type, device_value) = self
            .protocol
            .get_device_identifier(&network_info.gateway_ip)
            .await?;

        // Create payment event
        let customer_keys = Keys::generate();
        let payment = PaymentEvent {
            tollgate_pubkey: advertisement.tollgate_pubkey.clone(),
            mac_address: device_value.clone(),
            cashu_token: payment_token.token.clone(),
            steps: initial_steps,
        };

        let payment_event = self
            .protocol
            .create_payment_event(&payment, &customer_keys, &device_type, &device_value)
            .await?;

        // Send payment and get session response
        let session_response = self
            .protocol
            .send_payment(&network_info.gateway_ip, &payment_event)
            .await?;

        // Calculate allotment and session end
        let allotment = self
            .protocol
            .calculate_allotment(initial_steps, advertisement.step_size);
        let cost = self.protocol.calculate_cost(&pricing_option, initial_steps);

        // Create session
        let mut session = Session::new(
            advertisement.tollgate_pubkey.clone(),
            network_info.gateway_ip.clone(),
            device_value,
            pricing_option,
            advertisement.clone(),
            allotment,
            session_response.session_end,
            cost,
        )?;

        // Update session with response
        session.update_from_response(&session_response)?;

        // Add to session manager
        session_manager.add_session(session);

        log::info!(
            "Successfully started TollGate session for {}",
            advertisement.tollgate_pubkey
        );
        Ok(())
    }

    /// Check all sessions for renewal needs (background task)
    async fn check_sessions_for_renewal(
        session_manager: &Arc<Mutex<SessionManager>>,
        wallet: &Arc<Mutex<TollGateWallet>>,
        _current_network: &Arc<RwLock<Option<NetworkInfo>>>,
        protocol: &TollGateProtocol,
    ) -> TollGateResult<()> {
        let sessions_needing_renewal: Vec<Session> = {
            let manager = session_manager.lock().await;
            manager
                .get_sessions_needing_renewal()
                .into_iter()
                .cloned()
                .collect()
        };

        for session in sessions_needing_renewal {
            if let Err(e) =
                Self::renew_session(session_manager, wallet, protocol, &session.tollgate_pubkey)
                    .await
            {
                log::error!("Failed to renew session {}: {}", session.id, e);

                // Mark session as error
                let mut manager = session_manager.lock().await;
                if let Some(session) = manager.get_session_mut(&session.tollgate_pubkey) {
                    session.set_error(format!("Renewal failed: {}", e));
                }
            }
        }

        Ok(())
    }

    /// Renew a specific session
    async fn renew_session(
        session_manager: &Arc<Mutex<SessionManager>>,
        wallet: &Arc<Mutex<TollGateWallet>>,
        protocol: &TollGateProtocol,
        tollgate_pubkey: &str,
    ) -> TollGateResult<()> {
        let (session_clone, renewal_steps) = {
            let manager = session_manager.lock().await;
            let session = manager
                .get_session(tollgate_pubkey)
                .ok_or_else(|| TollGateError::session("Session not found for renewal"))?;

            // Calculate renewal steps (same as initial or based on remaining time)
            let renewal_steps = if session.advertisement.metric == "milliseconds" {
                300000 / session.advertisement.step_size // 5 more minutes
            } else {
                1024 * 1024 * 5 / session.advertisement.step_size // 5MB more
            };

            (session.clone(), renewal_steps)
        };

        // Mark session as renewing
        {
            let mut manager = session_manager.lock().await;
            if let Some(session) = manager.get_session_mut(tollgate_pubkey) {
                session.status = SessionStatus::Renewing;
            }
        }

        // Create renewal payment
        let wallet_guard = wallet.lock().await;
        let payment_token = wallet_guard
            .create_payment_token(&session_clone.pricing_option, renewal_steps)
            .await?;
        drop(wallet_guard);

        // Get device identifier
        let (device_type, device_value) = protocol
            .get_device_identifier(&session_clone.gateway_ip)
            .await?;

        // Create payment event
        let customer_keys = session_clone.get_customer_keys()?;
        let payment = PaymentEvent {
            tollgate_pubkey: session_clone.tollgate_pubkey.clone(),
            mac_address: device_value.clone(),
            cashu_token: payment_token.token,
            steps: renewal_steps,
        };

        let payment_event = protocol
            .create_payment_event(&payment, &customer_keys, &device_type, &device_value)
            .await?;

        // Send renewal payment
        let session_response = protocol
            .send_payment(&session_clone.gateway_ip, &payment_event)
            .await?;

        // Update session with renewal
        {
            let mut manager = session_manager.lock().await;
            if let Some(session) = manager.get_session_mut(tollgate_pubkey) {
                let additional_allotment =
                    protocol.calculate_allotment(renewal_steps, session.advertisement.step_size);
                let additional_cost =
                    protocol.calculate_cost(&session.pricing_option, renewal_steps);

                session.mark_renewed(additional_allotment, additional_cost);
                session.session_end = session_response.session_end;

                log::info!(
                    "Successfully renewed session {} with {} additional allotment",
                    session.id,
                    additional_allotment
                );
            }
        }

        Ok(())
    }

    /// Force renewal of current session
    pub async fn force_renewal(&self, tollgate_pubkey: &str) -> TollGateResult<()> {
        Self::renew_session(
            &self.session_manager,
            &self.wallet,
            &self.protocol,
            tollgate_pubkey,
        )
        .await
    }

    /// Get current session information
    pub async fn get_current_session(&self) -> TollGateResult<Option<SessionInfo>> {
        let manager = self.session_manager.lock().await;
        let active_sessions = manager.get_active_sessions();

        // Return the most recent active session
        let session_info = active_sessions.first().map(|session| SessionInfo {
            id: session.id.clone(),
            tollgate_pubkey: session.tollgate_pubkey.clone(),
            gateway_ip: session.gateway_ip.clone(),
            status: session.status.clone(),
            usage_percentage: session.usage_percentage(),
            remaining_time_seconds: session.remaining_time_seconds(),
            remaining_data_bytes: session.remaining_data_bytes(),
            total_spent: session.total_spent,
        });

        Ok(session_info)
    }

    /// Add a mint to the wallet
    pub async fn add_mint(&self, mint_url: &str) -> TollGateResult<()> {
        let mut wallet = self.wallet.lock().await;
        wallet.add_mint(mint_url).await
    }

    /// Get wallet balance
    pub async fn get_wallet_balance(&self) -> TollGateResult<u64> {
        let wallet = self.wallet.lock().await;
        let balances = wallet.get_all_balances().await?;
        Ok(balances.iter().map(|b| b.balance).sum())
    }

    /// Get wallet summary including balances and metadata
    pub async fn get_wallet_summary(&self) -> TollGateResult<WalletSummary> {
        let wallet = self.wallet.lock().await;
        wallet.summary().await
    }

    /// Get the wallet's public key in hex format
    pub async fn get_pubkey_hex(&self) -> String {
        let wallet = self.wallet.lock().await;
        wallet.nostr_pubkey_hex()
    }

    /// List wallet transactions across mints
    pub async fn list_wallet_transactions(&self) -> TollGateResult<Vec<WalletTransactionEntry>> {
        let wallet = self.wallet.lock().await;
        wallet.list_transactions(None).await
    }

    /// Create a Nut18 payment request
    pub async fn create_nut18_payment_request(
        &self,
        amount: Option<u64>,
        description: Option<String>,
    ) -> TollGateResult<Nut18PaymentRequestInfo> {
        let wallet = self.wallet.lock().await;
        wallet.create_nut18_payment_request(amount, description)
    }

    /// Create a BOLT11 invoice
    pub async fn create_bolt11_invoice(
        &self,
        amount: u64,
        description: Option<String>,
    ) -> TollGateResult<Bolt11InvoiceInfo> {
        let wallet = self.wallet.lock().await;
        let invoice = wallet.create_bolt11_invoice(amount, description).await?;
        drop(wallet);

        self.spawn_mint_quote_monitor(
            invoice.mint_url.clone(),
            invoice.quote_id.clone(),
            invoice.expiry,
        );

        Ok(invoice)
    }

    /// Check a mint quote status and mint tokens if paid.
    pub async fn check_mint_quote(&self, mint_url: &str, quote_id: &str) -> TollGateResult<bool> {
        let wallet = self.wallet.lock().await;
        wallet.check_mint_quote(mint_url, quote_id).await
    }

    fn spawn_mint_quote_monitor(&self, mint_url: String, quote_id: String, expiry: u64) {
        let wallet = self.wallet.clone();

        tokio::spawn(async move {
            let poll_interval = Duration::from_secs(3);

            loop {
                let now = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                    Ok(duration) => duration.as_secs(),
                    Err(_) => 0,
                };

                if now >= expiry {
                    log::warn!(
                        "Mint quote {} for mint {} expired before payment was detected",
                        quote_id,
                        mint_url
                    );
                    break;
                }

                let mint_wallet = {
                    let guard = wallet.lock().await;
                    guard.clone_wallet_for_mint(&mint_url)
                };

                let Some(mint_wallet) = mint_wallet else {
                    log::warn!(
                        "Mint {} no longer available while monitoring quote {}",
                        mint_url,
                        quote_id
                    );
                    break;
                };

                match mint_wallet.mint_quote_state(&quote_id).await {
                    Ok(status) => {
                        if status.state == cdk::nuts::MintQuoteState::Paid {
                            match mint_wallet
                                .mint(&status.quote, SplitTarget::default(), None)
                                .await
                            {
                                Ok(_) => {
                                    log::info!(
                                        "Minted tokens for quote {} at mint {}",
                                        quote_id,
                                        mint_url
                                    );
                                    break;
                                }
                                Err(err) => {
                                    log::error!(
                                        "Failed to mint tokens for quote {} at mint {}: {}",
                                        quote_id,
                                        mint_url,
                                        err
                                    );
                                }
                            }
                        }
                    }
                    Err(err) => {
                        log::warn!(
                            "Failed to check quote {} at mint {}: {}",
                            quote_id,
                            mint_url,
                            err
                        );
                    }
                }

                tokio::time::sleep(poll_interval).await;
            }
        });
    }

    /// Pay a Nut18 payment request
    pub async fn pay_nut18_payment_request(
        &self,
        request: &str,
        custom_amount: Option<u64>,
    ) -> TollGateResult<()> {
        let wallet = self.wallet.lock().await;
        wallet
            .pay_nut18_payment_request(request, custom_amount)
            .await
    }

    /// Pay a Nut18 payment request, returning a Token if no transport is defined
    pub async fn pay_nut18_payment_request_with_token(
        &self,
        request: &str,
        custom_amount: Option<u64>,
    ) -> TollGateResult<PayNut18Result> {
        let wallet = self.wallet.lock().await;
        wallet
            .pay_nut18_payment_request_with_token(request, custom_amount)
            .await
    }

    /// Pay a BOLT11 invoice
    pub async fn pay_bolt11_invoice(&self, invoice: &str) -> TollGateResult<Bolt11PaymentResult> {
        let wallet = self.wallet.lock().await;
        wallet.pay_bolt11_invoice(invoice).await
    }

    /// Receive a cashu token
    pub async fn receive_cashu_token(&self, token: &str) -> TollGateResult<CashuReceiveResult> {
        let mut wallet = self.wallet.lock().await;
        wallet.receive_cashu_token(token).await
    }

    /// Detect if current network is a TollGate
    pub async fn detect_tollgate(
        &self,
        gateway_ip: &str,
        mac_address: &str,
    ) -> TollGateResult<NetworkInfo> {
        self.network_detector
            .detect_tollgate(gateway_ip, mac_address)
            .await
    }

    /// Get all active sessions
    pub async fn get_active_sessions(&self) -> TollGateResult<Vec<SessionInfo>> {
        let manager = self.session_manager.lock().await;
        let active_sessions: Vec<SessionInfo> = manager
            .get_active_sessions()
            .iter()
            .map(|session| SessionInfo {
                id: session.id.clone(),
                tollgate_pubkey: session.tollgate_pubkey.clone(),
                gateway_ip: session.gateway_ip.clone(),
                status: session.status.clone(),
                usage_percentage: session.usage_percentage(),
                remaining_time_seconds: session.remaining_time_seconds(),
                remaining_data_bytes: session.remaining_data_bytes(),
                total_spent: session.total_spent,
            })
            .collect();
        Ok(active_sessions)
    }

    /// Get the wallet's Nostr keys
    pub async fn get_wallet_keys(&self) -> nostr::Keys {
        let wallet = self.wallet.lock().await;
        wallet.get_keys()
    }

    /// Load persisted state from storage
    async fn load_persisted_state(&self) -> TollGateResult<()> {
        // TODO: Implement persistence loading from file/database
        // For now, just log that we're loading state
        log::info!("Loading persisted state (not implemented yet)");
        Ok(())
    }

    /// Persist current state to storage
    async fn persist_state(session_manager: &Arc<Mutex<SessionManager>>) -> TollGateResult<()> {
        // TODO: Implement persistence saving to file/database
        let manager = session_manager.lock().await;
        let _serialized = manager.serialize()?;

        // For now, just log that we're persisting state
        log::debug!("Persisting state with {} sessions", manager.session_count());
        Ok(())
    }
}

impl Drop for TollGateService {
    fn drop(&mut self) {
        // Stop background service when service is dropped
        if let Some(task) = self.background_task.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_service_creation() {
        let service = TollGateService::new().await;
        assert!(service.is_ok());
    }

    #[tokio::test]
    async fn test_auto_tollgate_toggle() {
        let service = TollGateService::new().await.unwrap();

        // Initially disabled
        let status = service.get_status().await.unwrap();
        assert!(!status.auto_tollgate_enabled);

        // Enable
        service.set_auto_tollgate_enabled(true).await.unwrap();
        let status = service.get_status().await.unwrap();
        assert!(status.auto_tollgate_enabled);

        // Disable
        service.set_auto_tollgate_enabled(false).await.unwrap();
        let status = service.get_status().await.unwrap();
        assert!(!status.auto_tollgate_enabled);
    }
}
