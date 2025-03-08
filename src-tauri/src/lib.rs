use tauri_plugin_log::Target;
use tauri_plugin_log::TargetKind;


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    builder
    .plugin(
        tauri_plugin_log::Builder::new()
            .targets([
                Target::new(TargetKind::Stdout),
                Target::new(TargetKind::Webview),
            ])
            .build(),
    )
    .plugin(tauri_plugin_http::init())
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_os::init())
    .plugin(tauri_plugin_androidwifi::init())
//     .plugin(tauri_plugin_sharetarget::init())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
