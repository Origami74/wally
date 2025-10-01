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

mod wallet;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    builder = builder.setup(|app| {
        // Initialize TollGate service
        let rt = tokio::runtime::Runtime::new().unwrap();
        let service = rt.block_on(async {
            let mut service = TollGateService::new()
                .await
                .expect("Failed to create TollGate service");

            service
                .start_background_service()
                .await
                .expect("Failed to start background service");

            service
        });

        app.manage(Arc::new(Mutex::new(service)));

        #[cfg(target_os = "macos")]
        {
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(panel) = window.to_panel() {
                    panel.set_released_when_closed(true);
                }
            }

            let app_handle = app.app_handle();
            setup_macos_panel(&app_handle)?;
        }

        log::info!("TollGate service initialized");
        Ok(())
    });

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
                    toggle_panel(&tray.app_handle(), Some(position));
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
                let _ = panel.order_out(None);
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

        let mut frame: NSRect = unsafe { msg_send![panel_id, frame] };

        frame.origin.y = (monitor_pos.y + monitor_size.height) - menubar_height - frame.size.height;
        frame.origin.x = logical_pos.x - frame.size.width / 2.0;

        let _: () = unsafe { msg_send![panel_id, setFrame: frame display: NO] };
    }
}
