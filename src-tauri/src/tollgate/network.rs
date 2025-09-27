//! Network detection and validation for TollGate connections
//! 
//! Handles:
//! - TollGate network detection
//! - Gateway IP validation
//! - MAC address retrieval
//! - Network connectivity checks

use crate::tollgate::errors::{TollGateError, TollGateResult};
use crate::tollgate::protocol::{TollGateAdvertisement, TollGateProtocol};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::time::Duration;

/// Network information for a detected TollGate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub gateway_ip: String,
    pub mac_address: String,
    pub is_tollgate: bool,
    pub advertisement: Option<TollGateAdvertisement>,
}

/// Network detector for TollGate networks
pub struct NetworkDetector {
    client: reqwest::Client,
    protocol: TollGateProtocol,
}

impl NetworkDetector {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            protocol: TollGateProtocol::new(),
        }
    }

    /// Detect if current network is a TollGate
    pub async fn detect_tollgate(&self, gateway_ip: &str, mac_address: &str) -> TollGateResult<NetworkInfo> {
        // Validate IP address format
        self.validate_gateway_ip(gateway_ip)?;
        self.validate_mac_address(mac_address)?;

        let mut network_info = NetworkInfo {
            gateway_ip: gateway_ip.to_string(),
            mac_address: mac_address.to_string(),
            is_tollgate: false,
            advertisement: None,
        };

        // Check if this is a TollGate network by trying to fetch the pubkey
        match self.check_tollgate_endpoint(gateway_ip).await {
            Ok(true) => {
                network_info.is_tollgate = true;
                
                // Fetch and validate advertisement
                match self.protocol.fetch_advertisement(gateway_ip).await {
                    Ok(advertisement) => {
                        self.protocol.validate_advertisement(&advertisement)?;
                        network_info.advertisement = Some(advertisement);
                    }
                    Err(e) => {
                        log::warn!("Failed to fetch TollGate advertisement: {}", e);
                        // Still mark as TollGate but without advertisement
                    }
                }
            }
            Ok(false) => {
                log::debug!("Network {} is not a TollGate", gateway_ip);
            }
            Err(e) => {
                log::debug!("Error checking TollGate endpoint {}: {}", gateway_ip, e);
            }
        }

        Ok(network_info)
    }

    /// Check if the gateway has TollGate endpoints using TIP-03
    async fn check_tollgate_endpoint(&self, gateway_ip: &str) -> TollGateResult<bool> {
        // Check for TIP-03 endpoint on port 2121
        let tollgate_url = format!("http://{}:2121/", gateway_ip);
        
        match self.client.get(&tollgate_url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    // Try to parse as JSON to see if it's a valid kind 10021 event
                    if let Ok(event_json) = response.json::<serde_json::Value>().await {
                        if let Some(kind) = event_json["kind"].as_u64() {
                            if kind == 10021 {
                                return Ok(true);
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // Endpoint not available
            }
        }

        Ok(false)
    }

    /// Validate gateway IP address format
    fn validate_gateway_ip(&self, ip: &str) -> TollGateResult<()> {
        ip.parse::<IpAddr>()
            .map_err(|_| TollGateError::InvalidGatewayIp(ip.to_string()))?;
        Ok(())
    }

    /// Validate MAC address format
    fn validate_mac_address(&self, mac: &str) -> TollGateResult<()> {
        // Basic MAC address validation (xx:xx:xx:xx:xx:xx format)
        let parts: Vec<&str> = mac.split(':').collect();
        
        if parts.len() != 6 {
            return Err(TollGateError::InvalidMacAddress(mac.to_string()));
        }

        for part in parts {
            if part.len() != 2 {
                return Err(TollGateError::InvalidMacAddress(mac.to_string()));
            }
            
            if !part.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(TollGateError::InvalidMacAddress(mac.to_string()));
            }
        }

        Ok(())
    }

    /// Test network connectivity
    #[allow(dead_code)]
    pub async fn test_connectivity(&self) -> TollGateResult<bool> {
        // Test connectivity to a reliable endpoint
        let test_urls = [
            "https://api.cloudflare.com/client/v4/ips",
            "https://httpbin.org/ip",
            "https://api.github.com",
        ];

        for url in &test_urls {
            match self.client.get(*url).timeout(Duration::from_secs(3)).send().await {
                Ok(response) if response.status().is_success() => {
                    return Ok(true);
                }
                _ => continue,
            }
        }

        Ok(false)
    }

    /// Check if we can reach the TollGate relay
    #[allow(dead_code)]
    pub async fn test_tollgate_relay(&self, gateway_ip: &str) -> TollGateResult<bool> {
        // Test WebSocket connection to TollGate relay
        let _relay_url = format!("ws://{}:3334", gateway_ip);
        
        // For now, just test HTTP connectivity to the same port
        let http_url = format!("http://{}:3334", gateway_ip);
        
        match self.client.get(&http_url).timeout(Duration::from_secs(3)).send().await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Get network quality metrics
    #[allow(dead_code)]
    pub async fn get_network_quality(&self, gateway_ip: &str) -> NetworkQuality {
        let mut quality = NetworkQuality::default();

        // Test latency to gateway
        let start = std::time::Instant::now();
        match self.client
            .get(&format!("http://{}:2122/pubkey", gateway_ip))
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            Ok(_) => {
                quality.latency_ms = start.elapsed().as_millis() as u32;
                quality.gateway_reachable = true;
            }
            Err(_) => {
                quality.gateway_reachable = false;
            }
        }

        // Test internet connectivity
        quality.internet_reachable = self.test_connectivity().await.unwrap_or(false);

        // Test TollGate relay
        quality.relay_reachable = self.test_tollgate_relay(gateway_ip).await.unwrap_or(false);

        quality
    }
}

/// Network quality metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct NetworkQuality {
    pub latency_ms: u32,
    pub gateway_reachable: bool,
    pub internet_reachable: bool,
    pub relay_reachable: bool,
}

impl Default for NetworkQuality {
    fn default() -> Self {
        Self {
            latency_ms: 0,
            gateway_reachable: false,
            internet_reachable: false,
            relay_reachable: false,
        }
    }
}

impl NetworkQuality {
    /// Check if network quality is good enough for TollGate operations
    #[allow(dead_code)]
    pub fn is_good_quality(&self) -> bool {
        self.gateway_reachable && 
        self.relay_reachable && 
        self.latency_ms < 1000 // Less than 1 second latency
    }

    /// Get quality score (0.0 to 1.0)
    #[allow(dead_code)]
    pub fn quality_score(&self) -> f64 {
        let mut score = 0.0;
        
        if self.gateway_reachable {
            score += 0.4;
        }
        
        if self.relay_reachable {
            score += 0.3;
        }
        
        if self.internet_reachable {
            score += 0.2;
        }
        
        // Latency score (better latency = higher score)
        if self.latency_ms > 0 {
            let latency_score = (1000.0 - self.latency_ms.min(1000) as f64) / 1000.0;
            score += latency_score * 0.1;
        }
        
        score
    }
}

impl Default for NetworkDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_gateway_ip() {
        let detector = NetworkDetector::new();
        
        assert!(detector.validate_gateway_ip("192.168.1.1").is_ok());
        assert!(detector.validate_gateway_ip("10.0.0.1").is_ok());
        assert!(detector.validate_gateway_ip("invalid_ip").is_err());
        assert!(detector.validate_gateway_ip("999.999.999.999").is_err());
    }

    #[test]
    fn test_validate_mac_address() {
        let detector = NetworkDetector::new();
        
        assert!(detector.validate_mac_address("aa:bb:cc:dd:ee:ff").is_ok());
        assert!(detector.validate_mac_address("00:11:22:33:44:55").is_ok());
        assert!(detector.validate_mac_address("invalid_mac").is_err());
        assert!(detector.validate_mac_address("aa:bb:cc:dd:ee").is_err());
        assert!(detector.validate_mac_address("aa:bb:cc:dd:ee:gg").is_err());
    }

    #[test]
    fn test_network_quality_score() {
        let mut quality = NetworkQuality::default();
        assert_eq!(quality.quality_score(), 0.0);
        
        quality.gateway_reachable = true;
        quality.relay_reachable = true;
        quality.internet_reachable = true;
        quality.latency_ms = 100;
        
        let score = quality.quality_score();
        assert!(score > 0.8); // Should be high quality
        assert!(quality.is_good_quality());
    }
}