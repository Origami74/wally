//! Cashu wallet integration for TollGate payments
//!
//! Handles:
//! - Cashu token creation and management
//! - Mint compatibility checking
//! - Balance management
//! - Payment token generation

use crate::tollgate::errors::{TollGateError, TollGateResult};
use crate::tollgate::protocol::PricingOption;
use bip39::{Language, Mnemonic};
use cdk::mint_url::MintUrl;
use cdk::nuts::nut18::payment_request::PaymentRequest;
use cdk::nuts::CurrencyUnit;
use cdk::wallet::{
    types::{Transaction, TransactionDirection},
    MintQuote, SendOptions, Wallet,
};
use cdk::{amount::SplitTarget, Amount};
use cdk_sqlite::wallet::WalletSqliteDatabase;
use directories::ProjectDirs;
use nostr::prelude::{Keys, SecretKey, ToBech32};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

/// Cashu wallet for TollGate payments
pub struct TollGateWallet {
    wallets: HashMap<String, Wallet>, // keyed by mint URL
    default_mint: Option<String>,
    storage: WalletStoragePaths,
    secrets: WalletSecrets,
}

/// Payment token information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentToken {
    pub token: String,
    pub amount: u64,
    pub mint_url: String,
    pub unit: String,
}

/// Encoded NUT-18 payment request information
#[derive(Debug, Clone, Serialize)]
pub struct Nut18PaymentRequestInfo {
    pub request: String,
    pub amount: Option<u64>,
    pub unit: String,
    pub description: Option<String>,
    pub mints: Vec<String>,
}

/// Data returned when issuing a new BOLT11 invoice via the mint
#[derive(Debug, Clone, Serialize)]
pub struct Bolt11InvoiceInfo {
    pub quote_id: String,
    pub request: String,
    pub amount: Option<u64>,
    pub unit: String,
    pub expiry: u64,
    pub mint_url: String,
}

/// Result of paying a BOLT11 invoice
#[derive(Debug, Clone, Serialize)]
pub struct Bolt11PaymentResult {
    pub amount: u64,
    pub fee_paid: u64,
    pub preimage: Option<String>,
}

/// Result of receiving a cashu token
#[derive(Debug, Clone, Serialize)]
pub struct CashuReceiveResult {
    pub amount: u64,
    pub mint_url: String,
}

/// Result of paying a NUT18 payment request
#[derive(Debug, Clone, Serialize)]
pub struct PayNut18Result {
    pub amount: u64,
    pub token: Option<String>,
}

/// Snapshot of wallet state for UI consumption
#[derive(Debug, Clone, Serialize)]
pub struct WalletSummary {
    pub total: u64,
    pub default_mint: Option<String>,
    pub balances: Vec<WalletBalance>,
    pub npub: Option<String>,
}

/// Flattened transaction entry suitable for the frontend
#[derive(Debug, Clone, Serialize)]
pub struct WalletTransactionEntry {
    pub id: String,
    pub direction: String,
    pub amount: u64,
    pub fee: u64,
    pub unit: String,
    pub timestamp: u64,
    pub mint_url: String,
    pub memo: Option<String>,
    pub quote_id: Option<String>,
}

/// Wallet balance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletBalance {
    pub mint_url: String,
    pub balance: u64,
    pub unit: String,
    pub pending: u64,
}

#[derive(Debug, Clone)]
struct WalletStoragePaths {
    secrets_file: PathBuf,
    wallets_dir: PathBuf,
    mints_file: PathBuf,
}

impl WalletStoragePaths {
    fn new() -> TollGateResult<Self> {
        let project_dirs = ProjectDirs::from("com", "Tollgate", "TollgateApp")
            .ok_or_else(|| TollGateError::wallet("Unable to determine wallet storage directory"))?;

        let base_dir = project_dirs.data_dir().to_path_buf();
        let wallets_dir = base_dir.join("wallets");

        if let Some(parent) = base_dir.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::create_dir_all(&base_dir)?;
        fs::create_dir_all(&wallets_dir)?;

        let secrets_file = base_dir.join("wallet-secrets.json");
        let mints_file = base_dir.join("mints.json");

        Ok(Self {
            secrets_file,
            wallets_dir,
            mints_file,
        })
    }

    fn mint_db_path(&self, mint_url: &str) -> TollGateResult<PathBuf> {
        let hash = format!("{:x}", Sha256::digest(mint_url.as_bytes()));
        let sanitized: String = mint_url
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .collect();
        let prefix: String = sanitized.chars().take(32).collect();
        let stem = if prefix.is_empty() {
            hash[..16].to_string()
        } else {
            format!("{}-{}", prefix.to_lowercase(), &hash[..16])
        };

        Ok(self.wallets_dir.join(format!("{}.sqlite", stem)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoredSecrets {
    mnemonic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoredMints {
    mints: Vec<String>,
    default_mint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WalletSecrets {
    wallet_seed: [u8; 64],
    nostr_keys: Keys,
}

impl WalletSecrets {
    fn load_or_create(paths: &WalletStoragePaths) -> TollGateResult<Self> {
        if paths.secrets_file.exists() {
            let data = fs::read(&paths.secrets_file)?;
            let stored: StoredSecrets = serde_json::from_slice(&data)?;
            Self::from_stored(stored)
        } else {
            // TODO(security): store mnemonic in platform secure storage rather than plaintext file.
            Self::generate_and_persist(paths)
        }
    }

    fn generate_and_persist(paths: &WalletStoragePaths) -> TollGateResult<Self> {
        let mnemonic = Mnemonic::generate_in(Language::English, 12)
            .map_err(|e| TollGateError::wallet(format!("Failed to generate mnemonic: {}", e)))?;
        let phrase = mnemonic.to_string();
        let wallet_seed = mnemonic.to_seed("");

        let nostr_keys = derive_nostr_keys_from_seed(&wallet_seed)?;
        let secrets = Self {
            wallet_seed,
            nostr_keys,
        };

        let stored = StoredSecrets {
            mnemonic: Some(phrase),
        };

        if let Some(parent) = paths.secrets_file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&paths.secrets_file, serde_json::to_vec_pretty(&stored)?)?;

        Ok(secrets)
    }

    fn from_stored(stored: StoredSecrets) -> TollGateResult<Self> {
        if let Some(phrase) = stored.mnemonic {
            Self::from_mnemonic(phrase)
        } else {
            Err(TollGateError::wallet("Wallet secrets file is empty"))
        }
    }

    fn from_mnemonic(phrase: String) -> TollGateResult<Self> {
        let mnemonic = Mnemonic::parse_in(Language::English, phrase.trim())
            .map_err(|e| TollGateError::wallet(format!("Invalid mnemonic: {}", e)))?;
        let wallet_seed = mnemonic.to_seed("");

        let nostr_keys = derive_nostr_keys_from_seed(&wallet_seed)?;
        Ok(Self {
            wallet_seed,
            nostr_keys,
        })
    }

    pub fn wallet_seed(&self) -> [u8; 64] {
        self.wallet_seed
    }

    pub(crate) fn nostr_npub(&self) -> TollGateResult<String> {
        self.nostr_keys
            .public_key()
            .to_bech32()
            .map_err(|e| TollGateError::wallet(format!("Failed to encode npub: {}", e)))
    }
}

fn derive_nostr_keys_from_seed(seed: &[u8; 64]) -> TollGateResult<Keys> {
    let hash = Sha256::digest(seed);
    let secret_key = SecretKey::from_slice(hash.as_slice())
        .map_err(|e| TollGateError::wallet(format!("Failed to derive nostr key: {}", e)))?;
    Ok(Keys::new(secret_key))
}

impl TollGateWallet {
    pub fn clone_wallet_for_mint(&self, mint_url: &str) -> Option<Wallet> {
        self.wallets.get(mint_url).cloned()
    }

    pub fn new() -> TollGateResult<Self> {
        let storage = WalletStoragePaths::new()?;
        let secrets = WalletSecrets::load_or_create(&storage)?;

        Ok(Self {
            wallets: HashMap::new(),
            default_mint: None,
            storage,
            secrets,
        })
    }

    /// Load the list of mints from persistent storage
    fn load_mints_config(&self) -> TollGateResult<StoredMints> {
        if self.storage.mints_file.exists() {
            let data = fs::read(&self.storage.mints_file)?;
            let stored: StoredMints = serde_json::from_slice(&data).unwrap_or_default();
            Ok(stored)
        } else {
            Ok(StoredMints::default())
        }
    }

    /// Save the list of mints to persistent storage
    fn save_mints_config(&self) -> TollGateResult<()> {
        let stored = StoredMints {
            mints: self.wallets.keys().cloned().collect(),
            default_mint: self.default_mint.clone(),
        };

        if let Some(parent) = self.storage.mints_file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            &self.storage.mints_file,
            serde_json::to_vec_pretty(&stored)?,
        )?;
        Ok(())
    }

    /// Load existing mints from storage on startup
    pub async fn load_existing_mints(&mut self) -> TollGateResult<()> {
        let stored_mints = self.load_mints_config()?;

        for mint_url in stored_mints.mints {
            if !self.wallets.contains_key(&mint_url) {
                log::info!("Loading existing mint from storage: {}", mint_url);
                if let Err(e) = self.add_mint_internal(&mint_url).await {
                    log::warn!("Failed to load existing mint {}: {}", mint_url, e);
                }
            }
        }

        if let Some(default_mint) = stored_mints.default_mint {
            if self.wallets.contains_key(&default_mint) {
                self.default_mint = Some(default_mint);
            }
        }

        Ok(())
    }

    fn default_mint_url(&self) -> TollGateResult<&String> {
        self.default_mint
            .as_ref()
            .ok_or_else(|| TollGateError::wallet("No default mint configured"))
    }

    fn get_wallet_by_url(&self, mint_url: &str) -> TollGateResult<&Wallet> {
        self.wallets
            .get(mint_url)
            .ok_or_else(|| TollGateError::wallet(format!("Mint not found: {}", mint_url)))
    }

    fn default_wallet(&self) -> TollGateResult<&Wallet> {
        let mint = self.default_mint_url()?;
        self.get_wallet_by_url(mint)
    }

    fn wallet_for_payment_request(&self, request: &PaymentRequest) -> TollGateResult<&Wallet> {
        if let Some(mints) = &request.mints {
            for mint in mints {
                let key = mint.to_string();
                if let Some(wallet) = self.wallets.get(&key) {
                    return Ok(wallet);
                }
            }
        }

        let default_mint = self.default_mint_url()?;
        self.get_wallet_by_url(default_mint)
    }

    /// Internal method to add a mint without persisting config
    async fn add_mint_internal(&mut self, mint_url: &str) -> TollGateResult<()> {
        if self.wallets.contains_key(mint_url) {
            return Ok(()); // Already added
        }

        let db_path = self.storage.mint_db_path(mint_url)?;
        let localstore = WalletSqliteDatabase::new(db_path).await.map_err(|e| {
            TollGateError::wallet(format!(
                "Failed to open wallet database for mint {}: {}",
                mint_url, e
            ))
        })?;

        let wallet = Wallet::new(
            mint_url,
            CurrencyUnit::Sat,
            Arc::new(localstore),
            self.secrets.wallet_seed(),
            None,
        )
        .map_err(|e| {
            TollGateError::wallet(format!(
                "Failed to create wallet for mint {}: {}",
                mint_url, e
            ))
        })?;

        self.wallets.insert(mint_url.to_string(), wallet);

        // Set as default if it's the first mint
        if self.default_mint.is_none() {
            self.default_mint = Some(mint_url.to_string());
        }

        log::info!("Added mint to wallet: {}", mint_url);
        Ok(())
    }

    /// Add a mint to the wallet and persist it
    pub async fn add_mint(&mut self, mint_url: &str) -> TollGateResult<()> {
        self.add_mint_internal(mint_url).await?;
        self.save_mints_config()?;
        Ok(())
    }

    /// Set the default mint
    pub async fn set_default_mint(&mut self, mint_url: &str) -> TollGateResult<()> {
        if !self.wallets.contains_key(mint_url) {
            self.add_mint_internal(mint_url).await?;
        }

        self.default_mint = Some(mint_url.to_string());
        self.save_mints_config()?;
        Ok(())
    }

    /// Return the wallet's npub, if derivable
    pub fn nostr_npub(&self) -> Option<String> {
        self.secrets.nostr_npub().ok()
    }

    /// Get the wallet's Nostr keys
    pub fn get_keys(&self) -> nostr::Keys {
        self.secrets.nostr_keys.clone()
    }

    /// Get balance for a specific mint
    pub async fn get_balance(&self, mint_url: &str) -> TollGateResult<u64> {
        let wallet = self
            .wallets
            .get(mint_url)
            .ok_or_else(|| TollGateError::wallet(format!("Mint not found: {}", mint_url)))?;

        let balance = wallet
            .total_balance()
            .await
            .map_err(|e| TollGateError::wallet(format!("Failed to get balance: {}", e)))?;

        Ok(balance.into())
    }

    /// Get balance for all mints
    pub async fn get_all_balances(&self) -> TollGateResult<Vec<WalletBalance>> {
        let mut balances = Vec::new();

        for (mint_url, wallet) in &self.wallets {
            let balance = wallet.total_balance().await.map_err(|e| {
                TollGateError::wallet(format!("Failed to get balance for {}: {}", mint_url, e))
            })?;

            balances.push(WalletBalance {
                mint_url: mint_url.clone(),
                balance: balance.into(),
                unit: "sat".to_string(), // TODO: Get actual unit from wallet
                pending: 0,              // TODO: Get pending balance
            });
        }

        Ok(balances)
    }

    /// Summarize balances and metadata for UI consumption
    pub async fn summary(&self) -> TollGateResult<WalletSummary> {
        let balances = self.get_all_balances().await?;
        let total = balances.iter().map(|b| b.balance).sum();
        let npub = self.nostr_npub();

        Ok(WalletSummary {
            total,
            default_mint: self.default_mint.clone(),
            balances,
            npub,
        })
    }

    /// Create a NUT-18 payment request
    pub fn create_nut18_payment_request(
        &self,
        amount: Option<u64>,
        description: Option<String>,
    ) -> TollGateResult<Nut18PaymentRequestInfo> {
        if self.wallets.is_empty() {
            return Err(TollGateError::wallet(
                "Add a mint before generating a payment request",
            ));
        }

        let mint_urls: Vec<MintUrl> = self
            .wallets
            .keys()
            .map(|url| {
                MintUrl::from_str(url)
                    .map_err(|e| TollGateError::wallet(format!("Invalid mint URL {}: {}", url, e)))
            })
            .collect::<Result<_, _>>()?;

        let mut builder = PaymentRequest::builder()
            .unit(CurrencyUnit::Sat)
            .single_use(true)
            .mints(mint_urls.clone());

        if let Some(amount) = amount {
            builder = builder.amount(amount);
        }
        if let Some(desc) = description.clone() {
            builder = builder.description(desc);
        }

        let request = builder.build();
        let request_string = request.to_string();
        let response = Nut18PaymentRequestInfo {
            request: request_string,
            amount,
            unit: CurrencyUnit::Sat.to_string(),
            description,
            mints: mint_urls.into_iter().map(|m| m.to_string()).collect(),
        };

        Ok(response)
    }

    /// Create a BOLT11 invoice via the active mint
    pub async fn create_bolt11_invoice(
        &self,
        amount: u64,
        description: Option<String>,
    ) -> TollGateResult<Bolt11InvoiceInfo> {
        let wallet = self.default_wallet()?;
        let quote = wallet
            .mint_quote(Amount::from(amount), description.clone())
            .await
            .map_err(|e| TollGateError::wallet(format!("Failed to request mint quote: {}", e)))?;

        Ok(Bolt11InvoiceInfo {
            quote_id: quote.id.clone(),
            request: quote.request.clone(),
            amount: quote.amount.map(u64::from),
            unit: quote.unit.to_string(),
            expiry: quote.expiry,
            mint_url: quote.mint_url.to_string(),
        })
    }

    /// Pay a NUT-18 payment request
    ///
    /// This will error if the payment request has no transport defined.
    pub async fn pay_nut18_payment_request(
        &self,
        request: &str,
        custom_amount: Option<u64>,
    ) -> TollGateResult<()> {
        let payment_request = PaymentRequest::from_str(request)
            .map_err(|e| TollGateError::wallet(format!("Invalid payment request: {}", e)))?;

        // Check if there's a transport defined
        if payment_request.transports.is_empty() {
            return Err(TollGateError::wallet(
                "Payment request has no transport defined. Use pay_nut18_payment_request_with_token to get the token."
            ));
        }

        let wallet = self.wallet_for_payment_request(&payment_request)?;
        let custom_amount = custom_amount.map(Amount::from);

        let _ = wallet
            .pay_request(payment_request, custom_amount)
            .await
            .map_err(|e| TollGateError::wallet(format!("Failed to pay request: {}", e)))?;

        Ok(())
    }

    /// Pay a NUT-18 payment request, returning a Token if no transport is defined
    ///
    /// If the payment request has a transport (Nostr or HTTP), the wallet will pay it
    /// via that transport and return None. If no transport is defined, this method
    /// returns the encoded token that should be delivered out-of-band.
    pub async fn pay_nut18_payment_request_with_token(
        &self,
        request: &str,
        custom_amount: Option<u64>,
    ) -> TollGateResult<PayNut18Result> {
        use cdk::wallet::SendOptions;

        let payment_request = PaymentRequest::from_str(request)
            .map_err(|e| TollGateError::wallet(format!("Invalid payment request: {}", e)))?;

        let wallet = self.wallet_for_payment_request(&payment_request)?;

        let amount = match payment_request.amount {
            Some(amount) => amount,
            None => match custom_amount {
                Some(a) => Amount::from(a),
                None => {
                    return Err(TollGateError::wallet(
                        "Amount not specified in request and no custom amount provided",
                    ))
                }
            },
        };

        let amount_u64: u64 = amount.into();

        // Check if there's a transport defined
        let has_transport = !payment_request.transports.is_empty();

        if has_transport {
            // If transport exists, use pay_request which will handle delivery
            wallet
                .pay_request(payment_request, custom_amount.map(Amount::from))
                .await
                .map_err(|e| TollGateError::wallet(format!("Failed to pay request: {}", e)))?;

            Ok(PayNut18Result {
                amount: amount_u64,
                token: None,
            })
        } else {
            // No transport, prepare and confirm the send, then return the encoded token
            let prepared_send = wallet
                .prepare_send(
                    amount,
                    SendOptions {
                        include_fee: true,
                        ..Default::default()
                    },
                )
                .await
                .map_err(|e| TollGateError::wallet(format!("Failed to prepare send: {}", e)))?;

            let token = prepared_send
                .confirm(None)
                .await
                .map_err(|e| TollGateError::wallet(format!("Failed to confirm send: {}", e)))?;

            Ok(PayNut18Result {
                amount: amount_u64,
                token: Some(token.to_string()),
            })
        }
    }

    /// Pay a BOLT11 invoice using the default mint
    pub async fn pay_bolt11_invoice(&self, invoice: &str) -> TollGateResult<Bolt11PaymentResult> {
        let wallet = self.default_wallet()?;
        let quote = wallet
            .melt_quote(invoice.to_string(), None)
            .await
            .map_err(|e| TollGateError::wallet(format!("Failed to request melt quote: {}", e)))?;

        let melted = wallet
            .melt(&quote.id)
            .await
            .map_err(|e| TollGateError::wallet(format!("Failed to pay invoice: {}", e)))?;
        let amount: u64 = melted.amount.into();
        let fee_paid: u64 = melted.fee_paid.into();

        Ok(Bolt11PaymentResult {
            amount,
            fee_paid,
            preimage: melted.preimage,
        })
    }

    /// Receive a cashu token and add it to the wallet
    pub async fn receive_cashu_token(&mut self, token: &str) -> TollGateResult<CashuReceiveResult> {
        // Parse the token to determine which mint it belongs to
        let cashu_token = cdk::nuts::Token::from_str(token)
            .map_err(|e| TollGateError::wallet(format!("Invalid cashu token: {}", e)))?;

        // Get the mint URL from the token
        let mint_url = cashu_token
            .mint_url()
            .map_err(|e| TollGateError::wallet(format!("Failed to get mint URL: {}", e)))?
            .to_string();

        // Check if we have a wallet for this mint, if not add it automatically
        // Note: add_mint() now persists the config automatically
        if !self.wallets.contains_key(&mint_url) {
            log::info!("Mint {} not found, adding it automatically", mint_url);
            self.add_mint(&mint_url).await?;
        }

        // Get the wallet (should exist now)
        let wallet = self.wallets.get(&mint_url).ok_or_else(|| {
            TollGateError::wallet(format!("Failed to create wallet for mint: {}", mint_url))
        })?;

        // Receive the token with default options
        let received_amount = wallet
            .receive(token, cdk::wallet::ReceiveOptions::default())
            .await
            .map_err(|e| TollGateError::wallet(format!("Failed to receive token: {}", e)))?;

        // Convert amount to u64
        let total_amount: u64 = received_amount.into();

        log::info!(
            "Successfully received {} sats from token at mint {}",
            total_amount,
            mint_url
        );
        Ok(CashuReceiveResult {
            amount: total_amount,
            mint_url,
        })
    }

    /// List transactions across all configured mints
    pub async fn list_transactions(
        &self,
        direction: Option<TransactionDirection>,
    ) -> TollGateResult<Vec<WalletTransactionEntry>> {
        let mut transactions: Vec<WalletTransactionEntry> = Vec::new();

        for wallet in self.wallets.values() {
            let mut wallet_transactions =
                wallet.list_transactions(direction).await.map_err(|e| {
                    TollGateError::wallet(format!("Failed to list transactions: {}", e))
                })?;
            transactions.extend(wallet_transactions.drain(..).map(Into::into));
        }

        transactions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(transactions)
    }

    /// Check if we can afford a payment
    pub async fn can_afford(
        &self,
        pricing_option: &PricingOption,
        steps: u64,
    ) -> TollGateResult<bool> {
        let required_amount = pricing_option.price_per_step * steps;
        let balance = self.get_balance(&pricing_option.mint_url).await?;

        Ok(balance >= required_amount)
    }

    /// Create a payment token for the specified amount
    pub async fn create_payment_token(
        &self,
        pricing_option: &PricingOption,
        steps: u64,
    ) -> TollGateResult<PaymentToken> {
        let amount = pricing_option.price_per_step * steps;

        if amount < pricing_option.min_steps * pricing_option.price_per_step {
            return Err(TollGateError::wallet(format!(
                "Amount {} is below minimum {} steps",
                steps, pricing_option.min_steps
            )));
        }

        let wallet = self.wallets.get(&pricing_option.mint_url).ok_or_else(|| {
            TollGateError::wallet(format!("Mint not found: {}", pricing_option.mint_url))
        })?;

        // Check balance
        let balance = wallet
            .total_balance()
            .await
            .map_err(|e| TollGateError::wallet(format!("Failed to get balance: {}", e)))?;

        if balance < Amount::from(amount) {
            return Err(TollGateError::InsufficientFunds {
                needed: amount,
                available: balance.into(),
            });
        }

        // Create the payment token
        let prepared_send = wallet
            .prepare_send(Amount::from(amount), SendOptions::default())
            .await
            .map_err(|e| {
                TollGateError::wallet(format!("Failed to prepare payment token: {}", e))
            })?;

        let token = prepared_send.confirm(None).await.map_err(|e| {
            TollGateError::wallet(format!("Failed to confirm payment token: {}", e))
        })?;

        Ok(PaymentToken {
            token: token.to_string(),
            amount,
            mint_url: pricing_option.mint_url.clone(),
            unit: pricing_option.price_unit.clone(),
        })
    }

    pub async fn create_external_token(
        &self,
        amount_sats: u64,
        mint_url: Option<String>,
    ) -> TollGateResult<String> {
        let target_mint = if let Some(mint) = mint_url {
            mint
        } else {
            let summary = self.summary().await?;
            if summary.balances.is_empty() {
                return Err(TollGateError::wallet("No mints configured".to_string()));
            }

            let mut selected_mint = None;
            for wallet_balance in &summary.balances {
                if wallet_balance.balance >= amount_sats {
                    selected_mint = Some(wallet_balance.mint_url.clone());
                    break;
                }
            }

            match selected_mint {
                Some(mint) => mint,
                None => {
                    let total_balance: u64 = summary.balances.iter().map(|b| b.balance).sum();
                    return Err(TollGateError::wallet(format!(
                        "Insufficient balance: {} sats available across all mints, {} sats requested",
                        total_balance, amount_sats
                    )));
                }
            }
        };

        let balance = self.get_balance(&target_mint).await?;
        if balance < amount_sats {
            return Err(TollGateError::wallet(format!(
                "Insufficient balance: {} sats available, {} sats requested",
                balance, amount_sats
            )));
        }

        let wallet = self.wallets.get(&target_mint).ok_or_else(|| {
            TollGateError::wallet(format!("Wallet not found for mint: {}", target_mint))
        })?;

        let prepared_send = wallet
            .prepare_send(
                cdk::Amount::from(amount_sats),
                cdk::wallet::SendOptions {
                    include_fee: true,
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| TollGateError::wallet(format!("Failed to prepare token: {}", e)))?;

        let token = prepared_send
            .confirm(None)
            .await
            .map_err(|e| TollGateError::wallet(format!("Failed to create token: {}", e)))?;

        log::info!(
            "Created external token: {} sats from mint {}",
            amount_sats,
            target_mint
        );
        Ok(token.to_string())
    }

    /// Request a mint quote for loading the wallet
    #[allow(dead_code)]
    pub async fn request_mint_quote(
        &self,
        mint_url: &str,
        amount: u64,
    ) -> TollGateResult<MintQuote> {
        let wallet = self
            .wallets
            .get(mint_url)
            .ok_or_else(|| TollGateError::wallet(format!("Mint not found: {}", mint_url)))?;

        let quote = wallet
            .mint_quote(Amount::from(amount), None)
            .await
            .map_err(|e| TollGateError::wallet(format!("Failed to request mint quote: {}", e)))?;

        Ok(quote)
    }

    /// Check mint quote status and mint tokens if paid
    #[allow(dead_code)]
    pub async fn check_mint_quote(&self, mint_url: &str, quote_id: &str) -> TollGateResult<bool> {
        let wallet = self
            .wallets
            .get(mint_url)
            .ok_or_else(|| TollGateError::wallet(format!("Mint not found: {}", mint_url)))?;

        let status = wallet
            .mint_quote_state(quote_id)
            .await
            .map_err(|e| TollGateError::wallet(format!("Failed to check mint quote: {}", e)))?;

        if status.state == cdk::nuts::MintQuoteState::Paid {
            // Mint the tokens
            wallet
                .mint(&status.quote, SplitTarget::default(), None)
                .await
                .map_err(|e| TollGateError::wallet(format!("Failed to mint tokens: {}", e)))?;

            log::info!("Successfully minted tokens for quote {}", quote_id);
            return Ok(true);
        }

        Ok(false)
    }

    /// Find the best pricing option based on available balances
    pub async fn select_best_pricing_option(
        &self,
        options: &[PricingOption],
        steps: u64,
    ) -> TollGateResult<PricingOption> {
        let mut compatible_options = Vec::new();

        for option in options {
            if self.wallets.contains_key(&option.mint_url) {
                if let Ok(can_afford) = self.can_afford(option, steps).await {
                    if can_afford {
                        compatible_options.push(option.clone());
                    }
                }
            }
        }

        if compatible_options.is_empty() {
            return Err(TollGateError::wallet(
                "No compatible pricing options with sufficient balance",
            ));
        }

        // Select the option with the lowest total cost
        let best_option = compatible_options
            .into_iter()
            .min_by_key(|option| option.price_per_step * steps)
            .unwrap();

        Ok(best_option)
    }
}

impl From<Transaction> for WalletTransactionEntry {
    fn from(tx: Transaction) -> Self {
        let direction = match tx.direction {
            TransactionDirection::Incoming => "incoming",
            TransactionDirection::Outgoing => "outgoing",
        }
        .to_string();

        let amount: u64 = tx.amount.into();
        let fee: u64 = tx.fee.into();

        WalletTransactionEntry {
            id: tx.id().to_string(),
            direction,
            amount,
            fee,
            unit: tx.unit.to_string(),
            timestamp: tx.timestamp,
            mint_url: tx.mint_url.to_string(),
            memo: tx.memo,
            quote_id: tx.quote_id,
        }
    }
}

impl Default for TollGateWallet {
    fn default() -> Self {
        Self::new().expect("Failed to initialize TollGate wallet")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wallet_creation() {
        let _wallet = TollGateWallet::new().expect("wallet");
        // Basic wallet creation test
    }

    #[test]
    fn test_payment_token_creation() {
        let token = PaymentToken {
            token: "cashuAbc123".to_string(),
            amount: 100,
            mint_url: "https://mint.example.com".to_string(),
            unit: "sat".to_string(),
        };

        assert_eq!(token.amount, 100);
        assert_eq!(token.unit, "sat");
    }

    #[test]
    fn test_wallet_balance() {
        let balance = WalletBalance {
            mint_url: "https://mint.example.com".to_string(),
            balance: 1000,
            unit: "sat".to_string(),
            pending: 0,
        };

        assert_eq!(balance.balance, 1000);
        assert_eq!(balance.pending, 0);
    }
}
