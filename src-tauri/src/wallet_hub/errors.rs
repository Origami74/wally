//! Error types for TollGate operations

use thiserror::Error;

pub type WalletResult<T> = Result<T, WalletError>;

#[derive(Error, Debug)]
pub enum WalletError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Nostr error: {0}")]
    Nostr(#[from] nostr::event::builder::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Wallet error: {0}")]
    Wallet(String),

    #[allow(dead_code)]
    #[error("Insufficient funds: need {needed} sats, have {available} sats")]
    InsufficientFunds { needed: u64, available: u64 },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl WalletError {
    pub fn wallet(msg: impl Into<String>) -> Self {
        Self::Wallet(msg.into())
    }
}
