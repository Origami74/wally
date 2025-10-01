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
  last_check_time: Arc<Mutex<std::time::Instant>>,
  active_session: Arc<Mutex<Option<TollgateSession>>>,
}

#[derive(Debug, Clone)]
struct TollgateSession {
  tollgate_pubkey: String,
  gateway_ip: String,
  mac_address: String,
  step_size: u64,
  metric: String,
  pricing_option: TollgatePricingOption,
  total_allotment: u64,
  current_usage: u64,
  session_start: std::time::Instant,
  last_payment: std::time::Instant,
  renewal_threshold: f64, // 0.8 = 80%
}

#[derive(Debug, Clone)]
struct TollgatePricingOption {
  mint_url: String,
  price: String,
  unit: String,
}

impl<R: Runtime> NetworkMonitor<R> {
  fn new(app: AppHandle<R>) -> Self {
    Self {
      app,
      last_gateway: Arc::new(Mutex::new(None)),
      last_network_status: Arc::new(Mutex::new(None)),
      last_check_time: Arc::new(Mutex::new(std::time::Instant::now())),
      active_session: Arc::new(Mutex::new(None)),
    }
  }

  async fn start_monitoring(&mut self) {
    loop {
      if let Err(e) = self.check_network_changes().await {
        eprintln!("Network monitoring error: {}", e);
      }
      
      // Check for automatic payment needs
      if let Err(e) = self.check_automatic_payment().await {
        eprintln!("Automatic payment check error: {}", e);
      }
      
      sleep(Duration::from_secs(1)).await;
    }
  }

  async fn check_network_changes(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    
    // Check if we should do a periodic check (every 10 seconds) even without gateway change
    let should_periodic_check = {
      let mut last_check = self.last_check_time.lock().unwrap();
      let elapsed = last_check.elapsed();
      if elapsed.as_secs() >= 10 {
        *last_check = std::time::Instant::now();
        true
      } else {
        false
      }
    };
    
    if gateway_changed || should_periodic_check {
      let check_reason = if gateway_changed { "gateway changed" } else { "periodic check" };
      println!("[WiFi Debug] Checking network status (reason: {})", check_reason);
      
      // Get full network status (includes tollgate detection)
      println!("[WiFi Debug] Background monitor: Getting full network status...");
      let network_status = self.get_full_network_status().await?;
      println!("[WiFi Debug] Background monitor: Network status - Gateway: {:?}, MAC: {:?}, Tollgate: {}",
               network_status.gateway_ip, network_status.mac_address, network_status.is_tollgate);
      
      // Update stored status
      {
        let mut last_status = self.last_network_status.lock().unwrap();
        *last_status = Some(network_status.clone());
      }
      
      // Emit network change event with tollgate info included
      println!("[WiFi Debug] Background monitor: Emitting network-status-changed event with Gateway: {:?}, MAC: {:?}",
               network_status.gateway_ip, network_status.mac_address);
      if let Err(e) = self.app.emit("network-status-changed", &network_status) {
        eprintln!("Failed to emit network status change: {}", e);
      } else {
        println!("[WiFi Debug] Background monitor: Successfully emitted network-status-changed event");
      }
      
      // Also emit separate tollgate event if tollgate detected
      if network_status.is_tollgate {
        println!("[WiFi Debug] Background monitor: Emitting tollgate-detected event with Gateway: {:?}, MAC: {:?}",
                 network_status.gateway_ip, network_status.mac_address);
        if let Err(e) = self.app.emit("tollgate-detected", &network_status) {
          eprintln!("Failed to emit tollgate detection: {}", e);
        } else {
          println!("[WiFi Debug] Background monitor: Successfully emitted tollgate-detected event");
        }
        
        // Start automatic payment session if not already active
        if let Err(e) = self.start_automatic_payment_session(&network_status).await {
          eprintln!("Failed to start automatic payment session: {}", e);
        }
      } else {
        // Clear session if no tollgate detected
        {
          let mut session = self.active_session.lock().unwrap();
          if session.is_some() {
            println!("[Tollgate Payment] No tollgate detected, clearing active session");
            *session = None;
          }
        }
      }
    }
    
    Ok(())
  }

  async fn get_current_gateway(&self) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    // Use default-net to get the default gateway
    match default_net::get_default_gateway() {
      Ok(gateway) => Ok(Some(gateway.ip_addr.to_string())),
      Err(_) => Ok(None),
    }
  }

  async fn get_full_network_status(&self) -> Result<NetworkStatusResponse, Box<dyn std::error::Error + Send + Sync>> {
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

  async fn get_current_wifi_info(&self) -> Result<Option<CurrentWifi>, Box<dyn std::error::Error + Send + Sync>> {
    println!("[WiFi Debug] Background monitor: Skipping WiFi scan - focusing on tollgate functionality");
    
    // Skip WiFi scanning for now - focus on gateway and tollgate detection
    Ok(None)
  }

  async fn start_automatic_payment_session(&self, network_status: &NetworkStatusResponse) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let advertisement = network_status.tollgate_advertisement.as_ref().unwrap();
    let gateway_ip = network_status.gateway_ip.as_ref().unwrap();
    let mac_address = network_status.mac_address.as_ref().unwrap();

    // Check if we already have an active session for this tollgate
    {
      let session = self.active_session.lock().unwrap();
      if let Some(existing_session) = &*session {
        if existing_session.tollgate_pubkey == advertisement.tollgate_pubkey {
          println!("[Tollgate Payment] Session already exists for tollgate: {}", existing_session.tollgate_pubkey);
          return Ok(());
        }
      }
    }

    println!("[Tollgate Payment] Detected new tollgate: {}", advertisement.tollgate_pubkey);
    println!("[Tollgate Payment] Making initial payment to establish session...");

    // Parse step_size and metric
    let step_size = advertisement.step_size.as_ref()
      .and_then(|s| s.parse::<u64>().ok())
      .unwrap_or(60000); // Default to 1 minute

    let metric = advertisement.metric.as_ref()
      .map(|s| s.clone())
      .unwrap_or_else(|| "milliseconds".to_string());

    // Select the first available pricing option (in a real implementation, we'd choose based on available mints)
    let pricing_option = if let Some(first_option) = advertisement.pricing_options.first() {
      TollgatePricingOption {
        mint_url: first_option.mint_url.clone(),
        price: first_option.price.clone(),
        unit: first_option.unit.clone(),
      }
    } else {
      println!("[Tollgate Payment] No pricing options available");
      return Ok(());
    };

    println!("[Tollgate Payment] Step size: {} {}, Price: {} {} per step", step_size, metric, pricing_option.price, pricing_option.unit);

    // Make initial payment to establish session
    match self.make_initial_tollgate_payment(gateway_ip, &advertisement.tollgate_pubkey, mac_address, &pricing_option, step_size, &metric).await {
      Ok(allotment) => {
        println!("[Tollgate Payment] Initial payment successful! Received allotment: {} {}", allotment, metric);
        
        // Now create the active session
        let session = TollgateSession {
          tollgate_pubkey: advertisement.tollgate_pubkey.clone(),
          gateway_ip: gateway_ip.clone(),
          mac_address: mac_address.clone(),
          step_size,
          metric,
          pricing_option,
          total_allotment: allotment,
          current_usage: 0,
          session_start: std::time::Instant::now(),
          last_payment: std::time::Instant::now(),
          renewal_threshold: 0.8, // 80%
        };

        // Store the active session
        {
          let mut active_session = self.active_session.lock().unwrap();
          *active_session = Some(session);
        }
        
        println!("[Tollgate Payment] Session established and active!");
      }
      Err(e) => {
        eprintln!("[Tollgate Payment] Failed to make initial payment: {}", e);
      }
    }

    Ok(())
  }

  async fn check_automatic_payment(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (should_renew, debug_info) = {
      let mut session_guard = self.active_session.lock().unwrap();
      if let Some(session) = &mut *session_guard {
        // Update current usage based on time elapsed
        let elapsed = session.session_start.elapsed();
        let current_usage = if session.metric == "milliseconds" {
          elapsed.as_millis() as u64
        } else {
          // For bytes, we'd need to track actual data usage
          // For now, just use time as a proxy
          elapsed.as_millis() as u64
        };

        // Update the session's current usage
        session.current_usage = current_usage;

        // Check if we need renewal (80% threshold)
        if session.total_allotment > 0 {
          let usage_percent = current_usage as f64 / session.total_allotment as f64;
          let needs_renewal = usage_percent >= session.renewal_threshold;
          
          let debug_info = if needs_renewal {
            format!("[Tollgate Payment] Usage at {:.1}%, triggering renewal (threshold: {:.1}%)",
                   usage_percent * 100.0, session.renewal_threshold * 100.0)
          } else {
            format!("[Tollgate Payment] Usage at {:.1}%, no renewal needed (threshold: {:.1}%)",
                   usage_percent * 100.0, session.renewal_threshold * 100.0)
          };
          
          (needs_renewal, Some(debug_info))
        } else {
          // This shouldn't happen for active sessions
          (false, Some("[Tollgate Payment] Active session has no allotment - this is unexpected".to_string()))
        }
      } else {
        (false, None)
      }
    };

    if let Some(info) = debug_info {
      println!("{}", info);
    }

    if should_renew {
      if let Err(e) = self.make_renewal_payment().await {
        eprintln!("[Tollgate Payment] Failed to make renewal payment: {}", e);
      }
    }

    Ok(())
  }

  async fn make_tollgate_payment(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (tollgate_pubkey, gateway_ip, mac_address, step_size, pricing_option) = {
      let session_guard = self.active_session.lock().unwrap();
      if let Some(session) = &*session_guard {
        (
          session.tollgate_pubkey.clone(),
          session.gateway_ip.clone(),
          session.mac_address.clone(),
          session.step_size,
          session.pricing_option.clone(),
        )
      } else {
        return Ok(()); // No active session
      }
    };

    println!("[Tollgate Payment] Making payment for {} {} to tollgate {}", step_size, "milliseconds", tollgate_pubkey);

    // Calculate required amount (1 step)
    let steps = 1u64;
    let required_amount = pricing_option.price.parse::<u64>().unwrap_or(0) * steps;

    println!("[Tollgate Payment] Required amount: {} {} for {} steps", required_amount, pricing_option.unit, steps);

    // Request payment from wallet service
    match self.request_payment_from_wallet(&pricing_option.mint_url, required_amount).await {
      Ok(cashu_token) => {
        println!("[Tollgate Payment] Received cashu token, sending to tollgate...");
        
        // Send payment to tollgate using TIP-03 HTTP endpoint
        match self.send_payment_to_tollgate(&gateway_ip, &tollgate_pubkey, &mac_address, &cashu_token).await {
          Ok(allotment) => {
            println!("[Tollgate Payment] Payment successful! Received allotment: {} milliseconds", allotment);
            
            // Update session with new allotment
            {
              let mut session_guard = self.active_session.lock().unwrap();
              if let Some(session) = &mut *session_guard {
                session.total_allotment += allotment;
                session.last_payment = std::time::Instant::now();
                println!("[Tollgate Payment] Updated session - Total allotment: {} milliseconds", session.total_allotment);
              }
            }
          }
          Err(e) => {
            eprintln!("[Tollgate Payment] Failed to send payment to tollgate: {}", e);
          }
        }
      }
      Err(e) => {
        eprintln!("[Tollgate Payment] Failed to get payment from wallet: {}", e);
      }
    }

    Ok(())
  }

  async fn make_initial_tollgate_payment(&self, gateway_ip: &str, tollgate_pubkey: &str, mac_address: &str, pricing_option: &TollgatePricingOption, step_size: u64, metric: &str) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    println!("[Tollgate Payment] Making initial payment for {} {} to tollgate {}", step_size, metric, tollgate_pubkey);

    // Trigger captive portal before making payment
    if let Err(e) = self.trigger_captive_portal(gateway_ip).await {
      println!("[Tollgate Payment] Warning: Failed to trigger captive portal: {}", e);
    }

    // Calculate required amount (1 step)
    let steps = 1u64;
    let required_amount = pricing_option.price.parse::<u64>().unwrap_or(0) * steps;

    println!("[Tollgate Payment] Required amount: {} {} for {} steps", required_amount, pricing_option.unit, steps);

    // Request payment from wallet service
    let cashu_token = self.request_payment_from_wallet(&pricing_option.mint_url, required_amount).await?;
    println!("[Tollgate Payment] Received cashu token, sending to tollgate...");
    
    // Send payment to tollgate using TIP-03 HTTP endpoint
    let allotment = self.send_payment_to_tollgate(gateway_ip, tollgate_pubkey, mac_address, &cashu_token).await?;
    println!("[Tollgate Payment] Initial payment successful! Received allotment: {} {}", allotment, metric);
    
    Ok(allotment)
  }

  async fn make_renewal_payment(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (tollgate_pubkey, gateway_ip, mac_address, step_size, pricing_option, metric) = {
      let session_guard = self.active_session.lock().unwrap();
      if let Some(session) = &*session_guard {
        (
          session.tollgate_pubkey.clone(),
          session.gateway_ip.clone(),
          session.mac_address.clone(),
          session.step_size,
          session.pricing_option.clone(),
          session.metric.clone(),
        )
      } else {
        return Ok(()); // No active session
      }
    };

    println!("[Tollgate Payment] Making renewal payment for {} {} to tollgate {}", step_size, metric, tollgate_pubkey);

    // Trigger captive portal before making payment
    if let Err(e) = self.trigger_captive_portal(&gateway_ip).await {
      println!("[Tollgate Payment] Warning: Failed to trigger captive portal: {}", e);
    }

    // Calculate required amount (1 step)
    let steps = 1u64;
    let required_amount = pricing_option.price.parse::<u64>().unwrap_or(0) * steps;

    println!("[Tollgate Payment] Required amount: {} {} for {} steps", required_amount, pricing_option.unit, steps);

    // Request payment from wallet service
    match self.request_payment_from_wallet(&pricing_option.mint_url, required_amount).await {
      Ok(cashu_token) => {
        println!("[Tollgate Payment] Received cashu token, sending to tollgate...");
        
        // Send payment to tollgate using TIP-3 HTTP endpoint
        match self.send_payment_to_tollgate(&gateway_ip, &tollgate_pubkey, &mac_address, &cashu_token).await {
          Ok(allotment) => {
            println!("[Tollgate Payment] Renewal payment successful! Received allotment: {} {}", allotment, metric);
            
            // Update session with new allotment
            {
              let mut session_guard = self.active_session.lock().unwrap();
              if let Some(session) = &mut *session_guard {
                session.total_allotment += allotment;
                session.last_payment = std::time::Instant::now();
                println!("[Tollgate Payment] Updated session - Total allotment: {} {}", session.total_allotment, metric);
              }
            }
          }
          Err(e) => {
            eprintln!("[Tollgate Payment] Failed to send renewal payment to tollgate: {}", e);
          }
        }
      }
      Err(e) => {
        eprintln!("[Tollgate Payment] Failed to get renewal payment from wallet: {}", e);
      }
    }

    Ok(())
  }

  async fn trigger_captive_portal(&self, gateway_ip: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Make a GET request to the gateway's port 80 to trigger captive portal software
    // This is a workaround to ensure the captive portal session is created before payment
    let client = reqwest::Client::new();
    let url = format!("http://{}:80/", gateway_ip);
    
    println!("[Captive Portal] Triggering captive portal at: {}", url);
    
    match client
      .get(&url)
      .timeout(std::time::Duration::from_secs(5))
      .send()
      .await
    {
      Ok(response) => {
        println!("[Captive Portal] Trigger successful, status: {}", response.status());
        Ok(())
      }
      Err(e) => {
        println!("[Captive Portal] Trigger failed: {}", e);
        // Don't fail the payment process if captive portal trigger fails
        Ok(())
      }
    }
  }

  async fn request_payment_from_wallet(&self, mint_url: &str, amount: u64) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    // This would call the main wallet service to create a cashu token
    // For now, return a mock token
    println!("[Tollgate Payment] Requesting {} sats from mint: {}", amount, mint_url);
    
    // In the real implementation, this would:
    // 1. Call the wallet service via Tauri command
    // 2. Request a cashu token for the specified amount and mint
    // 3. Return the token string
    
    // Mock implementation for now
    Ok("cashuAeyJ0b2tlbiI6W3sicHJvb2ZzIjpbeyJhbW91bnQiOjEsImlkIjoiMDA5YTFmMjkzMjUzZTQxZSIsInNlY3JldCI6IjQwNzkxNWJjMjEyYmU2MWE3N2UzZTZkMmFlYjRjNzI3OTgwYmRhNTFjZDA2YTZhZmMyOWUyODYxNzY4YTc4MzciLCJDIjoiMDJiYzlhZGY5NTY0ZTlkNjhkZjNmMzJjNzYzMzQ0NjE5NzM0ZjI4YzQ5ZjEyOWZlZGNiMjQ1ZGY0ZjZkNzNkOWNkIn1dLCJtaW50IjoiaHR0cHM6Ly84MzMzLnNwYWNlOjMzMzgifV0sInVuaXQiOiJzYXQiLCJtZW1vIjoiVGVzdCJ9".to_string())
  }

  async fn send_payment_to_tollgate(&self, gateway_ip: &str, tollgate_pubkey: &str, mac_address: &str, cashu_token: &str) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    // Generate customer keys for this payment
    let customer_keys = nostr::Keys::generate();
    
    // Parse tollgate pubkey
    let tollgate_pk = nostr::PublicKey::from_hex(tollgate_pubkey)
      .map_err(|e| format!("Invalid tollgate pubkey: {}", e))?;
    
    // Create payment event according to TIP-01
    let payment_event = nostr::EventBuilder::new(
      nostr::Kind::Custom(21000),
      ""
    )
    .tags(vec![
      nostr::Tag::public_key(tollgate_pk),
      nostr::Tag::custom(
        nostr::TagKind::Custom("device-identifier".into()),
        vec!["mac".to_string(), mac_address.to_string()]
      ),
      nostr::Tag::custom(
        nostr::TagKind::Custom("payment".into()),
        vec![cashu_token.to_string()]
      ),
    ])
    .sign(&customer_keys)
    .await
    .map_err(|e| format!("Failed to create and sign event: {}", e))?;
    
    // Convert to JSON for sending
    let payment_json = serde_json::to_value(&payment_event)
      .map_err(|e| format!("Failed to serialize event: {}", e))?;
    
    println!("[Tollgate Payment] Sending payment event:");
    println!("{}", serde_json::to_string_pretty(&payment_json).unwrap_or_default());

    // Send to tollgate using TIP-03 HTTP endpoint
    let client = reqwest::Client::new();
    let url = format!("http://{}:2121/", gateway_ip);
    
    println!("[Tollgate Payment] Sending payment to: {}", url);
    
    let response = client
      .post(&url)
      .header("Content-Type", "application/json")
      .json(&payment_json)
      .timeout(std::time::Duration::from_secs(10))
      .send()
      .await?;

    if response.status().is_success() {
      // Parse session response (kind 1022)
      let session_response: serde_json::Value = response.json().await?;
      
      // Extract allotment from response
      if let Some(tags) = session_response["tags"].as_array() {
        for tag in tags {
          if let Some(tag_array) = tag.as_array() {
            if tag_array.len() >= 2 && tag_array[0].as_str() == Some("allotment") {
              if let Some(allotment_str) = tag_array[1].as_str() {
                if let Ok(allotment) = allotment_str.parse::<u64>() {
                  return Ok(allotment);
                }
              }
            }
          }
        }
      }
      
      // Default allotment if not found in response
      Ok(60000) // 1 minute default
    } else {
      Err(format!("Payment failed with status: {}", response.status()).into())
    }
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
