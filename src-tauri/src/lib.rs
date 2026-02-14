use std::sync::Arc;
use tauri::{Manager, State};
use tokio::sync::Mutex;

mod wallet_hub;
use wallet_hub::WalletService;

// Global state for the Wallet service
type WalletState = Arc<Mutex<WalletService>>;

// Global state for the NWC service
type NwcState = Arc<Mutex<Option<NostrWalletConnect>>>;

mod connection_server;
mod nostr_providers;
mod nwc;
mod nwc_storage;
mod proxy;
mod relay;
mod routstr;
mod wallet;

use nwc::{BudgetRenewalPeriod, NostrWalletConnect};
use wallet::*;

#[tauri::command]
async fn nwc_list_connections(
    nwc_state: State<'_, NwcState>,
) -> Result<Vec<serde_json::Value>, String> {
    let nwc_lock = nwc_state.lock().await;
    let nwc = nwc_lock.as_ref().ok_or("NWC service not initialized")?;

    let connections = nwc.get_connections().await;
    let connection_data: Vec<serde_json::Value> = connections
        .iter()
        .map(|conn| {
            serde_json::json!({
                "pubkey": conn.keys.public_key().to_string(),
                "pubkey_hex": conn.keys.public_key().to_hex(),
                "budget_msats": conn.budget.total_budget_msats,
                "used_budget_msats": conn.budget.used_budget_msats,
                "renewal_period": match conn.budget.renewal_period {
                    BudgetRenewalPeriod::Daily => "daily",
                    BudgetRenewalPeriod::Weekly => "weekly",
                    BudgetRenewalPeriod::Monthly => "monthly",
                    BudgetRenewalPeriod::Yearly => "yearly",
                    BudgetRenewalPeriod::Never => "never",
                },
                "name": conn.name.clone(),
            })
        })
        .collect();

    Ok(connection_data)
}

fn parse_budget_period(value: &str) -> Result<BudgetRenewalPeriod, String> {
    match value {
        "daily" => Ok(BudgetRenewalPeriod::Daily),
        "weekly" => Ok(BudgetRenewalPeriod::Weekly),
        "monthly" => Ok(BudgetRenewalPeriod::Monthly),
        "yearly" => Ok(BudgetRenewalPeriod::Yearly),
        "never" => Ok(BudgetRenewalPeriod::Never),
        _ => Err(format!("Invalid renewal period: {}", value)),
    }
}

#[tauri::command]
async fn nwc_update_connection_budget(
    pubkey: String,
    budget_sats: u64,
    renewal_period: String,
    nwc_state: State<'_, NwcState>,
) -> Result<serde_json::Value, String> {
    let period = parse_budget_period(&renewal_period)?;

    let nwc_lock = nwc_state.lock().await;
    let nwc = nwc_lock
        .as_ref()
        .ok_or_else(|| "NWC service not initialized".to_string())?;

    let updated = nwc
        .update_connection_budget(&pubkey, budget_sats, period)
        .await
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "budget_msats": updated.budget.total_budget_msats,
        "used_budget_msats": updated.budget.used_budget_msats,
        "renewal_period": renewal_period,
    }))
}

#[tauri::command]
async fn nwc_update_connection_name(
    pubkey: String,
    name: String,
    nwc_state: State<'_, NwcState>,
) -> Result<String, String> {
    let nwc_lock = nwc_state.lock().await;
    let nwc = nwc_lock
        .as_ref()
        .ok_or_else(|| "NWC service not initialized".to_string())?;

    let updated = nwc
        .update_connection_name(&pubkey, &name)
        .await
        .map_err(|e| e.to_string())?;

    Ok(updated.name.clone())
}

#[tauri::command]
async fn nwc_get_service_pubkey(nwc_state: State<'_, NwcState>) -> Result<String, String> {
    let nwc_lock = nwc_state.lock().await;
    let nwc = nwc_lock.as_ref().ok_or("NWC service not initialized")?;
    Ok(nwc.service_pubkey().to_string())
}

#[tauri::command]
async fn nwc_remove_connection(
    pubkey: String,
    nwc_state: State<'_, NwcState>,
) -> Result<(), String> {
    println!("Rust: removing NWC connection {pubkey}");
    let nwc_lock = nwc_state.lock().await;
    let nwc = nwc_lock.as_ref().ok_or("NWC service not initialized")?;

    nwc.remove_connection(&pubkey)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn nwc_create_standard_connection(
    use_local_relay: Option<bool>,
    nwc_state: State<'_, NwcState>,
) -> Result<String, String> {
    let nwc_lock = nwc_state.lock().await;
    let nwc = nwc_lock.as_ref().ok_or("NWC service not initialized")?;

    nwc.create_standard_nwc_uri(use_local_relay.unwrap_or(false))
        .await
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    builder = builder.setup(|app| {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .expect("Failed to get app data dir");

        // Initialize Wallet service and runtime
        let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());

        // Start local Nostr relay before NWC service
        {
            let rt_clone = rt.clone();
            rt_clone.block_on(async {
                log::info!("=== Starting local Nostr relay ===");
                if let Err(e) = relay::start_relay_server(relay::DEFAULT_RELAY_PORT).await {
                    log::error!("Failed to start local Nostr relay: {}", e);
                } else {
                    log::info!(
                        "Local Nostr relay started on port {}",
                        relay::DEFAULT_RELAY_PORT
                    );
                    // Give the relay a moment to fully initialize
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    log::info!("=== Relay ready ===");
                }
            });
        }

        let service_arc = {
            let rt_clone = rt.clone();
            let app_data_dir_clone = app_data_dir.clone();
            rt_clone.block_on(async {
                let service = WalletService::new(app_data_dir_clone)
                    .await
                    .expect("Failed to create Wallet service");

                Arc::new(Mutex::new(service))
            })
        };

        // Initialize NWC service
        let nwc_arc = {
            let rt_clone = rt.clone();
            let service_clone = service_arc.clone();
            let app_data_dir_clone = app_data_dir.clone();
            rt_clone.block_on(async {
                let service = service_clone.lock().await;
                let wallet_keys = service.get_wallet_keys().await;

                let nwc_secret = wallet_keys.secret_key().clone();
                drop(service); // Release lock before creating NWC

                match NostrWalletConnect::new(nwc_secret, service_clone.clone(), app_data_dir_clone)
                    .await
                {
                    Ok(nwc) => {
                        log::info!("NWC service initialized");
                        Arc::new(Mutex::new(Some(nwc)))
                    }
                    Err(e) => {
                        log::error!("Failed to initialize NWC service: {}", e);
                        Arc::new(Mutex::new(None))
                    }
                }
            })
        };

        // Start NWC event processing loop
        let nwc_clone = nwc_arc.clone();
        let rt_clone = rt.clone();
        rt_clone.spawn(async move {
            log::info!("=== Starting NWC event processing task ===");

            let nwc_service = {
                let nwc_lock = nwc_clone.lock().await;
                nwc_lock.as_ref().cloned()
            }; // Lock is released here

            if let Some(nwc) = nwc_service {
                log::info!("Starting NWC service and connecting to relay...");
                if let Err(e) = nwc.start().await {
                    log::error!("Failed to start NWC service: {}", e);
                    return;
                }
                log::info!("✓ NWC service started and connected");

                log::info!("Starting NWC event processing loop...");
                if let Err(e) = nwc.process_events_loop().await {
                    log::error!("NWC event processing loop ended with error: {}", e);
                } else {
                    log::warn!("NWC event processing loop ended (should run indefinitely)");
                }
            } else {
                log::warn!("NWC service not initialized, skipping event processing");
            }
        });

        // Start connection server
        let connection_app_handle = app.handle().clone();
        let pending_connections =
            Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new()));
        let pending_connections_for_server = pending_connections.clone();
        let rt_clone = rt.clone();
        rt_clone.spawn(async move {
            if let Err(e) = connection_server::start_connection_server(
                connection_app_handle,
                pending_connections_for_server,
                connection_server::DEFAULT_CONNECTION_PORT,
            )
            .await
            {
                log::error!("Failed to start connection server: {}", e);
            } else {
                log::info!(
                    "Connection server started successfully on port {}",
                    connection_server::DEFAULT_CONNECTION_PORT
                );
            }
        });

        // Initialize Routstr service
        let routstr_arc = Arc::new(Mutex::new(routstr::RoutstrService::new(app_data_dir)));


        let routstr_clone = routstr_arc.clone();
        let rt_clone = rt.clone();
        rt_clone.spawn(async move {
            routstr::initialize_routstr_auto_update(routstr_clone).await;
        });

        app.manage(service_arc);
        app.manage(nwc_arc);
        app.manage(routstr_arc);
        app.manage(rt.clone());
        app.manage(pending_connections);

        rt.spawn(start_provider_monitoring());

        log::info!("Wallet service initialized");
        Ok(())
    });

    #[tauri::command]
    async fn discover_nostr_providers() -> Result<Vec<nostr_providers::NostrProvider>, String> {
        nostr_providers::discover_providers()
            .await
            .map_err(|e| e.to_string())
    }

    async fn start_provider_monitoring() {
        tokio::spawn(async {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
            loop {
                interval.tick().await;
                match nostr_providers::discover_providers().await {
                    Ok(providers) => {
                        log::info!("Updated provider list: {} providers found", providers.len());
                    }
                    Err(e) => {
                        log::warn!("Failed to update providers: {}", e);
                    }
                }
            }
        });
    }

    builder = builder
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_notification::init());

    #[cfg(mobile)]
    {
        builder = builder.plugin(tauri_plugin_barcode_scanner::init());
    }

    builder = builder.invoke_handler(tauri::generate_handler![
            add_mint,
            set_default_mint,
            remove_mint,
            get_wallet_balance,
            create_nut18_payment_request,
            create_bolt11_invoice,
            pay_nut18_payment_request,
            pay_bolt11_invoice,
            get_wallet_summary,
            list_wallet_transactions,
            receive_cashu_token,
            create_external_token,
            nwc_list_connections,
            nwc_remove_connection,
            nwc_get_service_pubkey,
            nwc_update_connection_budget,
            nwc_update_connection_name,
            nwc_create_standard_connection,
            connection_server::nwc_approve_connection,
            connection_server::nwc_reject_connection,
            routstr::routstr_connect_service,
            routstr::routstr_disconnect_service,
            routstr::routstr_refresh_models,
            routstr::routstr_get_models,
            routstr::routstr_get_connection_status,
            routstr::routstr_create_wallet,
            routstr::routstr_create_balance_with_token,
            routstr::routstr_get_all_api_keys,
            routstr::routstr_get_all_wallet_balances,
            routstr::routstr_get_wallet_balance_for_key,
            routstr::routstr_top_up_wallet_for_key,
            routstr::routstr_refund_wallet_for_key,
            routstr::routstr_remove_api_key,
            routstr::routstr_clear_config,
            routstr::routstr_force_reset_all_api_keys,
            routstr::routstr_get_proxy_status,
            routstr::routstr_get_ui_state,
            routstr::routstr_set_selected_mint,
            routstr::routstr_get_selected_mint,
            discover_nostr_providers,
        ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
