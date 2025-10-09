use anyhow::{anyhow, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RoutstrStoragePaths {
    pub config_file: PathBuf,
}

impl RoutstrStoragePaths {
    fn new() -> Result<Self> {
        let project_dirs = ProjectDirs::from("com", "Tollgate", "TollgateApp")
            .ok_or_else(|| anyhow!("Unable to determine Routstr storage directory"))?;

        let base_dir = project_dirs.data_dir().join("routstr");
        let config_file = base_dir.join("config.json");

        // Ensure directories exist
        fs::create_dir_all(&base_dir)?;

        Ok(Self { config_file })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiKeyEntry {
    pub api_key: String,
    pub creation_cashu_token: Option<String>,
    pub created_at: u64,
    pub alias: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct RoutstrStoredConfig {
    pub base_url: Option<String>,
    pub api_keys: Vec<ApiKeyEntry>,
    pub use_proxy: bool,
    pub proxy_endpoint: Option<String>,
    pub target_service_url: Option<String>,
    pub use_onion: bool,
    pub payment_required: bool,
    pub cost_per_request_sats: u64,
    pub use_manual_url: bool,
    pub selected_provider_id: Option<String>,
    pub service_mode: String,
    pub selected_mint_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Architecture {
    pub modality: Option<String>,
    pub input_modalities: Vec<String>,
    pub output_modalities: Vec<String>,
    pub tokenizer: Option<String>,
    pub instruct_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Pricing {
    pub prompt: f64,
    pub completion: f64,
    pub request: f64,
    pub image: f64,
    pub web_search: f64,
    pub internal_reasoning: f64,
    pub max_prompt_cost: f64,
    pub max_completion_cost: f64,
    pub max_cost: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SatsPricing {
    pub prompt: f64,
    pub completion: f64,
    pub request: f64,
    pub image: f64,
    pub web_search: f64,
    pub internal_reasoning: f64,
    pub max_prompt_cost: Option<f64>,
    pub max_completion_cost: Option<f64>,
    pub max_cost: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TopProvider {
    pub context_length: u32,
    pub max_completion_tokens: Option<u32>,
    pub is_moderated: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RoutstrModel {
    pub id: String,
    pub name: String,
    pub created: u64,
    pub description: Option<String>,
    pub context_length: Option<u32>,
    pub architecture: Option<Architecture>,
    pub pricing: Option<Pricing>,
    pub sats_pricing: Option<SatsPricing>,
    pub per_request_limits: Option<serde_json::Value>,
    pub top_provider: Option<TopProvider>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelsResponse {
    pub data: Vec<RoutstrModel>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RoutstrWalletBalance {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    pub balance: u64,
    pub reserved: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RoutstrTopUpRequest {
    pub cashu_token: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RoutstrTopUpResponse {
    pub msats: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RoutstrCreateResponse {
    pub api_key: String,
    pub balance: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RoutstrRefundResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sats: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msats: Option<String>,
}

#[derive(Debug)]
pub struct RoutstrService {
    pub base_url: Option<String>,
    pub models: Vec<RoutstrModel>,
    pub api_keys: Vec<ApiKeyEntry>,
    pub use_proxy: bool,
    pub proxy_endpoint: Option<String>,
    pub target_service_url: Option<String>,
    pub use_onion: bool,
    pub payment_required: bool,
    pub cost_per_request_sats: u64,
    pub use_manual_url: bool,
    pub selected_provider_id: Option<String>,
    pub service_mode: String,
    pub selected_mint_url: Option<String>,
    client: reqwest::Client,
    storage: RoutstrStoragePaths,
    auto_update_handle: Option<tokio::task::JoinHandle<()>>,
}

impl Clone for RoutstrService {
    fn clone(&self) -> Self {
        Self {
            base_url: self.base_url.clone(),
            models: self.models.clone(),
            api_keys: self.api_keys.clone(),
            use_proxy: self.use_proxy,
            proxy_endpoint: self.proxy_endpoint.clone(),
            target_service_url: self.target_service_url.clone(),
            use_onion: self.use_onion,
            payment_required: self.payment_required,
            cost_per_request_sats: self.cost_per_request_sats,
            use_manual_url: self.use_manual_url,
            selected_provider_id: self.selected_provider_id.clone(),
            service_mode: self.service_mode.clone(),
            selected_mint_url: self.selected_mint_url.clone(),
            client: self.client.clone(),
            storage: self.storage.clone(),
            auto_update_handle: None, // Don't clone the handle
        }
    }
}

impl Default for RoutstrService {
    fn default() -> Self {
        Self::new()
    }
}

impl RoutstrService {
    pub fn new() -> Self {
        let storage = RoutstrStoragePaths::new().unwrap_or_else(|e| {
            log::error!("Failed to initialize Routstr storage: {}", e);
            // Fallback to defaults if storage init fails
            RoutstrStoragePaths {
                config_file: std::env::temp_dir()
                    .join("routstr_fallback")
                    .join("config.json"),
            }
        });

        let mut service = Self {
            base_url: None,
            models: Vec::new(),
            api_keys: Vec::new(),
            use_proxy: false,
            proxy_endpoint: None,
            target_service_url: None,
            use_onion: false,
            payment_required: false,
            cost_per_request_sats: 10,
            use_manual_url: true,
            selected_provider_id: None,
            service_mode: "wallet".to_string(),
            selected_mint_url: None,
            client: reqwest::Client::new(),
            storage,
            auto_update_handle: None,
        };

        // Load existing configuration
        if let Err(e) = service.load_config() {
            log::warn!("Failed to load Routstr configuration: {}", e);
        }

        service
    }

    pub async fn connect_to_service(&mut self, url: String) -> Result<()> {
        // Validate and format URL
        let formatted_url = if url.starts_with("http://") || url.starts_with("https://") {
            url.trim_end_matches('/').to_string()
        } else {
            format!("https://{}", url.trim_end_matches('/'))
        };

        // Test connection by fetching models
        let models_url = format!("{}/v1/models", formatted_url);

        let response = self
            .client
            .get(&models_url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to connect to Routstr service: HTTP {}",
                response.status()
            ));
        }

        let models_response: ModelsResponse = response.json().await?;

        self.base_url = Some(formatted_url);
        self.models = models_response.data;

        // Persist updated configuration
        if let Err(e) = self.save_config() {
            log::error!("Failed to save connection configuration: {}", e);
        }

        log::info!(
            "Successfully connected to Routstr service at {} with {} models",
            self.base_url.as_ref().unwrap(),
            self.models.len()
        );

        Ok(())
    }

    pub fn start_auto_update(&mut self, state: RoutstrState) {
        // Cancel any existing auto-update task
        self.stop_auto_update();

        let handle = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60)); // Update every minute

            loop {
                interval.tick().await;

                let mut service = state.lock().await;
                if service.base_url.is_some() {
                    if let Err(e) = service.refresh_models().await {
                        log::warn!("Auto-refresh models failed: {}", e);
                    } else {
                        log::debug!("Auto-refreshed models successfully");
                    }
                } else {
                    // If no longer connected, stop the auto-update task
                    log::debug!("No longer connected, stopping auto-update task");
                    break;
                }
                drop(service); // Release the lock
            }
        });

        self.auto_update_handle = Some(handle);
        log::info!("Started automatic model price updates (every 60 seconds)");
    }

    pub fn stop_auto_update(&mut self) {
        if let Some(handle) = self.auto_update_handle.take() {
            handle.abort();
            log::info!("Stopped automatic model price updates");
        }
    }

    pub fn set_selected_mint(&mut self, mint_url: Option<String>) {
        self.selected_mint_url = mint_url;

        // Persist updated configuration
        if let Err(e) = self.save_config() {
            log::error!("Failed to save mint selection: {}", e);
        }
    }

    pub fn get_selected_mint(&self) -> Option<&String> {
        self.selected_mint_url.as_ref()
    }

    pub async fn refresh_models(&mut self) -> Result<()> {
        let base_url = self
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to any Routstr service"))?;

        let models_url = format!("{}/v1/models", base_url);

        let response = self
            .client
            .get(&models_url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Failed to refresh models: HTTP {}",
                response.status()
            ));
        }

        let models_response: ModelsResponse = response.json().await?;
        self.models = models_response.data;

        log::info!(
            "Refreshed {} models from Routstr service",
            self.models.len()
        );

        Ok(())
    }

    pub fn get_models(&self) -> &Vec<RoutstrModel> {
        &self.models
    }

    pub fn get_base_url(&self) -> Option<&String> {
        self.base_url.as_ref()
    }

    pub fn is_connected(&self) -> bool {
        self.base_url.is_some()
    }

    pub fn disconnect(&mut self) {
        // Stop auto-update task first
        self.stop_auto_update();

        self.base_url = None;
        self.models.clear();

        // Persist updated configuration
        if let Err(e) = self.save_config() {
            log::error!("Failed to save disconnect configuration: {}", e);
        }

        log::info!("Disconnected from Routstr service");
    }

    pub fn clear_config(&mut self) -> Result<()> {
        self.models.clear();
        self.api_keys.clear();
        Ok(())
    }

    pub fn add_api_key(
        &mut self,
        api_key: String,
        creation_cashu_token: Option<String>,
        alias: Option<String>,
    ) {
        let entry = ApiKeyEntry {
            api_key,
            creation_cashu_token,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            alias,
        };
        self.api_keys.push(entry);

        // Persist updated configuration
        if let Err(e) = self.save_config() {
            log::error!("Failed to save API key configuration: {}", e);
        }

        log::info!("Added new API key to Routstr service");
    }

    pub fn get_api_keys(&self) -> &Vec<ApiKeyEntry> {
        &self.api_keys
    }

    pub fn remove_api_key(&mut self, api_key: &str) -> bool {
        let initial_len = self.api_keys.len();
        self.api_keys.retain(|entry| entry.api_key != api_key);

        if self.api_keys.len() < initial_len {
            // Persist updated configuration
            if let Err(e) = self.save_config() {
                log::error!("Failed to save API key configuration after removal: {}", e);
            }
            log::info!("Removed API key from Routstr service");
            true
        } else {
            false
        }
    }

    pub async fn create_wallet(
        &mut self,
        base_url: String,
        cashu_token: String,
    ) -> Result<RoutstrCreateResponse> {
        // Format URL
        let formatted_url = if base_url.starts_with("http://") || base_url.starts_with("https://") {
            base_url.trim_end_matches('/').to_string()
        } else {
            format!("https://{}", base_url.trim_end_matches('/'))
        };

        let create_url = format!("{}/v1/balance/create", formatted_url);

        log::info!(
            "Creating wallet at {} with cashu token prefix: {}...",
            create_url,
            &cashu_token.chars().take(8).collect::<String>()
        );

        let response = self
            .client
            .get(&create_url)
            .query(&[("initial_balance_token", &cashu_token)])
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            log::error!("Wallet creation failed: HTTP {} - {}", status, error_text);
            return Err(anyhow!(
                "Failed to create wallet: HTTP {} - {}",
                status,
                error_text
            ));
        }

        let create_response: RoutstrCreateResponse = response.json().await?;

        // Store the credentials and connection info
        self.base_url = Some(formatted_url);
        self.add_api_key(create_response.api_key.clone(), Some(cashu_token), None);

        // Persist the configuration
        if let Err(e) = self.save_config() {
            log::error!("Failed to save wallet creation configuration: {}", e);
        }

        log::info!(
            "Successfully created wallet with API key: {}...",
            &create_response.api_key.chars().take(8).collect::<String>()
        );

        Ok(create_response)
    }

    pub async fn create_balance_with_token(
        &mut self,
        cashu_token: String,
    ) -> Result<RoutstrCreateResponse> {
        let base_url = self
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to any Routstr service"))?;

        let create_url = format!("{}/v1/balance/create", base_url);

        log::info!(
            "Creating balance at {} with cashu token prefix: {}...",
            create_url,
            &cashu_token.chars().take(8).collect::<String>()
        );

        let response = self
            .client
            .get(&create_url)
            .query(&[("initial_balance_token", &cashu_token)])
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            log::error!("Balance creation failed: HTTP {} - {}", status, error_text);
            return Err(anyhow!(
                "Failed to create balance: HTTP {} - {}",
                status,
                error_text
            ));
        }

        let create_response: RoutstrCreateResponse = response.json().await?;

        // Store the credentials and creation token
        self.add_api_key(create_response.api_key.clone(), Some(cashu_token), None);

        // Persist the configuration
        if let Err(e) = self.save_config() {
            log::error!("Failed to save balance creation configuration: {}", e);
        }

        log::info!(
            "Successfully created balance with API key: {}...",
            &create_response.api_key.chars().take(8).collect::<String>()
        );

        Ok(create_response)
    }

    pub async fn get_wallet_balance_for_key(&self, api_key: &str) -> Result<RoutstrWalletBalance> {
        let base_url = self
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to any Routstr service"))?;

        let balance_url = format!("{}/v1/balance/info", base_url);

        log::info!(
            "Attempting balance check to {} with API key prefix: {}...",
            balance_url,
            &api_key.chars().take(8).collect::<String>()
        );

        let response = self
            .client
            .get(&balance_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            log::error!("Balance check failed: HTTP {} - {}", status, error_text);
            return Err(anyhow!(
                "Failed to get wallet balance: HTTP {} - {}",
                status,
                error_text
            ));
        }

        let mut balance: RoutstrWalletBalance = response.json().await?;
        balance.api_key = Some(api_key.to_string());

        log::info!(
            "Retrieved wallet balance: {} (reserved: {})",
            balance.balance,
            balance.reserved
        );

        Ok(balance)
    }

    pub async fn get_all_wallet_balances(&self) -> Vec<RoutstrWalletBalance> {
        let mut balances = Vec::new();
        for entry in &self.api_keys {
            match self.get_wallet_balance_for_key(&entry.api_key).await {
                Ok(balance) => balances.push(balance),
                Err(e) => {
                    log::warn!(
                        "Failed to get balance for API key {}: {}",
                        &entry.api_key.chars().take(8).collect::<String>(),
                        e
                    );
                }
            }
        }
        balances
    }

    pub async fn top_up_wallet_for_key(
        &self,
        api_key: &str,
        cashu_token: String,
    ) -> Result<RoutstrTopUpResponse> {
        let base_url = self
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to any Routstr service"))?;

        let topup_url = format!("{}/v1/balance/topup", base_url);

        let request_body = RoutstrTopUpRequest { cashu_token };

        log::info!(
            "Attempting topup to {} with API key prefix: {}...",
            topup_url,
            &api_key.chars().take(8).collect::<String>()
        );

        let response = self
            .client
            .post(&topup_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            log::error!("Topup failed: HTTP {} - {}", status, error_text);
            return Err(anyhow!(
                "Failed to top up wallet: HTTP {} - {}",
                status,
                error_text
            ));
        }

        let topup_response: RoutstrTopUpResponse = response.json().await?;

        log::info!("Topped up wallet: +{} msats", topup_response.msats);

        Ok(topup_response)
    }

    pub async fn refund_wallet_for_key(&self, api_key: &str) -> Result<RoutstrRefundResponse> {
        let base_url = self
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to any Routstr service"))?;

        let refund_url = format!("{}/v1/balance/refund", base_url);

        let response = self
            .client
            .post(&refund_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!(
                "Failed to refund wallet: HTTP {} - {}",
                status,
                error_text
            ));
        }

        let refund_response: RoutstrRefundResponse = response.json().await?;

        log::info!("Refunded wallet balance");

        Ok(refund_response)
    }

    fn load_config(&mut self) -> Result<()> {
        if self.storage.config_file.exists() {
            let data = fs::read(&self.storage.config_file)?;
            let stored: RoutstrStoredConfig = serde_json::from_slice(&data).unwrap_or_default();

            self.base_url = stored.base_url;
            self.api_keys = stored.api_keys;
            self.use_proxy = stored.use_proxy;
            self.proxy_endpoint = stored.proxy_endpoint;
            self.target_service_url = stored.target_service_url;
            self.use_onion = stored.use_onion;
            self.payment_required = stored.payment_required;
            self.cost_per_request_sats = stored.cost_per_request_sats;
            self.use_manual_url = stored.use_manual_url;
            self.selected_provider_id = stored.selected_provider_id;
            self.service_mode = stored.service_mode;
            self.selected_mint_url = stored.selected_mint_url;

            log::info!("Loaded Routstr configuration from storage");
        }
        Ok(())
    }

    fn save_config(&self) -> Result<()> {
        let config = RoutstrStoredConfig {
            base_url: self.base_url.clone(),
            api_keys: self.api_keys.clone(),
            use_proxy: self.use_proxy,
            proxy_endpoint: self.proxy_endpoint.clone(),
            target_service_url: self.target_service_url.clone(),
            use_onion: self.use_onion,
            payment_required: self.payment_required,
            cost_per_request_sats: self.cost_per_request_sats,
            use_manual_url: self.use_manual_url,
            selected_provider_id: self.selected_provider_id.clone(),
            service_mode: self.service_mode.clone(),
            selected_mint_url: self.selected_mint_url.clone(),
        };

        if let Some(parent) = self.storage.config_file.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(
            &self.storage.config_file,
            serde_json::to_vec_pretty(&config)?,
        )?;

        log::debug!("Saved Routstr configuration to storage");
        Ok(())
    }

    pub async fn force_reset_all_api_keys(&mut self) -> Result<()> {
        log::info!("Starting force reset of all API keys");

        for api_key_entry in &self.api_keys {
            match self.refund_wallet_for_key(&api_key_entry.api_key).await {
                Ok(_) => {
                    log::info!(
                        "Successfully refunded wallet for API key: {}...",
                        &api_key_entry.api_key.chars().take(8).collect::<String>()
                    );
                }
                Err(e) => {
                    log::warn!("Failed to refund wallet for API key: {}..., error: {} - continuing with force reset",
                        &api_key_entry.api_key.chars().take(8).collect::<String>(), e);
                }
            }
        }

        // Stop auto-update task
        self.stop_auto_update();

        self.api_keys.clear();
        self.models.clear();

        if let Err(e) = self.save_config() {
            log::error!("Failed to save configuration after force reset: {}", e);
        }

        log::info!("Force reset of all API keys completed");
        Ok(())
    }
}

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

pub type RoutstrState = Arc<Mutex<RoutstrService>>;

pub async fn initialize_routstr_auto_update(state: RoutstrState) {
    let mut service = state.lock().await;
    if service.is_connected() {
        service.start_auto_update(state.clone());
        log::info!("Initialized automatic model updates for existing Routstr connection");
    }
}

#[tauri::command]
pub async fn routstr_connect_service(
    url: String,
    use_manual_url: Option<bool>,
    selected_provider_id: Option<String>,
    service_mode: Option<String>,
    state: tauri::State<'_, RoutstrState>,
) -> Result<(), String> {
    let mut service = state.lock().await;

    // Update UI state if provided
    if let Some(manual_url) = use_manual_url {
        service.use_manual_url = manual_url;
    }
    if let Some(provider_id) = selected_provider_id {
        service.selected_provider_id = Some(provider_id);
    }
    if let Some(mode) = service_mode {
        service.service_mode = mode;
    }

    // Connect to the service
    service
        .connect_to_service(url)
        .await
        .map_err(|e| e.to_string())?;

    // Start automatic model updates
    service.start_auto_update(state.inner().clone());

    Ok(())
}

#[tauri::command]
pub async fn routstr_disconnect_service(
    state: tauri::State<'_, RoutstrState>,
) -> Result<(), String> {
    let mut service = state.lock().await;
    service.disconnect();
    Ok(())
}

#[tauri::command]
pub async fn routstr_refresh_models(state: tauri::State<'_, RoutstrState>) -> Result<(), String> {
    let mut service = state.lock().await;
    service.refresh_models().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn routstr_get_models(
    state: tauri::State<'_, RoutstrState>,
) -> Result<Vec<RoutstrModel>, String> {
    let service = state.lock().await;
    Ok(service.get_models().clone())
}

#[tauri::command]
pub async fn routstr_get_connection_status(
    state: tauri::State<'_, RoutstrState>,
) -> Result<serde_json::Value, String> {
    let service = state.lock().await;
    Ok(serde_json::json!({
        "connected": service.is_connected(),
        "base_url": service.get_base_url(),
        "model_count": service.get_models().len(),
        "has_api_key": !service.api_keys.is_empty()
    }))
}

#[tauri::command]
pub async fn routstr_create_wallet(
    url: String,
    cashu_token: String,
    state: tauri::State<'_, RoutstrState>,
) -> Result<RoutstrCreateResponse, String> {
    let mut service = state.lock().await;
    let result = service
        .create_wallet(url, cashu_token)
        .await
        .map_err(|e| e.to_string())?;

    // Start automatic model updates since we're now connected
    service.start_auto_update(state.inner().clone());

    Ok(result)
}

#[tauri::command]
pub async fn routstr_create_balance_with_token(
    cashu_token: String,
    state: tauri::State<'_, RoutstrState>,
) -> Result<RoutstrCreateResponse, String> {
    let mut service = state.lock().await;
    service
        .create_balance_with_token(cashu_token)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn routstr_get_all_api_keys(
    state: tauri::State<'_, RoutstrState>,
) -> Result<Vec<ApiKeyEntry>, String> {
    let service = state.lock().await;
    Ok(service.get_api_keys().clone())
}

#[tauri::command]
pub async fn routstr_get_all_wallet_balances(
    state: tauri::State<'_, RoutstrState>,
) -> Result<Vec<RoutstrWalletBalance>, String> {
    let service = state.lock().await;
    Ok(service.get_all_wallet_balances().await)
}

#[tauri::command]
pub async fn routstr_get_wallet_balance_for_key(
    api_key: String,
    state: tauri::State<'_, RoutstrState>,
) -> Result<RoutstrWalletBalance, String> {
    let service = state.lock().await;
    service
        .get_wallet_balance_for_key(&api_key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn routstr_top_up_wallet_for_key(
    api_key: String,
    cashu_token: String,
    state: tauri::State<'_, RoutstrState>,
) -> Result<RoutstrTopUpResponse, String> {
    let service = state.lock().await;
    service
        .top_up_wallet_for_key(&api_key, cashu_token)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn routstr_refund_wallet_for_key(
    api_key: String,
    routstr_state: tauri::State<'_, RoutstrState>,
    tollgate_state: tauri::State<'_, crate::TollGateState>,
) -> Result<RoutstrRefundResponse, String> {
    let refund_response = {
        let service = routstr_state.lock().await;
        service
            .refund_wallet_for_key(&api_key)
            .await
            .map_err(|e| e.to_string())?
    };

    if let Some(ref token) = refund_response.token {
        let tollgate_service = tollgate_state.lock().await;
        match tollgate_service.receive_cashu_token(token).await {
            Ok(result) => {
                log::info!(
                    "Successfully received refunded token into local wallet: {} sats from {}",
                    result.amount,
                    result.mint_url
                );
            }
            Err(e) => {
                log::warn!("Failed to receive refunded token into local wallet: {}", e);
            }
        }
    }

    // Remove the API key after successful refund
    {
        let mut service = routstr_state.lock().await;
        service.remove_api_key(&api_key);
    }

    Ok(refund_response)
}

#[tauri::command]
pub async fn routstr_remove_api_key(
    api_key: String,
    state: tauri::State<'_, RoutstrState>,
) -> Result<bool, String> {
    let mut service = state.lock().await;
    Ok(service.remove_api_key(&api_key))
}

#[tauri::command]
pub async fn routstr_clear_config(state: tauri::State<'_, RoutstrState>) -> Result<(), String> {
    let mut service = state.lock().await;
    service.clear_config().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn routstr_force_reset_all_api_keys(
    state: tauri::State<'_, RoutstrState>,
) -> Result<(), String> {
    let mut service = state.lock().await;
    service
        .force_reset_all_api_keys()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn routstr_get_proxy_status(
    state: tauri::State<'_, RoutstrState>,
) -> Result<serde_json::Value, String> {
    let service = state.lock().await;

    Ok(serde_json::json!({
        "use_proxy": service.use_proxy,
        "proxy_endpoint": service.proxy_endpoint,
        "target_service_url": service.target_service_url,
        "use_onion": service.use_onion,
        "payment_required": service.payment_required,
        "cost_per_request_sats": service.cost_per_request_sats,
        "use_manual_url": service.use_manual_url,
        "selected_provider_id": service.selected_provider_id,
        "service_mode": service.service_mode
    }))
}

#[tauri::command]
pub async fn routstr_get_ui_state(
    state: tauri::State<'_, RoutstrState>,
) -> Result<serde_json::Value, String> {
    let service = state.lock().await;

    Ok(serde_json::json!({
        "use_manual_url": service.use_manual_url,
        "selected_provider_id": service.selected_provider_id,
        "service_mode": service.service_mode,
        "selected_mint_url": service.selected_mint_url
    }))
}

#[tauri::command]
pub async fn routstr_set_selected_mint(
    mint_url: Option<String>,
    state: tauri::State<'_, RoutstrState>,
) -> Result<(), String> {
    let mut service = state.lock().await;
    service.set_selected_mint(mint_url);
    Ok(())
}

#[tauri::command]
pub async fn routstr_get_selected_mint(
    state: tauri::State<'_, RoutstrState>,
) -> Result<Option<String>, String> {
    let service = state.lock().await;
    Ok(service.get_selected_mint().cloned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn test_config_default_when_file_missing() {
        // Create service with non-existent config file
        let temp_dir = env::temp_dir().join("routstr_test_missing");
        let config_file = temp_dir.join("nonexistent.json");

        // Clean up any existing test files
        let _ = fs::remove_dir_all(&temp_dir);

        let mut service = RoutstrService {
            base_url: None,
            models: Vec::new(),
            api_keys: Vec::new(),
            use_proxy: false,
            proxy_endpoint: None,
            target_service_url: None,
            use_onion: false,
            payment_required: false,
            cost_per_request_sats: 10,
            use_manual_url: true,
            selected_provider_id: None,
            service_mode: "wallet".to_string(),
            selected_mint_url: None,
            client: reqwest::Client::new(),
            storage: RoutstrStoragePaths {
                config_file: config_file.clone(),
            },
            auto_update_handle: None,
        };

        // Load configuration (should succeed with defaults)
        service.load_config().expect("Failed to load config");

        // Verify default values are used
        assert_eq!(service.base_url, None);
        assert_eq!(service.api_keys.len(), 0);

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_api_key_management() {
        let mut service = RoutstrService::new();

        // Test adding API keys
        service.add_api_key(
            "test_key_1".to_string(),
            Some("token1".to_string()),
            Some("First Key".to_string()),
        );
        service.add_api_key("test_key_2".to_string(), None, None);

        // Verify keys are stored
        assert_eq!(service.api_keys.len(), 2);
        assert_eq!(service.api_keys[0].api_key, "test_key_1");
        assert_eq!(service.api_keys[1].api_key, "test_key_2");

        // Test removing API key
        assert!(service.remove_api_key("test_key_1"));
        assert_eq!(service.api_keys.len(), 1);
        assert_eq!(service.api_keys[0].api_key, "test_key_2");

        // Test removing non-existent key
        assert!(!service.remove_api_key("non_existent"));
        assert_eq!(service.api_keys.len(), 1);
    }
}
