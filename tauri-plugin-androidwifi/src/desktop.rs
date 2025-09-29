use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};
use wifi_rs::{prelude::*, WiFi};
use wifiscanner;
use mac_address::mac_address_by_name;
use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
  app: &AppHandle<R>,
  _api: PluginApi<R, C>,
) -> crate::Result<Androidwifi<R>> {
  Ok(Androidwifi(app.clone()))
}

/// Access to the androidwifi APIs.
pub struct Androidwifi<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Androidwifi<R> {
  pub fn get_wifi_details(&self, _payload: Empty) -> crate::Result<WifiDetailsResponse> {
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
    Ok (WifiDetailsResponse {
      wifis: Some(wifi_details)
    })
  }

  pub fn connect_wifi(&self, payload: ConnectWifiPayload) -> crate::Result<ConnectWifiResponse> {
    let config = Some(Config {
            interface: Some("en0"),
        });

    let mut wifi = WiFi::new(config);
    let ssid = payload.ssid;
    println!("ssid: {:?}", ssid);

    match wifi.connect(&ssid, "password") {
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

    Ok (ConnectWifiResponse {
      response: format!("YES, {}! You've been greeted from Rust!", ssid)
    })
    
  }
  pub fn get_current_wifi_details(&self, _payload: Empty) -> crate::Result<CurrentWifiResponse> {
    Ok(CurrentWifiResponse {
      // TODO mock data: replace with real current Wi-Fi lookup once implemented
      wifi: Some(CurrentWifi {
        ssid: "MockSSID".to_string(),
        bssid: "00:11:22:33:44:55".to_string(),
      }),
    })
  }

  pub fn get_mac_address(&self, _payload: GetMacAddressPayload) -> crate::Result<MacAddressResponse> {
    match mac_address_by_name("en0") {
      Ok(Some(ma)) => Ok(MacAddressResponse {
        mac_address: Some(ma.to_string()),
      }),
      Ok(None) | Err(_) => Ok(MacAddressResponse {
        // TODO mock data: replace with real MAC lookup once implemented
        mac_address: Some("AA:BB:CC:DD:EE:FF".to_string()),
      }),
    }
  }

  pub fn get_gateway_ip(&self, _payload: Empty) -> crate::Result<GatewayIpResponse> {
    Ok(GatewayIpResponse {
      // TODO mock data: replace with real gateway lookup once implemented
      gateway_ip: Some("192.168.0.1".to_string()),
    })
  }
}
