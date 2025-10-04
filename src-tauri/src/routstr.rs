use anyhow::{anyhow, Result};
use directories::ProjectDirs;
use reqwest;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration};

#[derive(Debug, Clone)]
pub struct RoutstrStoragePaths {
    pub base_dir: PathBuf,
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

        Ok(Self {
            base_dir,
            config_file,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RoutstrStoredConfig {
    pub auto_topup_enabled: bool,
    pub min_balance_threshold: u64,
    pub topup_amount_target: u64,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub creation_cashu_token: Option<String>,
}

impl Default for RoutstrStoredConfig {
    fn default() -> Self {
        Self {
            auto_topup_enabled: false,
            min_balance_threshold: 10000,
            topup_amount_target: 100000,
            base_url: None,
            api_key: None,
            creation_cashu_token: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Architecture {
    pub modality: String,
    pub input_modalities: Vec<String>,
    pub output_modalities: Vec<String>,
    pub tokenizer: String,
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
    pub max_prompt_cost: f64,
    pub max_completion_cost: f64,
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
    pub description: String,
    pub context_length: u32,
    pub architecture: Architecture,
    pub pricing: Pricing,
    pub sats_pricing: SatsPricing,
    pub per_request_limits: Option<serde_json::Value>,
    pub top_provider: TopProvider,
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
    pub api_key: Option<String>,
    pub creation_cashu_token: Option<String>,
    client: reqwest::Client,
    pub auto_topup_enabled: bool,
    pub min_balance_threshold: u64,
    pub topup_amount_target: u64,
    balance_monitor_task: Option<JoinHandle<()>>,
    storage: RoutstrStoragePaths,
}

impl Clone for RoutstrService {
    fn clone(&self) -> Self {
        Self {
            base_url: self.base_url.clone(),
            models: self.models.clone(),
            api_key: self.api_key.clone(),
            creation_cashu_token: self.creation_cashu_token.clone(),
            client: self.client.clone(),
            auto_topup_enabled: self.auto_topup_enabled,
            min_balance_threshold: self.min_balance_threshold,
            topup_amount_target: self.topup_amount_target,
            balance_monitor_task: None, // Don't clone the task handle
            storage: self.storage.clone(),
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
                base_dir: std::env::temp_dir().join("routstr_fallback"),
                config_file: std::env::temp_dir()
                    .join("routstr_fallback")
                    .join("config.json"),
            }
        });

        let mut service = Self {
            base_url: None,
            models: Vec::new(),
            api_key: None,
            creation_cashu_token: None,
            client: reqwest::Client::new(),
            auto_topup_enabled: false,
            min_balance_threshold: 10000,
            topup_amount_target: 100000,
            balance_monitor_task: None,
            storage,
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
        // Stop balance monitoring task if it exists
        if let Some(task) = self.balance_monitor_task.take() {
            task.abort();
            log::info!("Stopped balance monitoring task");
        }

        self.base_url = None;
        self.models.clear();
        self.api_key = None;

        // Persist updated configuration
        if let Err(e) = self.save_config() {
            log::error!("Failed to save disconnect configuration: {}", e);
        }

        log::info!("Disconnected from Routstr service");
    }

    pub fn clear_config(&mut self) -> Result<()> {
        self.models.clear();
        self.api_key = None;
        self.creation_cashu_token = None;
        self.auto_topup_enabled = false;
        self.min_balance_threshold = 10000;
        self.topup_amount_target = 100000;

        if let Some(task) = self.balance_monitor_task.take() {
            task.abort();
        }

        Ok(())
    }

    pub fn set_api_key(&mut self, api_key: String) {
        self.api_key = Some(api_key);

        // Persist updated configuration
        if let Err(e) = self.save_config() {
            log::error!("Failed to save API key configuration: {}", e);
        }

        log::info!("API key set for Routstr service");
    }

    pub fn get_api_key(&self) -> Option<&String> {
        self.api_key.as_ref()
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
        self.api_key = Some(create_response.api_key.clone());
        self.creation_cashu_token = Some(cashu_token);

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
        self.api_key = Some(create_response.api_key.clone());
        self.creation_cashu_token = Some(cashu_token);

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

    pub async fn get_wallet_balance(&self) -> Result<RoutstrWalletBalance> {
        let base_url = self
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to any Routstr service"))?;

        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow!("No API key set for Routstr service"))?;

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

        let balance: RoutstrWalletBalance = response.json().await?;

        log::info!(
            "Retrieved wallet balance: {} (reserved: {})",
            balance.balance,
            balance.reserved
        );

        Ok(balance)
    }

    pub async fn top_up_wallet(&self, cashu_token: String) -> Result<RoutstrTopUpResponse> {
        let base_url = self
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to any Routstr service"))?;

        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow!("No API key set for Routstr service"))?;

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

    pub async fn refund_wallet(&self) -> Result<RoutstrRefundResponse> {
        let base_url = self
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("Not connected to any Routstr service"))?;

        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| anyhow!("No API key set for Routstr service"))?;

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

            self.auto_topup_enabled = stored.auto_topup_enabled;
            self.min_balance_threshold = stored.min_balance_threshold;
            self.topup_amount_target = stored.topup_amount_target;
            self.base_url = stored.base_url;
            self.api_key = stored.api_key;
            self.creation_cashu_token = stored.creation_cashu_token;

            log::info!("Loaded Routstr configuration from storage");
        }
        Ok(())
    }

    fn save_config(&self) -> Result<()> {
        let config = RoutstrStoredConfig {
            auto_topup_enabled: self.auto_topup_enabled,
            min_balance_threshold: self.min_balance_threshold,
            topup_amount_target: self.topup_amount_target,
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            creation_cashu_token: self.creation_cashu_token.clone(),
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

    pub fn set_auto_topup_config(&mut self, enabled: bool, min_threshold: u64, target_amount: u64) {
        self.auto_topup_enabled = enabled;
        self.min_balance_threshold = min_threshold;
        self.topup_amount_target = target_amount;

        // Persist configuration
        if let Err(e) = self.save_config() {
            log::error!("Failed to save auto-topup configuration: {}", e);
        }

        log::info!(
            "Auto-topup config updated: enabled={}, threshold={}, target={}",
            enabled,
            min_threshold,
            target_amount
        );
    }

    pub fn start_balance_monitoring(&mut self, service_state: RoutstrState) {
        // Stop existing task if running
        if let Some(task) = self.balance_monitor_task.take() {
            task.abort();
        }

        if !self.auto_topup_enabled {
            log::info!("Auto-topup disabled, not starting balance monitoring");
            return;
        }

        let min_threshold = self.min_balance_threshold;
        let target_amount = self.topup_amount_target;

        log::info!(
            "Starting balance monitoring: threshold={} msats, target={} msats",
            min_threshold,
            target_amount
        );

        let task = tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30)); // Check every 30 seconds

            loop {
                interval.tick().await;

                // Check if auto-topup is still enabled
                let (is_enabled, current_threshold) = {
                    let service = service_state.lock().await;
                    (service.auto_topup_enabled, service.min_balance_threshold)
                };

                if !is_enabled {
                    log::debug!("Auto-topup disabled, stopping balance monitoring");
                    break;
                }

                // Get current balance
                match Self::check_and_topup_if_needed(
                    &service_state,
                    current_threshold,
                    target_amount,
                )
                .await
                {
                    Ok(topped_up) => {
                        if topped_up {
                            log::info!("Successfully performed auto-topup");
                        }
                    }
                    Err(e) => {
                        log::error!("Error during balance check/topup: {}", e);
                    }
                }
            }

            log::info!("Balance monitoring task ended");
        });

        self.balance_monitor_task = Some(task);
    }

    async fn check_and_topup_if_needed(
        service_state: &RoutstrState,
        min_threshold: u64,
        target_amount: u64,
    ) -> Result<bool> {
        let service = service_state.lock().await;

        // Only proceed if we have API key and are connected
        if service.api_key.is_none() || service.base_url.is_none() {
            return Ok(false);
        }

        // Check current balance
        match service.get_wallet_balance().await {
            Ok(balance) => {
                log::debug!(
                    "Current balance: {} msats, threshold: {} msats",
                    balance.balance,
                    min_threshold
                );

                if balance.balance < min_threshold {
                    log::info!(
                        "Balance {} is below threshold {}, auto-topup needed",
                        balance.balance,
                        min_threshold
                    );

                    // Calculate how much we need to reach target
                    let needed_amount = target_amount.saturating_sub(balance.balance);
                    log::info!(
                        "Need {} msats to reach target of {} msats",
                        needed_amount,
                        target_amount
                    );

                    log::warn!("Auto-topup needed but requires user to provide Cashu token");

                    return Ok(false);
                }

                Ok(false)
            }
            Err(e) => {
                log::debug!("Failed to check balance: {}", e);
                Err(e)
            }
        }
    }
}

// Global state for the Routstr service
use std::sync::Arc;
use tokio::sync::Mutex;

pub type RoutstrState = Arc<Mutex<RoutstrService>>;

#[tauri::command]
pub async fn routstr_connect_service(
    url: String,
    state: tauri::State<'_, RoutstrState>,
) -> Result<(), String> {
    let mut service = state.lock().await;
    service
        .connect_to_service(url)
        .await
        .map_err(|e| e.to_string())
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
        "has_api_key": service.get_api_key().is_some()
    }))
}

#[tauri::command]
pub async fn routstr_set_api_key(
    api_key: String,
    state: tauri::State<'_, RoutstrState>,
) -> Result<(), String> {
    let mut service = state.lock().await;
    service.set_api_key(api_key);
    Ok(())
}

#[tauri::command]
pub async fn routstr_create_wallet(
    url: String,
    cashu_token: String,
    state: tauri::State<'_, RoutstrState>,
) -> Result<RoutstrCreateResponse, String> {
    let mut service = state.lock().await;
    service
        .create_wallet(url, cashu_token)
        .await
        .map_err(|e| e.to_string())
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
pub async fn routstr_get_wallet_balance(
    state: tauri::State<'_, RoutstrState>,
) -> Result<RoutstrWalletBalance, String> {
    let service = state.lock().await;
    service
        .get_wallet_balance()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn routstr_top_up_wallet(
    cashu_token: String,
    state: tauri::State<'_, RoutstrState>,
) -> Result<RoutstrTopUpResponse, String> {
    let service = state.lock().await;
    service
        .top_up_wallet(cashu_token)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn routstr_refund_wallet(
    routstr_state: tauri::State<'_, RoutstrState>,
    tollgate_state: tauri::State<'_, crate::TollGateState>,
) -> Result<RoutstrRefundResponse, String> {
    let refund_response = {
        let service = routstr_state.lock().await;
        service.refund_wallet().await.map_err(|e| e.to_string())?
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

    Ok(refund_response)
}

#[tauri::command]
pub async fn routstr_set_auto_topup_config(
    enabled: bool,
    min_threshold: u64,
    target_amount: u64,
    state: tauri::State<'_, RoutstrState>,
) -> Result<(), String> {
    let mut service = state.lock().await;
    service.set_auto_topup_config(enabled, min_threshold, target_amount);

    // Start or stop balance monitoring based on enabled state
    if enabled {
        service.start_balance_monitoring(state.inner().clone());
    }

    Ok(())
}

#[tauri::command]
pub async fn routstr_get_auto_topup_config(
    state: tauri::State<'_, RoutstrState>,
) -> Result<serde_json::Value, String> {
    let service = state.lock().await;
    Ok(serde_json::json!({
        "enabled": service.auto_topup_enabled,
        "min_threshold": service.min_balance_threshold,
        "target_amount": service.topup_amount_target
    }))
}

#[tauri::command]
pub async fn routstr_get_stored_api_key(
    state: tauri::State<'_, RoutstrState>,
) -> Result<Option<String>, String> {
    let service = state.lock().await;
    Ok(service.api_key.clone())
}

#[tauri::command]
pub async fn routstr_clear_config(state: tauri::State<'_, RoutstrState>) -> Result<(), String> {
    let mut service = state.lock().await;
    service.clear_config().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    #[test]
    fn test_config_persistence() {
        // Create a temporary directory for testing
        let temp_dir = env::temp_dir().join("routstr_test");
        let config_file = temp_dir.join("config.json");

        // Clean up any existing test files
        let _ = fs::remove_dir_all(&temp_dir);

        // Create service with custom storage
        let storage = RoutstrStoragePaths {
            base_dir: temp_dir.clone(),
            config_file: config_file.clone(),
        };

        let mut service = RoutstrService {
            base_url: None,
            models: Vec::new(),
            api_key: None,
            client: reqwest::Client::new(),
            auto_topup_enabled: false,
            min_balance_threshold: 10000,
            topup_amount_target: 100000,
            balance_monitor_task: None,
            storage,
        };

        // Set some configuration
        service.set_auto_topup_config(true, 25000, 150000);
        service.base_url = Some("https://test.routstr.com".to_string());
        service.api_key = Some("test_api_key".to_string());

        // Save configuration
        service.save_config().expect("Failed to save config");

        // Verify file was created
        assert!(config_file.exists(), "Config file should exist");

        // Create a new service instance to test loading
        let mut new_service = RoutstrService {
            base_url: None,
            models: Vec::new(),
            api_key: None,
            client: reqwest::Client::new(),
            auto_topup_enabled: false,
            min_balance_threshold: 10000,
            topup_amount_target: 100000,
            balance_monitor_task: None,
            storage: RoutstrStoragePaths {
                base_dir: temp_dir.clone(),
                config_file: config_file.clone(),
            },
        };

        // Load configuration
        new_service.load_config().expect("Failed to load config");

        // Verify configuration was loaded correctly
        assert_eq!(new_service.auto_topup_enabled, true);
        assert_eq!(new_service.min_balance_threshold, 25000);
        assert_eq!(new_service.topup_amount_target, 150000);
        assert_eq!(
            new_service.base_url,
            Some("https://test.routstr.com".to_string())
        );
        assert_eq!(new_service.api_key, Some("test_api_key".to_string()));

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

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
            api_key: None,
            client: reqwest::Client::new(),
            auto_topup_enabled: false,
            min_balance_threshold: 10000,
            topup_amount_target: 100000,
            balance_monitor_task: None,
            storage: RoutstrStoragePaths {
                base_dir: temp_dir.clone(),
                config_file: config_file.clone(),
            },
        };

        // Load configuration (should succeed with defaults)
        service.load_config().expect("Failed to load config");

        // Verify default values are used
        assert_eq!(service.auto_topup_enabled, false);
        assert_eq!(service.min_balance_threshold, 10000);
        assert_eq!(service.topup_amount_target, 100000);
        assert_eq!(service.base_url, None);
        assert_eq!(service.api_key, None);

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_api_key_storage_and_retrieval() {
        // Create a temporary directory for testing
        let temp_dir = env::temp_dir().join("routstr_test_api_key");
        let config_file = temp_dir.join("config.json");

        // Clean up any existing test files
        let _ = fs::remove_dir_all(&temp_dir);

        // Create service with custom storage
        let storage = RoutstrStoragePaths {
            base_dir: temp_dir.clone(),
            config_file: config_file.clone(),
        };

        let mut service = RoutstrService {
            base_url: None,
            models: Vec::new(),
            api_key: None,
            client: reqwest::Client::new(),
            auto_topup_enabled: false,
            min_balance_threshold: 10000,
            topup_amount_target: 100000,
            balance_monitor_task: None,
            storage,
        };

        // Set API key
        service.set_api_key("test_api_key_12345".to_string());

        // Verify it's set in memory
        assert_eq!(
            service.get_api_key(),
            Some(&"test_api_key_12345".to_string())
        );

        // Create a new service instance to test loading
        let mut new_service = RoutstrService {
            base_url: None,
            models: Vec::new(),
            api_key: None,
            client: reqwest::Client::new(),
            auto_topup_enabled: false,
            min_balance_threshold: 10000,
            topup_amount_target: 100000,
            balance_monitor_task: None,
            storage: RoutstrStoragePaths {
                base_dir: temp_dir.clone(),
                config_file: config_file.clone(),
            },
        };

        // Load configuration
        new_service.load_config().expect("Failed to load config");

        // Verify API key was loaded correctly
        assert_eq!(
            new_service.get_api_key(),
            Some(&"test_api_key_12345".to_string())
        );

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
