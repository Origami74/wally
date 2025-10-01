use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime, Emitter};
use wifi_rs::{prelude::*, WiFi};
// Removed wifiscanner due to macOS BSSID detection issues
use mac_address::mac_address_by_name;
use default_net;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;
use crate::models::*;
use crate::tollgate::TollgateDetector;

pub fn init<R: Runtime, C: DeserializeOwned>(
  app: &AppHandle<R>,
  _api: PluginApi<R, C>,
) -> crate::Result<Androidwifi<R>> {
  let androidwifi = Androidwifi(app.clone());
  
  // Start network monitoring in background using Tauri's async runtime
  let app_handle = app.clone();
  tauri::async_runtime::spawn(async move {
    let mut monitor = NetworkMonitor::new(app_handle);
    monitor.start_monitoring().await;
  });
  
  Ok(androidwifi)
}

/// Access to the androidwifi APIs.
pub struct Androidwifi<R: Runtime>(AppHandle<R>);

struct NetworkMonitor<R: Runtime> {
  app: AppHandle<R>,
  last_gateway: Arc<Mutex<Option<String>>>,
  last_network_status: Arc<Mutex<Option<NetworkStatusResponse>>>,
}

impl<R: Runtime> NetworkMonitor<R> {
  fn new(app: AppHandle<R>) -> Self {
    Self {
      app,
      last_gateway: Arc::new(Mutex::new(None)),
      last_network_status: Arc::new(Mutex::new(None)),
    }
  }

  async fn start_monitoring(&mut self) {
    loop {
      if let Err(e) = self.check_network_changes().await {
        eprintln!("Network monitoring error: {}", e);
      }
      sleep(Duration::from_secs(5)).await;
    }
  }

  async fn check_network_changes(&self) -> Result<(), Box<dyn std::error::Error>> {
    let current_gateway = self.get_current_gateway().await?;
    
    let gateway_changed = {
      let mut last_gateway = self.last_gateway.lock().unwrap();
      let changed = *last_gateway != current_gateway;
      if changed {
        println!("Gateway changed from {:?} to {:?}", *last_gateway, current_gateway);
        *last_gateway = current_gateway.clone();
      }
      changed
    };
    
    if gateway_changed {
      
      // Get full network status (includes tollgate detection)
      println!("[WiFi Debug] Background monitor: Getting full network status...");
      let network_status = self.get_full_network_status().await?;
      println!("[WiFi Debug] Background monitor: Network status - Gateway: {:?}, Tollgate: {}",
               network_status.gateway_ip, network_status.is_tollgate);
      
      // Update stored status
      {
        let mut last_status = self.last_network_status.lock().unwrap();
        *last_status = Some(network_status.clone());
      }
      
      // Emit network change event with tollgate info included
      println!("[WiFi Debug] Background monitor: Emitting network-status-changed event");
      if let Err(e) = self.app.emit("network-status-changed", &network_status) {
        eprintln!("Failed to emit network status change: {}", e);
      } else {
        println!("[WiFi Debug] Background monitor: Successfully emitted network-status-changed event");
      }
      
      // Also emit separate tollgate event if tollgate detected
      if network_status.is_tollgate {
        println!("[WiFi Debug] Background monitor: Emitting tollgate-detected event");
        if let Err(e) = self.app.emit("tollgate-detected", &network_status) {
          eprintln!("Failed to emit tollgate detection: {}", e);
        } else {
          println!("[WiFi Debug] Background monitor: Successfully emitted tollgate-detected event");
        }
      }
    }
    
    Ok(())
  }

  async fn get_current_gateway(&self) -> Result<Option<String>, Box<dyn std::error::Error>> {
    // Use default-net to get the default gateway
    match default_net::get_default_gateway() {
      Ok(gateway) => Ok(Some(gateway.ip_addr.to_string())),
      Err(_) => Ok(None),
    }
  }

  async fn get_full_network_status(&self) -> Result<NetworkStatusResponse, Box<dyn std::error::Error>> {
    println!("[WiFi Debug] get_full_network_status: Starting...");
    let gateway_ip = self.get_current_gateway().await?;
    println!("[WiFi Debug] get_full_network_status: Gateway IP: {:?}", gateway_ip);
    
    // Get MAC address for the default interface
    let mac_address = if let Ok(default_interface) = default_net::get_default_interface() {
      mac_address_by_name(&default_interface.name)
        .ok()
        .flatten()
        .map(|mac| mac.to_string())
    } else {
      None
    };
    println!("[WiFi Debug] get_full_network_status: MAC address: {:?}", mac_address);

    // Get current WiFi info
    let current_wifi = self.get_current_wifi_info().await?;
    println!("[WiFi Debug] get_full_network_status: WiFi info: {:?}", current_wifi);
    
    // Check if this is a tollgate
    let (is_tollgate, tollgate_advertisement) = if let Some(ref gateway) = gateway_ip {
      println!("[WiFi Debug] get_full_network_status: Checking tollgate at gateway: {}", gateway);
      let detector = TollgateDetector::new();
      match detector.check_tollgate_at_gateway(gateway).await {
        Ok(detection) => {
          println!("[WiFi Debug] get_full_network_status: Tollgate detection successful - is_tollgate: {}, has_ad: {}",
                   detection.is_tollgate, detection.advertisement.is_some());
          (detection.is_tollgate, detection.advertisement)
        },
        Err(e) => {
          println!("[WiFi Debug] get_full_network_status: Tollgate detection failed: {:?}", e);
          (false, None)
        },
      }
    } else {
      println!("[WiFi Debug] get_full_network_status: No gateway, skipping tollgate detection");
      (false, None)
    };

    let response = NetworkStatusResponse {
      gateway_ip,
      mac_address,
      current_wifi,
      is_tollgate,
      tollgate_advertisement,
    };
    
    println!("[WiFi Debug] get_full_network_status: Final response - is_tollgate: {}, has_ad: {}",
             response.is_tollgate, response.tollgate_advertisement.is_some());
    
    Ok(response)
  }

  async fn get_current_wifi_info(&self) -> Result<Option<CurrentWifi>, Box<dyn std::error::Error>> {
    println!("[WiFi Debug] Background monitor: Skipping WiFi scan - focusing on tollgate functionality");
    
    // Skip WiFi scanning for now - focus on gateway and tollgate detection
    Ok(None)
  }

}

impl<R: Runtime> Androidwifi<R> {
  pub fn get_wifi_details(&self, _payload: Empty) -> crate::Result<WifiDetailsResponse> {
    println!("[WiFi Debug] Skipping WiFi scan - focusing on tollgate functionality");
    
    // Skip WiFi scanning for now - focus on gateway and tollgate detection
    Ok(WifiDetailsResponse {
      wifis: Some(vec![])
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
    println!("[WiFi Debug] Skipping WiFi details - focusing on tollgate functionality");
    
    // Skip WiFi scanning for now - focus on gateway and tollgate detection
    Ok(CurrentWifiResponse {
      wifi: Some(CurrentWifi {
        ssid: "Current Network".to_string(),
        bssid: "00:00:00:00:00:00".to_string(),
      }),
    })
  }

  pub fn get_mac_address(&self, _payload: GetMacAddressPayload) -> crate::Result<MacAddressResponse> {
    // Try to get MAC address for the default interface
    let mac_address = if let Ok(default_interface) = default_net::get_default_interface() {
      mac_address_by_name(&default_interface.name)
        .ok()
        .flatten()
        .map(|mac| mac.to_string())
    } else {
      // Fallback to common interface names
      mac_address_by_name("en0")
        .ok()
        .flatten()
        .or_else(|| mac_address_by_name("wlan0").ok().flatten())
        .or_else(|| mac_address_by_name("wifi0").ok().flatten())
        .map(|mac| mac.to_string())
    };

    Ok(MacAddressResponse {
      mac_address,
    })
  }

  pub fn get_gateway_ip(&self, _payload: Empty) -> crate::Result<GatewayIpResponse> {
    // Use default-net to get the actual gateway IP
    let gateway_ip = match default_net::get_default_gateway() {
      Ok(gateway) => Some(gateway.ip_addr.to_string()),
      Err(_) => None,
    };

    Ok(GatewayIpResponse {
      gateway_ip,
    })
  }

  pub fn detect_tollgate(&self, _payload: Empty) -> crate::Result<TollgateDetectionResponse> {
    // For now, return a simple response without async detection
    // The async detection will happen in the background monitoring
    let _gateway_ip = match default_net::get_default_gateway() {
      Ok(gateway) => gateway.ip_addr.to_string(),
      Err(_) => return Ok(TollgateDetectionResponse {
        is_tollgate: false,
        advertisement: None,
      }),
    };

    // Return basic info - the background monitor will handle async tollgate detection
    Ok(TollgateDetectionResponse {
      is_tollgate: false, // Will be updated by background monitoring
      advertisement: None,
    })
  }

  pub async fn get_network_status(&self, _payload: Empty) -> crate::Result<NetworkStatusResponse> {
    println!("[WiFi Debug] Getting network status...");
    
    let gateway_ip = match default_net::get_default_gateway() {
      Ok(gateway) => {
        let ip = gateway.ip_addr.to_string();
        println!("[WiFi Debug] Gateway IP: {}", ip);
        Some(ip)
      },
      Err(e) => {
        println!("[WiFi Debug] Failed to get gateway: {:?}", e);
        None
      }
    };
    
    // Get MAC address for the default interface
    let mac_address = if let Ok(default_interface) = default_net::get_default_interface() {
      println!("[WiFi Debug] Default interface: {}", default_interface.name);
      match mac_address_by_name(&default_interface.name) {
        Ok(Some(mac)) => {
          let mac_str = mac.to_string();
          println!("[WiFi Debug] MAC address: {}", mac_str);
          Some(mac_str)
        },
        Ok(None) => {
          println!("[WiFi Debug] No MAC address found for interface");
          None
        },
        Err(e) => {
          println!("[WiFi Debug] Error getting MAC address: {:?}", e);
          None
        }
      }
    } else {
      println!("[WiFi Debug] Failed to get default interface");
      None
    };

    // Skip WiFi scanning for now - focus on gateway and tollgate detection
    let current_wifi = Some(CurrentWifi {
      ssid: "Current Network".to_string(),
      bssid: "00:00:00:00:00:00".to_string(),
    });
    
    // Check for tollgate if we have a gateway
    let (is_tollgate, tollgate_advertisement) = if let Some(ref gateway) = gateway_ip {
      println!("[WiFi Debug] Checking for tollgate at gateway: {}", gateway);
      let detector = TollgateDetector::new();
      match detector.check_tollgate_at_gateway(gateway).await {
        Ok(detection_result) => {
          println!("[WiFi Debug] Tollgate detection result: is_tollgate={}, has_advertisement={}",
                   detection_result.is_tollgate, detection_result.advertisement.is_some());
          if let Some(ref ad) = detection_result.advertisement {
            println!("[WiFi Debug] Advertisement details: pubkey={}, tips={:?}, metric={:?}, step_size={:?}, pricing_options_count={}",
                     ad.tollgate_pubkey, ad.tips, ad.metric, ad.step_size, ad.pricing_options.len());
          }
          (detection_result.is_tollgate, detection_result.advertisement)
        },
        Err(e) => {
          println!("[WiFi Debug] Tollgate detection failed: {:?}", e);
          (false, None)
        }
      }
    } else {
      println!("[WiFi Debug] No gateway IP, skipping tollgate detection");
      (false, None)
    };
    
    println!("[WiFi Debug] Final network status - Gateway: {:?}, MAC: {:?}, WiFi: {:?}, Tollgate: {}",
             gateway_ip, mac_address, current_wifi, is_tollgate);
    
    Ok(NetworkStatusResponse {
      gateway_ip,
      mac_address,
      current_wifi,
      is_tollgate,
      tollgate_advertisement,
    })
  }

}
