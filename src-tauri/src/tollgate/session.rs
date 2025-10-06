//! Session management for TollGate connections
//!
//! Handles individual TollGate sessions including:
//! - Session lifecycle management
//! - Usage tracking and renewal
//! - Persistence across app restarts

use crate::tollgate::errors::{TollGateError, TollGateResult};
use crate::tollgate::protocol::{PricingOption, SessionResponse, TollGateAdvertisement};
use chrono::{DateTime, Utc};
use nostr::Keys;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Parameters for creating a new session
pub struct SessionParams {
    pub tollgate_pubkey: String,
    pub gateway_ip: String,
    pub mac_address: String,
    pub pricing_option: PricingOption,
    pub advertisement: TollGateAdvertisement,
    pub initial_allotment: u64,
    pub session_end: DateTime<Utc>,
    pub initial_cost: u64,
}

/// Session status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionStatus {
    /// Session is being initialized
    Initializing,
    /// Session is active and monitoring usage
    Active,
    /// Session is being renewed
    Renewing,
    /// Session has expired
    Expired,
    /// Session encountered an error
    Error(String),
}

/// Individual TollGate session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session identifier
    pub id: String,
    /// TollGate's public key
    pub tollgate_pubkey: String,
    /// Gateway IP address
    pub gateway_ip: String,
    /// Client MAC address
    pub mac_address: String,
    /// Current session status
    pub status: SessionStatus,
    /// Customer's private key for this session
    pub customer_keys: String, // Serialized Keys
    /// Selected pricing option
    pub pricing_option: PricingOption,
    /// Advertisement information
    pub advertisement: TollGateAdvertisement,
    /// Total allotment purchased (in metric units)
    pub total_allotment: u64,
    /// Current usage (in metric units)
    pub current_usage: u64,
    /// When session expires
    pub session_end: DateTime<Utc>,
    /// Renewal threshold (0.8 = 80%)
    pub renewal_threshold: f64,
    /// When session was created
    pub created_at: DateTime<Utc>,
    /// Last renewal time
    pub last_renewal: Option<DateTime<Utc>>,
    /// Total amount spent (in sats)
    pub total_spent: u64,
    /// Number of payments made
    pub payment_count: u32,
}

impl Session {
    /// Create a new session
    pub fn new(params: SessionParams) -> TollGateResult<Self> {
        let customer_keys = Keys::generate();

        Ok(Session {
            id: Uuid::new_v4().to_string(),
            tollgate_pubkey: params.tollgate_pubkey,
            gateway_ip: params.gateway_ip,
            mac_address: params.mac_address,
            status: SessionStatus::Initializing,
            customer_keys: customer_keys.secret_key().to_secret_hex(),
            pricing_option: params.pricing_option,
            advertisement: params.advertisement,
            total_allotment: params.initial_allotment,
            current_usage: 0,
            session_end: params.session_end,
            renewal_threshold: 0.8, // 80%
            created_at: Utc::now(),
            last_renewal: None,
            total_spent: params.initial_cost,
            payment_count: 1,
        })
    }

    /// Get customer keys from serialized string
    pub fn get_customer_keys(&self) -> TollGateResult<Keys> {
        Keys::parse(&self.customer_keys)
            .map_err(|e| TollGateError::session(format!("Invalid customer keys: {}", e)))
    }

    /// Update session from TollGate response
    pub fn update_from_response(&mut self, response: &SessionResponse) -> TollGateResult<()> {
        if response.mac_address != self.mac_address {
            return Err(TollGateError::session(
                "MAC address mismatch in session response",
            ));
        }

        self.total_allotment += response.allotment;
        self.session_end = response.session_end;
        self.status = SessionStatus::Active;

        Ok(())
    }

    /// Check if session needs renewal
    pub fn needs_renewal(&self) -> bool {
        if self.status != SessionStatus::Active {
            return false;
        }

        let usage_percent = self.current_usage as f64 / self.total_allotment as f64;
        usage_percent >= self.renewal_threshold
    }

    /// Check if session is expired
    pub fn is_expired(&self) -> bool {
        Utc::now() >= self.session_end || self.current_usage >= self.total_allotment
    }

    /// Update usage and check for expiration
    pub fn update_usage(&mut self, new_usage: u64) {
        self.current_usage = new_usage.min(self.total_allotment);

        if self.is_expired() {
            self.status = SessionStatus::Expired;
        }
    }

    /// Get remaining time in seconds (for time-based sessions)
    pub fn remaining_time_seconds(&self) -> Option<i64> {
        if self.advertisement.metric != "milliseconds" {
            return None;
        }

        let remaining_ms = self.total_allotment.saturating_sub(self.current_usage);
        Some((remaining_ms / 1000) as i64)
    }

    /// Get remaining data in bytes (for data-based sessions)
    pub fn remaining_data_bytes(&self) -> Option<u64> {
        if self.advertisement.metric != "bytes" {
            return None;
        }

        Some(self.total_allotment.saturating_sub(self.current_usage))
    }

    /// Get usage percentage (0.0 to 1.0)
    pub fn usage_percentage(&self) -> f64 {
        if self.total_allotment == 0 {
            return 1.0;
        }
        (self.current_usage as f64 / self.total_allotment as f64).min(1.0)
    }

    /// Mark session as renewed
    pub fn mark_renewed(&mut self, additional_allotment: u64, additional_cost: u64) {
        self.total_allotment += additional_allotment;
        self.total_spent += additional_cost;
        self.payment_count += 1;
        self.last_renewal = Some(Utc::now());
        self.status = SessionStatus::Active;
    }

    /// Set session error
    pub fn set_error(&mut self, error: String) {
        self.status = SessionStatus::Error(error);
    }

    /// Check if session is active
    pub fn is_active(&self) -> bool {
        matches!(self.status, SessionStatus::Active | SessionStatus::Renewing)
    }
}

/// Session manager for handling multiple sessions
#[derive(Debug, Default)]
pub struct SessionManager {
    sessions: HashMap<String, Session>, // keyed by tollgate_pubkey
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Add a new session
    pub fn add_session(&mut self, session: Session) {
        self.sessions
            .insert(session.tollgate_pubkey.clone(), session);
    }

    /// Get session by tollgate pubkey
    pub fn get_session(&self, tollgate_pubkey: &str) -> Option<&Session> {
        self.sessions.get(tollgate_pubkey)
    }

    /// Get mutable session by tollgate pubkey
    pub fn get_session_mut(&mut self, tollgate_pubkey: &str) -> Option<&mut Session> {
        self.sessions.get_mut(tollgate_pubkey)
    }

    /// Remove session
    #[allow(dead_code)]
    pub fn remove_session(&mut self, tollgate_pubkey: &str) -> Option<Session> {
        self.sessions.remove(tollgate_pubkey)
    }

    /// Get all active sessions
    pub fn get_active_sessions(&self) -> Vec<&Session> {
        self.sessions
            .values()
            .filter(|session| session.is_active())
            .collect()
    }

    /// Get all sessions that need renewal
    pub fn get_sessions_needing_renewal(&self) -> Vec<&Session> {
        self.sessions
            .values()
            .filter(|session| session.needs_renewal())
            .collect()
    }

    /// Get all expired sessions
    #[allow(dead_code)]
    pub fn get_expired_sessions(&self) -> Vec<&Session> {
        self.sessions
            .values()
            .filter(|session| session.is_expired())
            .collect()
    }

    /// Get all sessions as mutable iterator
    pub fn get_all_sessions_mut(&mut self) -> impl Iterator<Item = &mut Session> {
        self.sessions.values_mut()
    }

    /// Clean up expired sessions
    pub fn cleanup_expired_sessions(&mut self) {
        self.sessions.retain(|_, session| !session.is_expired());
    }

    /// Get session count
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Get active session count
    #[allow(dead_code)]
    pub fn active_session_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|session| session.is_active())
            .count()
    }

    /// Update usage for all sessions based on time (for time-based sessions)
    pub fn update_time_based_usage(&mut self) {
        let now = Utc::now();

        for session in self.sessions.values_mut() {
            if session.advertisement.metric == "milliseconds" && session.is_active() {
                let elapsed_ms = now
                    .signed_duration_since(session.created_at)
                    .num_milliseconds() as u64;

                session.update_usage(elapsed_ms);
            }
        }
    }

    /// Serialize sessions for persistence
    pub fn serialize(&self) -> TollGateResult<String> {
        serde_json::to_string(&self.sessions)
            .map_err(|e| TollGateError::session(format!("Failed to serialize sessions: {}", e)))
    }

    /// Deserialize sessions from persistence
    #[allow(dead_code)]
    pub fn deserialize(data: &str) -> TollGateResult<Self> {
        let sessions: HashMap<String, Session> = serde_json::from_str(data).map_err(|e| {
            TollGateError::session(format!("Failed to deserialize sessions: {}", e))
        })?;

        Ok(Self { sessions })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tollgate::protocol::PricingOption;

    fn create_test_session() -> Session {
        let pricing_option = PricingOption {
            asset_type: "cashu".to_string(),
            price_per_step: 1,
            price_unit: "sat".to_string(),
            mint_url: "https://mint.example.com".to_string(),
            min_steps: 60,
        };

        let advertisement = TollGateAdvertisement {
            metric: "milliseconds".to_string(),
            step_size: 1000,
            pricing_options: vec![pricing_option.clone()],
            tips: vec!["01".to_string()],
            tollgate_pubkey: "test_pubkey".to_string(),
        };

        Session::new(SessionParams {
            tollgate_pubkey: "test_pubkey".to_string(),
            gateway_ip: "192.168.1.1".to_string(),
            mac_address: "aa:bb:cc:dd:ee:ff".to_string(),
            pricing_option,
            advertisement,
            initial_allotment: 60000, // 60 seconds
            session_end: Utc::now() + chrono::Duration::minutes(1),
            initial_cost: 60, // 60 sats
        })
        .unwrap()
    }

    #[test]
    fn test_session_creation() {
        let session = create_test_session();
        assert_eq!(session.status, SessionStatus::Initializing);
        assert_eq!(session.total_allotment, 60000);
        assert_eq!(session.current_usage, 0);
    }

    #[test]
    fn test_needs_renewal() {
        let mut session = create_test_session();
        session.status = SessionStatus::Active;

        // At 50% usage, should not need renewal
        session.update_usage(30000);
        assert!(!session.needs_renewal());

        // At 80% usage, should need renewal
        session.update_usage(48000);
        assert!(session.needs_renewal());
    }

    #[test]
    fn test_usage_percentage() {
        let mut session = create_test_session();

        session.update_usage(30000);
        assert_eq!(session.usage_percentage(), 0.5);

        session.update_usage(60000);
        assert_eq!(session.usage_percentage(), 1.0);
    }

    #[test]
    fn test_session_manager() {
        let mut manager = SessionManager::new();
        let session = create_test_session();
        let pubkey = session.tollgate_pubkey.clone();

        manager.add_session(session);
        assert_eq!(manager.session_count(), 1);

        let retrieved = manager.get_session(&pubkey);
        assert!(retrieved.is_some());

        manager.remove_session(&pubkey);
        assert_eq!(manager.session_count(), 0);
    }
}
