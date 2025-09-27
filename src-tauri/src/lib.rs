use std::sync::Arc;
use tauri::{Manager, State};
use tauri_plugin_log::{Target, TargetKind};
use tokio::sync::Mutex;

mod tollgate;
use tollgate::TollGateService;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    builder
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
            
            log::info!("TollGate service initialized");
            Ok(())
        })
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
