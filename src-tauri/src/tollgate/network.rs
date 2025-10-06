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
    pub async fn detect_tollgate(
        &self,
        gateway_ip: &str,
        mac_address: &str,
    ) -> TollGateResult<NetworkInfo> {
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

}
