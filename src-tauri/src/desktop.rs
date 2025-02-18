

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
pub(crate) mod desktop_commands {

    use std::sync::{Arc, Mutex};
    use std::thread::sleep;
    use std::time::Duration;

    use tauri::{Manager, State};
    use tauri_plugin_log::{Target, TargetKind};

    use cdk::nuts::{CurrencyUnit, MintQuoteState};
    use cdk::wallet::MintQuote;
    use cdk::Amount;
    use cdk::{cdk_database::WalletMemoryDatabase, wallet::Wallet};
    use mac_address::mac_address_by_name;
    use tauri_plugin_androidwifi::WifiDetails;
    use wifi_rs::{prelude::*, WiFi};

    #[tauri::command]
    pub fn get_mac_address() -> String {
       match mac_address_by_name("en0") {
           Ok(Some(mac_address)) => mac_address.to_string(),
           Ok(None) => "null".to_string(),
           Err(err) => "err".to_string(),
       }
    }

    #[tauri::command]
    pub fn get_available_networks() -> Result<Vec<WifiDetails>, String> {
       let wifis  = wifiscanner::scan().expect("Failed to scan wifi");

        println!("{:?}", wifis);
        let mut wifi_details = Vec::new();
        wifis.iter().for_each(|wifi| {
            wifi_details.push(WifiDetails {
                ssid: wifi.ssid.to_string(),
                frequency: wifi.channel.to_string(),
                rssi: wifi.signal_level.to_string(),
                bssid: "".to_string(),
                capabilities: "".to_string(),
                information_elements: vec![],
            })
        });

       Ok(wifi_details)
    }


    #[tauri::command]
    pub fn connect_network(ssid: &str, password: &str) -> String {
       log::info!("Tauri is awesome2!");

       let config = Some(Config {
               interface: Some("en0"),
           });

       let mut wifi = WiFi::new(config);
       println!("ssid: {:?}", ssid);

       match wifi.connect(ssid, password) {
           Ok(result) => println!(
               "{}",
               if result == true {
                   "Connection Successful."
               } else {
                   "Invalid password."
               }
           ),
           Err(err) => println!("The following error occurred: {:?}", err),
       }

       format!("YES, {}! You've been greeted from Rust!", ssid)

    }
}

