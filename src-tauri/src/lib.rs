use std::sync::Arc;
use tauri::{Manager, State};
use tauri_plugin_log::{Target, TargetKind};
use tokio::sync::Mutex;
use tauri_plugin_androidwifi::{AndroidwifiExt, GetMacAddressPayload, Empty};

#[cfg(target_os = "macos")]
use tauri_nspanel::WebviewWindowExt;

mod tollgate;
use tollgate::TollGateService;
use tollgate::session::SessionStatus;

// Global state for the TollGate service
type TollGateState = Arc<Mutex<TollGateService>>;

#[tauri::command]
async fn toggle_auto_tollgate(
    enabled: bool,
    state: State<'_, TollGateState>,
) -> Result<(), String> {
    let service = state.lock().await;
    service.set_auto_tollgate_enabled(enabled).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_tollgate_status(
    state: State<'_, TollGateState>,
) -> Result<tollgate::service::ServiceStatus, String> {
    let service = state.lock().await;
    service.get_status().await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_current_session(
    state: State<'_, TollGateState>,
) -> Result<Option<tollgate::service::SessionInfo>, String> {
    let service = state.lock().await;
    service.get_current_session().await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn force_session_renewal(
    tollgate_pubkey: String,
    state: State<'_, TollGateState>,
) -> Result<(), String> {
    let service = state.lock().await;
    service.force_renewal(&tollgate_pubkey).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn add_mint(
    mint_url: String,
    state: State<'_, TollGateState>,
) -> Result<(), String> {
    let service = state.lock().await;
    service.add_mint(&mint_url).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_wallet_balance(
    state: State<'_, TollGateState>,
) -> Result<u64, String> {
    let service = state.lock().await;
    service.get_wallet_balance().await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn handle_network_connected(
    gateway_ip: String,
    mac_address: String,
    state: State<'_, TollGateState>,
) -> Result<(), String> {
    let service = state.lock().await;
    service.handle_network_connected(gateway_ip, mac_address).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn handle_network_disconnected(
    state: State<'_, TollGateState>,
) -> Result<(), String> {
    let service = state.lock().await;
    service.handle_network_disconnected().await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn detect_tollgate(gateway_ip: String, mac_address: String, state: State<'_, TollGateState>) -> Result<serde_json::Value, String> {
    let service = state.lock().await;
    
    match service.detect_tollgate(&gateway_ip, &mac_address).await {
        Ok(network_info) => {
            let result = serde_json::json!({
                "is_tollgate": network_info.advertisement.is_some(),
                "advertisement": network_info.advertisement
            });
            Ok(result)
        }
        Err(e) => Err(format!("Failed to detect TollGate: {}", e))
    }
}

#[tauri::command]
async fn get_mac_address(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let payload = GetMacAddressPayload {
        gateway_ip: "192.168.1.1".to_string(),
    };
    
    match app.androidwifi().get_mac_address(payload) {
        Ok(result) => Ok(serde_json::to_value(result).unwrap()),
        Err(e) => Err(format!("Failed to get MAC address: {}", e))
    }
}

#[tauri::command]
async fn get_current_wifi_details(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let payload = Empty { value: None };
    
    match app.androidwifi().get_current_wifi_details(payload) {
        Ok(result) => Ok(serde_json::to_value(result).unwrap()),
        Err(e) => Err(format!("Failed to get current WiFi details: {}", e))
    }
}

#[tauri::command]
async fn get_gateway_ip(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let payload = Empty { value: None };
    
    match app.androidwifi().get_gateway_ip(payload) {
        Ok(result) => Ok(serde_json::to_value(result).unwrap()),
        Err(e) => Err(format!("Failed to get gateway IP: {}", e))
    }
}

#[tauri::command]
async fn get_active_sessions(state: State<'_, TollGateState>) -> Result<Vec<serde_json::Value>, String> {
    let service = state.lock().await;
    
    let sessions = service.get_active_sessions().await.map_err(|e| e.to_string())?;
    let session_data: Vec<serde_json::Value> = sessions.iter().map(|session| {
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
    }).collect();
    
    Ok(session_data)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    builder = builder
        .setup(|app| {
            // Initialize TollGate service
            let rt = tokio::runtime::Runtime::new().unwrap();
            let service = rt.block_on(async {
                let mut service = TollGateService::new().await
                    .expect("Failed to create TollGate service");
                
                service.start_background_service().await
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
                        file_name: Some("tollgate_logs.txt".parse().unwrap())
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
        ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
