//! Cashu wallet integration
//!
//! Handles:
//! - Cashu token creation and management
//! - Mint compatibility checking
//! - Balance management

pub mod errors;
use self::errors::{WalletError, WalletResult};

use bip39::{Language, Mnemonic};
use cdk::cdk_database::WalletDatabase;
use cdk::mint_url::MintUrl;
use cdk::nuts::nut18::payment_request::PaymentRequest;
use cdk::nuts::CurrencyUnit;
use cdk::wallet::{
    types::{Transaction, TransactionDirection},
    SendOptions, Wallet,
};
use cdk::{amount::SplitTarget, Amount};
use cdk_sqlite::wallet::WalletSqliteDatabase;
use nostr::prelude::{Keys, SecretKey, ToBech32};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Cashu wallet hub
pub struct WalletHub {
    wallets: HashMap<String, Wallet>, // keyed by mint URL
    default_mint: Option<String>,
    storage: WalletStoragePaths,
    secrets: WalletSecrets,
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

/// Cashu token prepared for an external provider.
#[derive(Clone)]
pub struct ExternalPaymentToken {
    pub token: String,
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
    pub token: Option<String>,
}

/// Wallet balance information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletBalance {
    pub mint_url: String,
    pub balance: u64,
    pub unit: String,
    pub pending: u64,
}

/// Keyset information from mint
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeysetInfo {
    pub id: String,
    pub unit: String,
    pub active: bool,
}

#[derive(Debug, Clone)]
struct WalletStoragePaths {
    secrets_file: PathBuf,
    wallets_dir: PathBuf,
    mints_file: PathBuf,
}

impl WalletStoragePaths {
    fn new(base_dir: PathBuf) -> WalletResult<Self> {
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

    fn mint_db_path(&self, mint_url: &str) -> WalletResult<PathBuf> {
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
    #[serde(default)]
    units: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct WalletSecrets {
    wallet_seed: [u8; 64],
    nostr_keys: Keys,
}

impl WalletSecrets {
    fn load_or_create(paths: &WalletStoragePaths) -> WalletResult<Self> {
        if paths.secrets_file.exists() {
            let data = fs::read(&paths.secrets_file)?;
            let stored: StoredSecrets = serde_json::from_slice(&data)?;
            Self::from_stored(stored)
        } else {
            Self::generate_and_persist(paths)
        }
    }

    fn generate_and_persist(paths: &WalletStoragePaths) -> WalletResult<Self> {
        let mnemonic = Mnemonic::generate_in(Language::English, 12)
            .map_err(|e| WalletError::wallet(format!("Failed to generate mnemonic: {}", e)))?;
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

    fn from_stored(stored: StoredSecrets) -> WalletResult<Self> {
        if let Some(phrase) = stored.mnemonic {
            Self::from_mnemonic(phrase)
        } else {
            Err(WalletError::wallet("Wallet secrets file is empty"))
        }
    }

    fn from_mnemonic(phrase: String) -> WalletResult<Self> {
        let mnemonic = Mnemonic::parse_in(Language::English, phrase.trim())
            .map_err(|e| WalletError::wallet(format!("Invalid mnemonic: {}", e)))?;
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

    pub(crate) fn nostr_npub(&self) -> WalletResult<String> {
        self.nostr_keys
            .public_key()
            .to_bech32()
            .map_err(|e| WalletError::wallet(format!("Failed to encode npub: {}", e)))
    }
}

fn derive_nostr_keys_from_seed(seed: &[u8; 64]) -> WalletResult<Keys> {
    let hash = Sha256::digest(seed);
    let secret_key = SecretKey::from_slice(hash.as_slice())
        .map_err(|e| WalletError::wallet(format!("Failed to derive nostr key: {}", e)))?;
    Ok(Keys::new(secret_key))
}

/// Discover available keysets from a mint
async fn discover_mint_keysets(mint_url: &str) -> WalletResult<Vec<KeysetInfo>> {
    let client = reqwest::Client::new();
    let keys_url = format!("{}/v1/keys", mint_url.trim_end_matches('/'));

    let response = client
        .get(&keys_url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| {
            WalletError::wallet(format!("Failed to fetch keysets from {}: {}", keys_url, e))
        })?;

    if !response.status().is_success() {
        return Err(WalletError::wallet(format!(
            "Failed to fetch keysets from {}: {}",
            keys_url,
            response.status()
        )));
    }

    let keys_data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| WalletError::wallet(format!("Failed to parse keysets response: {}", e)))?;

    let mut keysets = Vec::new();

    if let Some(keysets_obj) = keys_data.get("keysets") {
        if let Some(keysets_array) = keysets_obj.as_array() {
            for keyset in keysets_array {
                if let (Some(id), Some(unit)) = (keyset.get("id"), keyset.get("unit")) {
                    if let (Some(id_str), Some(unit_str)) = (id.as_str(), unit.as_str()) {
                        keysets.push(KeysetInfo {
                            id: id_str.to_string(),
                            unit: unit_str.to_string(),
                            active: keyset
                                .get("active")
                                .and_then(|a| a.as_bool())
                                .unwrap_or(true),
                        });
                    }
                }
            }
        }
    }

    if keysets.is_empty() {
        keysets.push(KeysetInfo {
            id: "default".to_string(),
            unit: "msat".to_string(),
            active: true,
        });
    }

    Ok(keysets)
}

fn select_currency_unit(keysets: &[KeysetInfo]) -> CurrencyUnit {
    for keyset in keysets {
        if keyset.active && keyset.unit == "msat" {
            return CurrencyUnit::Msat;
        }
    }

    for keyset in keysets {
        if keyset.active && keyset.unit == "sat" {
            return CurrencyUnit::Sat;
        }
    }

    CurrencyUnit::Sat
}

fn sats_to_wallet_units(amount_sats: u64, unit: &CurrencyUnit) -> WalletResult<u64> {
    match unit.to_string().to_ascii_lowercase().as_str() {
        "sat" => Ok(amount_sats),
        "msat" => amount_sats
            .checked_mul(1_000)
            .ok_or_else(|| WalletError::wallet("Payment amount overflows msat wallet units")),
        other => Err(WalletError::wallet(format!(
            "Unsupported wallet currency unit for Routstr payment: {}",
            other
        ))),
    }
}

fn wallet_units_to_sats(amount: u64, unit: &CurrencyUnit) -> WalletResult<u64> {
    match unit.to_string().to_ascii_lowercase().as_str() {
        "sat" => Ok(amount),
        "msat" => Ok(amount / 1_000),
        other => Err(WalletError::wallet(format!(
            "Unsupported wallet currency unit for Routstr payment: {}",
            other
        ))),
    }
}

fn cashu_token_keyset_ids(token: &cdk::nuts::Token) -> Vec<String> {
    let mut keyset_ids: Vec<String> = match token {
        cdk::nuts::Token::TokenV3(token) => token
            .token
            .iter()
            .flat_map(|token| token.proofs.iter())
            .map(|proof| proof.keyset_id.to_string())
            .collect(),
        cdk::nuts::Token::TokenV4(token) => token
            .token
            .iter()
            .map(|token| token.keyset_id.to_string())
            .collect(),
    };
    keyset_ids.sort();
    keyset_ids.dedup();
    keyset_ids
}

fn ordered_mint_candidates<'a>(
    preferred_mint: Option<&str>,
    default_mint: Option<&str>,
    configured_mints: impl IntoIterator<Item = &'a str>,
    excluded_mints: &[String],
) -> Vec<String> {
    let mut remaining: Vec<String> = configured_mints
        .into_iter()
        .map(str::trim)
        .filter(|mint| !mint.is_empty())
        .map(str::to_string)
        .collect();
    remaining.sort();

    let mut candidates = Vec::new();
    for mint in preferred_mint
        .into_iter()
        .chain(default_mint)
        .chain(remaining.iter().map(String::as_str))
    {
        let mint = mint.trim();
        if mint.is_empty()
            || excluded_mints.iter().any(|excluded| excluded == mint)
            || candidates.iter().any(|candidate| candidate == mint)
        {
            continue;
        }
        candidates.push(mint.to_string());
    }
    candidates
}

impl WalletHub {
    pub fn clone_wallet_for_mint(&self, mint_url: &str) -> Option<Wallet> {
        self.wallets.get(mint_url).cloned()
    }

    pub fn new(base_dir: PathBuf) -> WalletResult<Self> {
        let storage = WalletStoragePaths::new(base_dir)?;
        let secrets = WalletSecrets::load_or_create(&storage)?;

        Ok(Self {
            wallets: HashMap::new(),
            default_mint: None,
            storage,
            secrets,
        })
    }

    fn load_mints_config(&self) -> WalletResult<StoredMints> {
        if self.storage.mints_file.exists() {
            let data = fs::read(&self.storage.mints_file)?;
            let stored: StoredMints = serde_json::from_slice(&data).unwrap_or_default();
            Ok(stored)
        } else {
            Ok(StoredMints::default())
        }
    }

    fn save_mints_config(&self) -> WalletResult<()> {
        let stored = StoredMints {
            mints: self.wallets.keys().cloned().collect(),
            default_mint: self.default_mint.clone(),
            units: self
                .wallets
                .iter()
                .map(|(mint_url, wallet)| (mint_url.clone(), wallet.unit.to_string()))
                .collect(),
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

    pub async fn load_existing_mints(&mut self) -> WalletResult<()> {
        let StoredMints {
            mints,
            default_mint,
            units,
        } = self.load_mints_config()?;

        for mint_url in mints {
            if !self.wallets.contains_key(&mint_url) {
                log::info!("Loading existing mint from storage: {}", mint_url);
                let load_result = if let Some(stored_unit) = units.get(&mint_url) {
                    match CurrencyUnit::from_str(stored_unit) {
                        Ok(unit) => self.add_mint_for_unit_internal(&mint_url, unit).await,
                        Err(error) => {
                            log::warn!(
                                "[wallet] Invalid stored unit for mint {}; rediscovering: {}",
                                mint_url,
                                error
                            );
                            self.add_mint_internal(&mint_url).await
                        }
                    }
                } else {
                    self.add_mint_internal(&mint_url).await
                };
                if let Err(error) = load_result {
                    log::warn!("Failed to load existing mint {}: {}", mint_url, error);
                }
            }
        }

        // If no mints were loaded, add a default one to ensure the wallet is usable
        if self.wallets.is_empty() {
            let default_public_mint = "https://8333.pw:8100";
            log::info!(
                "No mints configured, adding default: {}",
                default_public_mint
            );
            if let Err(e) = self.add_mint_internal(default_public_mint).await {
                log::warn!("Failed to add default mint {}: {}", default_public_mint, e);
            }
        }

        if let Some(default_mint) = default_mint {
            if self.wallets.contains_key(&default_mint) {
                self.default_mint = Some(default_mint);
            }
        }

        if self.default_mint.is_none() && !self.wallets.is_empty() {
            self.default_mint = self.wallets.keys().next().cloned();
        }

        Ok(())
    }

    fn default_mint_url(&self) -> WalletResult<&String> {
        self.default_mint
            .as_ref()
            .ok_or_else(|| WalletError::wallet("No default mint configured"))
    }

    fn get_wallet_by_url(&self, mint_url: &str) -> WalletResult<&Wallet> {
        self.wallets
            .get(mint_url)
            .ok_or_else(|| WalletError::wallet(format!("Mint not found: {}", mint_url)))
    }

    fn default_wallet(&self) -> WalletResult<&Wallet> {
        let mint = self.default_mint_url()?;
        self.get_wallet_by_url(mint)
    }

    fn wallet_for_payment_request(&self, request: &PaymentRequest) -> WalletResult<&Wallet> {
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

    async fn add_mint_internal(&mut self, mint_url: &str) -> WalletResult<()> {
        if self.wallets.contains_key(mint_url) {
            return Ok(());
        }

        let keysets = discover_mint_keysets(mint_url).await?;
        let currency_unit = select_currency_unit(&keysets);

        log::info!(
            "[wallet] Discovered {} keysets for mint {}; selected unit {}",
            keysets.len(),
            mint_url,
            currency_unit
        );

        self.add_mint_for_unit_internal(mint_url, currency_unit)
            .await
    }

    async fn add_mint_for_unit_internal(
        &mut self,
        mint_url: &str,
        currency_unit: CurrencyUnit,
    ) -> WalletResult<()> {
        if self
            .wallets
            .get(mint_url)
            .is_some_and(|wallet| wallet.unit == currency_unit)
        {
            return Ok(());
        }

        let wallet = self.create_wallet_for_unit(mint_url, currency_unit).await?;
        self.wallets.insert(mint_url.to_string(), wallet);

        if self.default_mint.is_none() {
            self.default_mint = Some(mint_url.to_string());
        }

        log::info!("[wallet] Mint wallet ready: {}", mint_url);
        Ok(())
    }

    async fn create_wallet_for_unit(
        &self,
        mint_url: &str,
        currency_unit: CurrencyUnit,
    ) -> WalletResult<Wallet> {
        let db_path = self.storage.mint_db_path(mint_url)?;
        let localstore = WalletSqliteDatabase::new(db_path).await.map_err(|e| {
            WalletError::wallet(format!(
                "Failed to open wallet database for mint {}: {}",
                mint_url, e
            ))
        })?;

        log::info!(
            "[wallet] Initializing mint wallet: mint={}, unit={}",
            mint_url,
            currency_unit
        );
        Wallet::new(
            mint_url,
            currency_unit,
            Arc::new(localstore),
            self.secrets.wallet_seed(),
            None,
        )
        .map_err(|e| {
            WalletError::wallet(format!(
                "Failed to create wallet for mint {}: {}",
                mint_url, e
            ))
        })
    }

    async fn evict_cached_token_keysets(
        &self,
        mint_url: &str,
        keyset_ids: &[String],
    ) -> WalletResult<()> {
        let db_path = self.storage.mint_db_path(mint_url)?;
        let localstore = WalletSqliteDatabase::new(db_path).await.map_err(|error| {
            WalletError::wallet(format!(
                "Failed to open wallet database for keyset refresh: {}",
                error
            ))
        })?;

        for keyset_id in keyset_ids {
            let Ok(keyset_id) = cdk::nuts::Id::from_str(keyset_id) else {
                log::debug!(
                    "[wallet] Skipping cache eviction for short keyset id {}",
                    keyset_id
                );
                continue;
            };
            localstore.remove_keys(&keyset_id).await.map_err(|error| {
                WalletError::wallet(format!(
                    "Failed to evict cached keyset {}: {}",
                    keyset_id, error
                ))
            })?;
            log::info!("[wallet] Evicted cached keyset keys: {}", keyset_id);
        }

        Ok(())
    }

    pub async fn add_mint(&mut self, mint_url: &str) -> WalletResult<()> {
        self.add_mint_internal(mint_url).await?;
        self.save_mints_config()?;
        Ok(())
    }

    pub async fn set_default_mint(&mut self, mint_url: &str) -> WalletResult<()> {
        if !self.wallets.contains_key(mint_url) {
            self.add_mint_internal(mint_url).await?;
        }

        self.default_mint = Some(mint_url.to_string());
        self.save_mints_config()?;
        Ok(())
    }

    pub async fn remove_mint(&mut self, mint_url: &str) -> WalletResult<()> {
        if !self.wallets.contains_key(mint_url) {
            return Err(WalletError::wallet(format!("Mint not found: {}", mint_url)));
        }

        let balance = self.get_balance(mint_url).await?;
        if balance > 0 {
            return Err(WalletError::wallet(format!(
                "Cannot remove mint with remaining balance: {} sats. Please spend or transfer tokens first.",
                balance
            )));
        }

        self.wallets.remove(mint_url);

        if let Some(ref default) = self.default_mint {
            if default == mint_url {
                self.default_mint = if self.wallets.is_empty() {
                    None
                } else {
                    self.wallets.keys().next().cloned()
                };
            }
        }

        self.save_mints_config()?;

        log::info!("Removed mint from wallet: {}", mint_url);
        Ok(())
    }

    pub fn nostr_npub(&self) -> Option<String> {
        self.secrets.nostr_npub().ok()
    }

    pub fn get_keys(&self) -> nostr::Keys {
        self.secrets.nostr_keys.clone()
    }

    pub async fn get_balance(&self, mint_url: &str) -> WalletResult<u64> {
        let wallet = self
            .wallets
            .get(mint_url)
            .ok_or_else(|| WalletError::wallet(format!("Mint not found: {}", mint_url)))?;

        let balance = wallet
            .total_balance()
            .await
            .map_err(|e| WalletError::wallet(format!("Failed to get balance: {}", e)))?;

        Ok(balance.into())
    }

    pub async fn get_all_balances(&self) -> WalletResult<Vec<WalletBalance>> {
        let mut balances = Vec::new();

        for (mint_url, wallet) in &self.wallets {
            let balance = wallet.total_balance().await.map_err(|e| {
                WalletError::wallet(format!("Failed to get balance for {}: {}", mint_url, e))
            })?;

            balances.push(WalletBalance {
                mint_url: mint_url.clone(),
                balance: balance.into(),
                unit: wallet.unit.to_string(),
                pending: 0,
            });
        }

        Ok(balances)
    }

    pub async fn summary(&self) -> WalletResult<WalletSummary> {
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

    pub fn create_nut18_payment_request(
        &self,
        amount: Option<u64>,
        description: Option<String>,
    ) -> WalletResult<Nut18PaymentRequestInfo> {
        if self.wallets.is_empty() {
            return Err(WalletError::wallet(
                "Add a mint before generating a payment request",
            ));
        }

        let mint_urls: Vec<MintUrl> = self
            .wallets
            .keys()
            .map(|url| {
                MintUrl::from_str(url)
                    .map_err(|e| WalletError::wallet(format!("Invalid mint URL {}: {}", url, e)))
            })
            .collect::<Result<_, _>>()?;

        let default_wallet = self.default_wallet()?;
        let payment_unit = default_wallet.unit.clone();

        let mut builder = PaymentRequest::builder()
            .unit(payment_unit.clone())
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
            unit: payment_unit.to_string(),
            description,
            mints: mint_urls.into_iter().map(|m| m.to_string()).collect(),
        };

        Ok(response)
    }

    pub async fn create_bolt11_invoice(
        &self,
        amount: u64,
        description: Option<String>,
    ) -> WalletResult<Bolt11InvoiceInfo> {
        let wallet = self.default_wallet()?;
        let quote = wallet
            .mint_quote(Amount::from(amount), description.clone())
            .await
            .map_err(|e| WalletError::wallet(format!("Failed to request mint quote: {}", e)))?;

        Ok(Bolt11InvoiceInfo {
            quote_id: quote.id.clone(),
            request: quote.request.clone(),
            amount: quote.amount.map(u64::from),
            unit: quote.unit.to_string(),
            expiry: quote.expiry,
            mint_url: quote.mint_url.to_string(),
        })
    }

    pub async fn pay_nut18_payment_request(
        &self,
        request: &str,
        custom_amount: Option<u64>,
    ) -> WalletResult<()> {
        let payment_request = PaymentRequest::from_str(request)
            .map_err(|e| WalletError::wallet(format!("Invalid payment request: {}", e)))?;

        if payment_request.transports.is_empty() {
            return Err(WalletError::wallet(
                "Payment request has no transport defined. Use pay_nut18_payment_request_with_token to get the token."
            ));
        }

        let wallet = self.wallet_for_payment_request(&payment_request)?;
        let custom_amount = custom_amount.map(Amount::from);

        let _ = wallet
            .pay_request(payment_request, custom_amount)
            .await
            .map_err(|e| WalletError::wallet(format!("Failed to pay request: {}", e)))?;

        Ok(())
    }

    pub async fn pay_nut18_payment_request_with_token(
        &self,
        request: &str,
        custom_amount: Option<u64>,
    ) -> WalletResult<PayNut18Result> {
        let payment_request = PaymentRequest::from_str(request)
            .map_err(|e| WalletError::wallet(format!("Invalid payment request: {}", e)))?;

        let wallet = self.wallet_for_payment_request(&payment_request)?;

        let amount = match payment_request.amount {
            Some(amount) => amount,
            None => match custom_amount {
                Some(a) => Amount::from(a),
                None => {
                    return Err(WalletError::wallet(
                        "Amount not specified in request and no custom amount provided",
                    ))
                }
            },
        };

        let amount_u64: u64 = amount.into();
        let has_transport = !payment_request.transports.is_empty();

        if has_transport {
            wallet
                .pay_request(payment_request, custom_amount.map(Amount::from))
                .await
                .map_err(|e| WalletError::wallet(format!("Failed to pay request: {}", e)))?;

            Ok(PayNut18Result {
                amount: amount_u64,
                token: None,
            })
        } else {
            let prepared_send = wallet
                .prepare_send(
                    amount,
                    SendOptions {
                        include_fee: true,
                        ..Default::default()
                    },
                )
                .await
                .map_err(|e| WalletError::wallet(format!("Failed to prepare send: {}", e)))?;

            let token = prepared_send
                .confirm(None)
                .await
                .map_err(|e| WalletError::wallet(format!("Failed to confirm send: {}", e)))?;

            Ok(PayNut18Result {
                amount: amount_u64,
                token: Some(token.to_string()),
            })
        }
    }

    pub async fn pay_bolt11_invoice(&self, invoice: &str) -> WalletResult<Bolt11PaymentResult> {
        let wallet = self.default_wallet()?;
        let quote = wallet
            .melt_quote(invoice.to_string(), None)
            .await
            .map_err(|e| WalletError::wallet(format!("Failed to request melt quote: {}", e)))?;

        let melted = wallet
            .melt(&quote.id)
            .await
            .map_err(|e| WalletError::wallet(format!("Failed to pay invoice: {}", e)))?;
        let amount: u64 = melted.amount.into();
        let fee_paid: u64 = melted.fee_paid.into();

        Ok(Bolt11PaymentResult {
            amount,
            fee_paid,
            preimage: melted.preimage,
        })
    }

    pub async fn receive_cashu_token(&mut self, token: &str) -> WalletResult<CashuReceiveResult> {
        log::info!(
            "[wallet] Cashu receive started: encoded_length={}",
            token.len()
        );

        let cashu_token = cdk::nuts::Token::from_str(token).map_err(|error| {
            log::error!(
                "[wallet] Cashu token parsing failed: encoded_length={}, error={}",
                token.len(),
                error
            );
            WalletError::wallet(format!("Invalid Cashu token: {}", error))
        })?;

        let token_kind = match &cashu_token {
            cdk::nuts::Token::TokenV3(_) => "v3",
            cdk::nuts::Token::TokenV4(_) => "v4",
        };
        // CDK treats V3 tokens without an explicit unit as sats.
        let token_unit = cashu_token.unit().unwrap_or_default();
        let token_amount: u64 = cashu_token
            .value()
            .map_err(|error| {
                WalletError::wallet(format!("Failed to read Cashu token amount: {}", error))
            })?
            .into();
        let mint_url = cashu_token
            .mint_url()
            .map_err(|error| {
                WalletError::wallet(format!("Failed to get Cashu token mint URL: {}", error))
            })?
            .to_string();

        let token_keyset_ids = cashu_token_keyset_ids(&cashu_token);
        let token_keysets = token_keyset_ids.join(",");
        log::info!(
            "[wallet] Cashu token decoded: version={}, mint={}, unit={}, amount={}, keysets=[{}]",
            token_kind,
            mint_url,
            token_unit,
            token_amount,
            token_keysets
        );

        let current_unit = self
            .wallets
            .get(&mint_url)
            .map(|wallet| wallet.unit.clone());
        let install_wallet_after_receive = current_unit.as_ref() != Some(&token_unit);
        let wallet = if install_wallet_after_receive {
            if let Some(current_unit) = current_unit {
                log::warn!(
                    "[wallet] Preparing token-unit wallet: mint={}, current_unit={}, token_unit={}",
                    mint_url,
                    current_unit,
                    token_unit
                );
            } else {
                log::info!(
                    "[wallet] Token mint is not configured; preparing mint={}, unit={}",
                    mint_url,
                    token_unit
                );
            }

            self.create_wallet_for_unit(&mint_url, token_unit.clone())
                .await
                .map_err(|error| {
                    log::error!(
                        "[wallet] Failed to initialize token mint wallet: mint={}, unit={}, error={}",
                        mint_url,
                        token_unit,
                        error
                    );
                    error
                })?
        } else {
            self.wallets.get(&mint_url).cloned().ok_or_else(|| {
                WalletError::wallet(format!("Failed to find wallet for mint: {}", mint_url))
            })?
        };

        // The pinned CDK 0.13 fork trusts cached key material. Force the
        // token's public keysets to be fetched and cryptographically verified
        // before receive mutates proof state. This avoids retrying a partially
        // started receive and fixes stale/corrupt key caches.
        self.evict_cached_token_keysets(&mint_url, &token_keyset_ids)
            .await
            .map_err(|error| {
                log::error!(
                    "[wallet] Cached keyset eviction failed: mint={}, keysets=[{}], error={}",
                    mint_url,
                    token_keysets,
                    error
                );
                error
            })?;
        let refreshed_keysets = wallet.refresh_keysets().await.map_err(|error| {
            log::error!(
                "[wallet] Mint keyset preflight failed: mint={}, unit={}, keysets=[{}], error={}",
                mint_url,
                token_unit,
                token_keysets,
                error
            );
            WalletError::wallet(format!(
                "The token uses keyset(s) [{}], but {} did not return matching keys: {}",
                token_keysets, mint_url, error
            ))
        })?;
        log::info!(
            "[wallet] Keyset preflight completed: mint={}, refreshed_keysets={}, token_keysets=[{}]",
            mint_url,
            refreshed_keysets.len(),
            token_keysets
        );

        let received_amount = wallet
            .receive(token, cdk::wallet::ReceiveOptions::default())
            .await
            .map_err(|error| {
                log::error!(
                    "[wallet] Cashu receive failed after keyset preflight: mint={}, unit={}, amount={}, keysets=[{}], error={}",
                    mint_url,
                    token_unit,
                    token_amount,
                    token_keysets,
                    error
                );
                WalletError::wallet(format!(
                    "Failed to receive Cashu token from {} ({}) after verifying keysets [{}]: {}",
                    mint_url, token_unit, token_keysets, error
                ))
            })?;

        if install_wallet_after_receive {
            self.wallets.insert(mint_url.clone(), wallet);
            if self.default_mint.is_none() {
                self.default_mint = Some(mint_url.clone());
            }
            if let Err(error) = self.save_mints_config() {
                log::error!(
                    "[wallet] Token was received, but mint configuration could not be saved: mint={}, error={}",
                    mint_url,
                    error
                );
            }
            log::info!(
                "[wallet] Installed token-unit wallet after successful receive: mint={}, unit={}",
                mint_url,
                token_unit
            );
        }

        let received_wallet_units: u64 = received_amount.into();
        let total_amount_sats = wallet_units_to_sats(received_wallet_units, &token_unit)?;

        log::info!(
            "[wallet] Cashu receive completed: mint={}, unit={}, received_units={}, received_sats={}, keysets=[{}]",
            mint_url,
            token_unit,
            received_wallet_units,
            total_amount_sats,
            token_keysets
        );
        Ok(CashuReceiveResult {
            amount: total_amount_sats,
            mint_url,
        })
    }

    pub async fn list_transactions(
        &self,
        direction: Option<TransactionDirection>,
    ) -> WalletResult<Vec<WalletTransactionEntry>> {
        let mut transactions: Vec<WalletTransactionEntry> = Vec::new();

        for wallet in self.wallets.values() {
            let mut wallet_transactions = wallet
                .list_transactions(direction)
                .await
                .map_err(|e| WalletError::wallet(format!("Failed to list transactions: {}", e)))?;
            transactions.extend(wallet_transactions.drain(..).map(Into::into));
        }

        transactions.sort_by_key(|transaction| std::cmp::Reverse(transaction.timestamp));
        Ok(transactions)
    }

    pub async fn create_external_token(
        &self,
        amount_sats: u64,
        mint_url: Option<String>,
    ) -> WalletResult<String> {
        if let Some(mint_url) = mint_url {
            return self
                .create_external_token_from_mint(amount_sats, &mint_url)
                .await
                .map(|payment| payment.token);
        }

        self.create_external_token_with_fallback(amount_sats, None, &[])
            .await
            .map(|payment| payment.token)
    }

    /// Create a token from the preferred mint, then deterministically try other
    /// funded mints that have not already failed for this provider request.
    pub async fn create_external_token_with_fallback(
        &self,
        amount_sats: u64,
        preferred_mint: Option<String>,
        excluded_mints: &[String],
    ) -> WalletResult<ExternalPaymentToken> {
        let candidates = ordered_mint_candidates(
            preferred_mint.as_deref(),
            self.default_mint.as_deref(),
            self.wallets.keys().map(String::as_str),
            excluded_mints,
        );

        if candidates.is_empty() {
            return Err(WalletError::wallet("No eligible mints configured"));
        }

        let mut total_balance_sats = 0_u64;
        let mut last_error = None;

        for (index, mint_url) in candidates.iter().enumerate() {
            let wallet = match self.wallets.get(mint_url) {
                Some(wallet) => wallet,
                None => continue,
            };

            let raw_balance: u64 = match wallet.total_balance().await {
                Ok(balance) => balance.into(),
                Err(error) => {
                    last_error = Some(format!("Failed to get balance for {}: {}", mint_url, error));
                    continue;
                }
            };
            let balance_sats = wallet_units_to_sats(raw_balance, &wallet.unit)?;
            total_balance_sats = total_balance_sats.saturating_add(balance_sats);
            if balance_sats < amount_sats {
                continue;
            }

            if index > 0 {
                log::info!(
                    "[wallet] Retrying payment token with fallback mint {} ({}/{})",
                    mint_url,
                    index + 1,
                    candidates.len()
                );
            }

            match self
                .create_external_token_from_mint(amount_sats, mint_url)
                .await
            {
                Ok(payment) => return Ok(payment),
                Err(error) => {
                    log::warn!(
                        "[wallet] Could not create payment token from mint {}: {}",
                        mint_url,
                        error
                    );
                    last_error = Some(error.to_string());
                }
            }
        }

        Err(WalletError::wallet(last_error.unwrap_or_else(|| {
            format!(
                "Insufficient balance: {} sats available across eligible mints, {} sats requested",
                total_balance_sats, amount_sats
            )
        })))
    }

    async fn create_external_token_from_mint(
        &self,
        amount_sats: u64,
        mint_url: &str,
    ) -> WalletResult<ExternalPaymentToken> {
        let wallet = self.wallets.get(mint_url).ok_or_else(|| {
            WalletError::wallet(format!("Wallet not found for mint: {}", mint_url))
        })?;

        let raw_balance: u64 = wallet
            .total_balance()
            .await
            .map_err(|e| WalletError::wallet(format!("Failed to get balance: {}", e)))?
            .into();
        let balance_sats = wallet_units_to_sats(raw_balance, &wallet.unit)?;
        if balance_sats < amount_sats {
            return Err(WalletError::wallet(format!(
                "Insufficient balance: {} sats available, {} sats requested",
                balance_sats, amount_sats
            )));
        }

        let wallet_amount = sats_to_wallet_units(amount_sats, &wallet.unit)?;
        let prepared_send = wallet
            .prepare_send(
                cdk::Amount::from(wallet_amount),
                cdk::wallet::SendOptions::default(),
            )
            .await
            .map_err(|e| WalletError::wallet(format!("Failed to prepare token: {}", e)))?;

        let token = prepared_send
            .confirm(None)
            .await
            .map_err(|e| WalletError::wallet(format!("Failed to create token: {}", e)))?;

        log::info!(
            "[wallet] Created external token for {} sats from mint {}",
            amount_sats,
            mint_url
        );
        Ok(ExternalPaymentToken {
            token: token.to_string(),
            mint_url: mint_url.to_string(),
        })
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
            token: None,
        }
    }
}

/// Main Wallet service
pub struct WalletService {
    wallet: Arc<Mutex<WalletHub>>,
}

impl WalletService {
    pub async fn new(base_dir: PathBuf) -> WalletResult<Self> {
        let mut wallet = WalletHub::new(base_dir)?;
        wallet.load_existing_mints().await?;
        Ok(Self {
            wallet: Arc::new(Mutex::new(wallet)),
        })
    }

    #[allow(dead_code)]
    pub fn get_wallet(&self) -> Arc<Mutex<WalletHub>> {
        self.wallet.clone()
    }

    pub async fn add_mint(&self, mint_url: &str) -> WalletResult<()> {
        let mut wallet = self.wallet.lock().await;
        wallet.add_mint(mint_url).await
    }

    pub async fn set_default_mint(&self, mint_url: &str) -> WalletResult<()> {
        let mut wallet = self.wallet.lock().await;
        wallet.set_default_mint(mint_url).await
    }

    pub async fn remove_mint(&self, mint_url: &str) -> WalletResult<()> {
        let mut wallet = self.wallet.lock().await;
        wallet.remove_mint(mint_url).await
    }

    pub async fn get_wallet_balance(&self) -> WalletResult<u64> {
        let wallet = self.wallet.lock().await;
        let balances = wallet.get_all_balances().await?;
        Ok(balances.iter().map(|b| b.balance).sum())
    }

    pub async fn get_wallet_summary(&self) -> WalletResult<WalletSummary> {
        let wallet = self.wallet.lock().await;
        wallet.summary().await
    }

    pub async fn list_wallet_transactions(&self) -> WalletResult<Vec<WalletTransactionEntry>> {
        let wallet = self.wallet.lock().await;
        wallet.list_transactions(None).await
    }

    pub async fn create_nut18_payment_request(
        &self,
        amount: Option<u64>,
        description: Option<String>,
    ) -> WalletResult<Nut18PaymentRequestInfo> {
        let wallet = self.wallet.lock().await;
        wallet.create_nut18_payment_request(amount, description)
    }

    pub async fn create_bolt11_invoice(
        &self,
        amount: u64,
        description: Option<String>,
    ) -> WalletResult<Bolt11InvoiceInfo> {
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

    fn spawn_mint_quote_monitor(&self, mint_url: String, quote_id: String, expiry: u64) {
        let wallet = self.wallet.clone();

        tokio::spawn(async move {
            let poll_interval = Duration::from_secs(3);

            loop {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                if now >= expiry {
                    log::warn!("Mint quote {} for mint {} expired", quote_id, mint_url);
                    break;
                }

                let mint_wallet = {
                    let guard = wallet.lock().await;
                    guard.clone_wallet_for_mint(&mint_url)
                };

                let Some(mint_wallet) = mint_wallet else {
                    break;
                };

                if let Ok(status) = mint_wallet.mint_quote_state(&quote_id).await {
                    if status.state == cdk::nuts::MintQuoteState::Paid {
                        let _ = mint_wallet
                            .mint(&status.quote, SplitTarget::default(), None)
                            .await;
                        log::info!("Minted tokens for quote {} at mint {}", quote_id, mint_url);
                        break;
                    }
                }

                tokio::time::sleep(poll_interval).await;
            }
        });
    }

    pub async fn pay_nut18_payment_request(
        &self,
        request: &str,
        custom_amount: Option<u64>,
    ) -> WalletResult<()> {
        let wallet = self.wallet.lock().await;
        wallet
            .pay_nut18_payment_request(request, custom_amount)
            .await
    }

    pub async fn pay_nut18_payment_request_with_token(
        &self,
        request: &str,
        custom_amount: Option<u64>,
    ) -> WalletResult<PayNut18Result> {
        let wallet = self.wallet.lock().await;
        wallet
            .pay_nut18_payment_request_with_token(request, custom_amount)
            .await
    }

    pub async fn pay_bolt11_invoice(&self, invoice: &str) -> WalletResult<Bolt11PaymentResult> {
        let wallet = self.wallet.lock().await;
        wallet.pay_bolt11_invoice(invoice).await
    }

    pub async fn receive_cashu_token(&self, token: &str) -> WalletResult<CashuReceiveResult> {
        let mut wallet = self.wallet.lock().await;
        wallet.receive_cashu_token(token).await
    }

    pub async fn create_external_token(
        &self,
        amount_sats: u64,
        mint_url: Option<String>,
    ) -> WalletResult<String> {
        let wallet = self.wallet.lock().await;
        wallet.create_external_token(amount_sats, mint_url).await
    }

    pub async fn create_external_token_with_fallback(
        &self,
        amount_sats: u64,
        preferred_mint: Option<String>,
        excluded_mints: &[String],
    ) -> WalletResult<ExternalPaymentToken> {
        let wallet = self.wallet.lock().await;
        wallet
            .create_external_token_with_fallback(amount_sats, preferred_mint, excluded_mints)
            .await
    }

    pub async fn get_wallet_keys(&self) -> nostr::Keys {
        let wallet = self.wallet.lock().await;
        wallet.get_keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_msat_default_when_mint_supports_sat_and_msat() {
        let keysets = vec![
            KeysetInfo {
                id: "msat-keyset".to_string(),
                unit: "msat".to_string(),
                active: true,
            },
            KeysetInfo {
                id: "sat-keyset".to_string(),
                unit: "sat".to_string(),
                active: true,
            },
        ];

        assert_eq!(select_currency_unit(&keysets), CurrencyUnit::Msat);
    }

    #[test]
    fn uses_msat_when_it_is_the_only_active_unit() {
        let keysets = vec![KeysetInfo {
            id: "msat-keyset".to_string(),
            unit: "msat".to_string(),
            active: true,
        }];

        assert_eq!(select_currency_unit(&keysets), CurrencyUnit::Msat);
    }

    #[test]
    fn stored_mints_without_units_remain_backward_compatible() {
        let stored: StoredMints = serde_json::from_str(
            r#"{"mints":["https://mint.example"],"default_mint":"https://mint.example"}"#,
        )
        .unwrap();

        assert!(stored.units.is_empty());
    }

    #[test]
    fn stored_mints_preserve_selected_wallet_unit() {
        let stored = StoredMints {
            mints: vec!["https://mint.example".to_string()],
            default_mint: Some("https://mint.example".to_string()),
            units: HashMap::from([("https://mint.example".to_string(), "sat".to_string())]),
        };
        let decoded: StoredMints =
            serde_json::from_slice(&serde_json::to_vec(&stored).unwrap()).unwrap();

        assert_eq!(
            decoded.units.get("https://mint.example"),
            Some(&"sat".to_string())
        );
    }

    #[test]
    fn converts_sats_for_sat_and_msat_wallets() {
        assert_eq!(sats_to_wallet_units(21, &CurrencyUnit::Sat).unwrap(), 21);
        assert_eq!(
            sats_to_wallet_units(21, &CurrencyUnit::Msat).unwrap(),
            21_000
        );
        assert_eq!(
            wallet_units_to_sats(21_999, &CurrencyUnit::Msat).unwrap(),
            21
        );
    }

    #[test]
    fn rejects_msat_conversion_overflow() {
        assert!(sats_to_wallet_units(u64::MAX, &CurrencyUnit::Msat).is_err());
    }

    #[test]
    fn orders_and_deduplicates_fallback_mints() {
        let configured = [
            "https://mint-c.example",
            "https://mint-a.example",
            "https://mint-b.example",
        ];
        let excluded = vec!["https://mint-a.example".to_string()];

        assert_eq!(
            ordered_mint_candidates(
                Some("https://mint-a.example"),
                Some("https://mint-b.example"),
                configured,
                &excluded,
            ),
            vec![
                "https://mint-b.example".to_string(),
                "https://mint-c.example".to_string(),
            ]
        );
    }
}
