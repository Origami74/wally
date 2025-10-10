#[cfg(target_os = "macos")]
use cocoa::{
    appkit::{NSMainMenuWindowLevel, NSWindowCollectionBehavior},
    base::{id, NO},
    foundation::NSRect,
};
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};
use std::sync::Arc;
#[cfg(target_os = "macos")]
use tauri::{
    image::Image,
    tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent},
    ActivationPolicy, LogicalPosition, PhysicalPosition,
};
use tauri::{Manager, State};
#[cfg(target_os = "macos")]
use tauri_nspanel::{ManagerExt, WebviewWindowExt};
use tauri_plugin_androidwifi::{AndroidwifiExt, Empty, GetMacAddressPayload};
use tauri_plugin_log::{Target, TargetKind};
use tokio::sync::Mutex;

mod tollgate;
use tollgate::session::SessionStatus;
use tollgate::TollGateService;

// Global state for the TollGate service
type TollGateState = Arc<Mutex<TollGateService>>;

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
async fn toggle_auto_tollgate(
    enabled: bool,
    state: State<'_, TollGateState>,
) -> Result<(), String> {
    let service = state.lock().await;
    service
        .set_auto_tollgate_enabled(enabled)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_tollgate_status(
    state: State<'_, TollGateState>,
) -> Result<tollgate::service::ServiceStatus, String> {
    let service = state.lock().await;
    service.get_status().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_current_session(
    state: State<'_, TollGateState>,
) -> Result<Option<tollgate::service::SessionInfo>, String> {
    let service = state.lock().await;
    service
        .get_current_session()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn force_session_renewal(
    tollgate_pubkey: String,
    state: State<'_, TollGateState>,
) -> Result<(), String> {
    let service = state.lock().await;
    service
        .force_renewal(&tollgate_pubkey)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn handle_network_connected(
    gateway_ip: String,
    mac_address: String,
    state: State<'_, TollGateState>,
) -> Result<(), String> {
    let service = state.lock().await;
    service
        .handle_network_connected(gateway_ip, mac_address)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn handle_network_disconnected(state: State<'_, TollGateState>) -> Result<(), String> {
    let service = state.lock().await;
    service
        .handle_network_disconnected()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn detect_tollgate(
    gateway_ip: String,
    mac_address: String,
    state: State<'_, TollGateState>,
) -> Result<serde_json::Value, String> {
    let service = state.lock().await;

    match service.detect_tollgate(&gateway_ip, &mac_address).await {
        Ok(network_info) => {
            let result = serde_json::json!({
                "is_tollgate": network_info.advertisement.is_some(),
                "advertisement": network_info.advertisement
            });
            Ok(result)
        }
        Err(e) => Err(format!("Failed to detect TollGate: {}", e)),
    }
}

#[tauri::command]
async fn get_mac_address(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let payload = GetMacAddressPayload {
        gateway_ip: "192.168.1.1".to_string(),
    };

    match app.androidwifi().get_mac_address(payload) {
        Ok(result) => Ok(serde_json::to_value(result).unwrap()),
        Err(e) => Err(format!("Failed to get MAC address: {}", e)),
    }
}

#[tauri::command]
async fn get_current_wifi_details(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let payload = Empty { value: None };

    match app.androidwifi().get_current_wifi_details(payload) {
        Ok(result) => Ok(serde_json::to_value(result).unwrap()),
        Err(e) => Err(format!("Failed to get current WiFi details: {}", e)),
    }
}

#[tauri::command]
async fn get_gateway_ip(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let payload = Empty { value: None };

    match app.androidwifi().get_gateway_ip(payload) {
        Ok(result) => Ok(serde_json::to_value(result).unwrap()),
        Err(e) => Err(format!("Failed to get gateway IP: {}", e)),
    }
}

#[tauri::command]
async fn get_active_sessions(
    state: State<'_, TollGateState>,
) -> Result<Vec<serde_json::Value>, String> {
    let service = state.lock().await;

    let sessions = service
        .get_active_sessions()
        .await
        .map_err(|e| e.to_string())?;
    let session_data: Vec<serde_json::Value> = sessions
        .iter()
        .map(|session| {
            serde_json::json!({
                "id": session.id,
                "tollgate_pubkey": session.tollgate_pubkey,
                "gateway_ip": session.gateway_ip,
                "status": match session.status {
                    SessionStatus::Initializing => "initializing",
                    SessionStatus::Active => "active",
                    SessionStatus::Renewing => "renewing",
                    SessionStatus::Expired => "expired",
                    SessionStatus::Error(_) => "error"
                },
                "usage_percentage": session.usage_percentage,
                "remaining_time_seconds": session.remaining_time_seconds,
                "remaining_data_bytes": session.remaining_data_bytes,
                "total_spent": session.total_spent
            })
        })
        .collect();

    Ok(session_data)
}

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
        // Initialize TollGate service and runtime
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
            rt_clone.block_on(async {
                let mut service = TollGateService::new()
                    .await
                    .expect("Failed to create TollGate service");

                service
                    .start_background_service()
                    .await
                    .expect("Failed to start background service");

                Arc::new(Mutex::new(service))
            })
        };

        // Initialize NWC service
        let nwc_arc = {
            let rt_clone = rt.clone();
            let service_clone = service_arc.clone();
            rt_clone.block_on(async {
                // Generate or load NWC service key (using wallet's secret key)
                let service = service_clone.lock().await;
                let wallet_keys = service.get_wallet_keys().await;

                // Use wallet's secret key for NWC service
                let nwc_secret = wallet_keys.secret_key().clone();
                drop(service); // Release lock before creating NWC

                match NostrWalletConnect::new(nwc_secret, service_clone.clone()).await {
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

            // Clone the NWC service out of the Arc<Mutex<>> to avoid holding the lock
            let nwc_service = {
                let nwc_lock = nwc_clone.lock().await;
                nwc_lock.as_ref().cloned()
            }; // Lock is released here

            if let Some(nwc) = nwc_service {
                // Start the NWC service (connect to relay)
                log::info!("Starting NWC service and connecting to relay...");
                if let Err(e) = nwc.start().await {
                    log::error!("Failed to start NWC service: {}", e);
                    return;
                }
                log::info!("✓ NWC service started and connected to wss://nostrue.com");

                // Process events in a loop
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

        // Start connection server to handle wallet connection requests
        let _connection_service = service_arc.clone();
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
        let routstr_arc = Arc::new(Mutex::new(routstr::RoutstrService::new()));

        // Initialize auto-update for Routstr if already connected
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

        #[cfg(target_os = "macos")]
        {
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(panel) = window.to_panel() {
                    panel.set_released_when_closed(true);
                }
            }

            let app_handle = app.app_handle();
            setup_macos_panel(app_handle)?;
        }

        log::info!("TollGate service initialized");
        log::info!(
            "Connection server available at http://127.0.0.1:{}",
            connection_server::DEFAULT_CONNECTION_PORT
        );
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

    #[cfg(target_os = "macos")]
    {
        builder = builder.plugin(tauri_nspanel::init());
    }

    builder = builder
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::Webview),
                    Target::new(TargetKind::LogDir {
                        file_name: Some("tollgate_logs.txt".parse().unwrap()),
                    }),
                ])
                .filter(|metadata| {
                    // Filter out CDK and Cashu trace logs
                    if metadata.level() == log::Level::Trace
                        && (metadata.target().starts_with("cdk")
                            || metadata.target().starts_with("cashu")
                            || metadata.target().starts_with("tracing::span")
                            || metadata.target().starts_with("hyper")
                            || metadata.target().starts_with("reqwest")
                            || metadata.target().starts_with("h2")
                            || metadata.target().starts_with("rustls"))
                    {
                        return false;
                    }
                    true
                })
                .build(),
        )
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_androidwifi::init())
        .invoke_handler(tauri::generate_handler![
            toggle_auto_tollgate,
            get_tollgate_status,
            get_current_session,
            force_session_renewal,
            add_mint,
            set_default_mint,
            remove_mint,
            get_wallet_balance,
            handle_network_connected,
            handle_network_disconnected,
            detect_tollgate,
            get_active_sessions,
            get_mac_address,
            get_current_wifi_details,
            get_gateway_ip,
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

#[cfg(target_os = "macos")]
fn setup_macos_panel(app: &tauri::AppHandle) -> tauri::Result<()> {
    app.set_activation_policy(ActivationPolicy::Accessory)?;

    if let Some(window) = app.get_webview_window("main") {
        let panel = window.to_panel()?;

        panel.set_level(NSMainMenuWindowLevel + 1);
        panel.set_collection_behaviour(
            NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                | NSWindowCollectionBehavior::NSWindowCollectionBehaviorStationary
                | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary,
        );

        panel.set_becomes_key_only_if_needed(false);
        panel.set_hides_on_deactivate(false);
        panel.set_has_shadow(true);
    }

    TrayIconBuilder::with_id("wally-tray")
        .icon(load_tray_icon()?)
        .icon_as_template(true)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button_state,
                position,
                ..
            } = event
            {
                if button_state == MouseButtonState::Up {
                    toggle_panel(tray.app_handle(), Some(position));
                }
            }
        })
        .build(app)?;

    Ok(())
}

#[cfg(target_os = "macos")]
fn toggle_panel(app: &tauri::AppHandle, anchor: Option<PhysicalPosition<f64>>) {
    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Ok(panel) = app_handle.get_webview_panel("main") {
            if panel.is_visible() {
                panel.order_out(None);
            } else {
                if let Some(position) = anchor {
                    if let Some(window) = app_handle.get_webview_window("main") {
                        position_panel_at_menubar_icon(&window, position);
                    }
                }
                panel.set_level(NSMainMenuWindowLevel + 1);
                panel.order_front_regardless();
                panel.make_key_and_order_front(None);
            }
        }
    });
}

#[cfg(target_os = "macos")]
fn load_tray_icon() -> tauri::Result<Image<'static>> {
    Image::from_bytes(include_bytes!("../icons/trayTemplate.png"))
}

#[cfg(target_os = "macos")]
fn position_panel_at_menubar_icon(window: &tauri::WebviewWindow, anchor: PhysicalPosition<f64>) {
    if let (Ok(panel_handle), Ok(Some(monitor))) = (window.ns_window(), window.current_monitor()) {
        let scale_factor = monitor.scale_factor();
        let logical_pos: LogicalPosition<f64> = anchor.to_logical(scale_factor);
        let monitor_pos = monitor.position().to_logical::<f64>(scale_factor);
        let monitor_size = monitor.size().to_logical::<f64>(scale_factor);

        let menubar_height = 24.0_f64;

        let panel_id: id = panel_handle as _;

        #[allow(unexpected_cfgs)]
        let mut frame: NSRect = unsafe { msg_send![panel_id, frame] };

        frame.origin.y = (monitor_pos.y + monitor_size.height) - menubar_height - frame.size.height;
        frame.origin.x = logical_pos.x - frame.size.width / 2.0;

        #[allow(unexpected_cfgs)]
        let _: () = unsafe { msg_send![panel_id, setFrame: frame display: NO] };
    }
}
