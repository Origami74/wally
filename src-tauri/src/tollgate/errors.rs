//! Error types for TollGate operations

use thiserror::Error;

pub type TollGateResult<T> = Result<T, TollGateError>;

#[derive(Error, Debug)]
pub enum TollGateError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Nostr error: {0}")]
    Nostr(#[from] nostr::event::builder::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Cashu wallet error: {0}")]
    Wallet(String),

    #[error("TollGate not found on network")]
    TollGateNotFound,

    #[error("Invalid TollGate advertisement: {0}")]
    InvalidAdvertisement(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Insufficient funds: need {needed} sats, have {available} sats")]
    InsufficientFunds { needed: u64, available: u64 },

    #[error("Session expired")]
    SessionExpired,

    #[error("Network disconnected")]
    NetworkDisconnected,

    #[error("Invalid MAC address: {0}")]
    InvalidMacAddress(String),

    #[error("Invalid gateway IP: {0}")]
    InvalidGatewayIp(String),

    #[error("Timeout waiting for session confirmation")]
    SessionTimeout,

    #[error("Background service error: {0}")]
    BackgroundService(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Other error: {0}")]
    Other(String),
}

impl TollGateError {
    pub fn wallet(msg: impl Into<String>) -> Self {
        Self::Wallet(msg.into())
    }

    pub fn session(msg: impl Into<String>) -> Self {
        Self::Session(msg.into())
    }

    pub fn protocol(msg: impl Into<String>) -> Self {
        Self::Protocol(msg.into())
    }

    #[allow(dead_code)]
    pub fn background_service(msg: impl Into<String>) -> Self {
        Self::BackgroundService(msg.into())
    }

    #[allow(dead_code)]
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}
