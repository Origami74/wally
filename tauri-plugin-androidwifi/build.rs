const COMMANDS: &[&str] = &[
    "get_wifi_details",
    "get_current_wifi_details",
    "connect_wifi",
    "get_mac_address",
    "get_gateway_ip",
    "mark_captive_portal_dismissed",
    "register-listener",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .build();
}
