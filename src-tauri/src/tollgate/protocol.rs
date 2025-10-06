//! TollGate protocol implementation
//!
//! Handles TollGate protocol operations following the reference implementation:
//! - Advertisement parsing (kind 10021)
//! - Payment creation (kind 21000)
//! - Session confirmation (kind 21001)

use crate::tollgate::errors::{TollGateError, TollGateResult};
use chrono::{DateTime, Utc};
use nostr::{Event, EventBuilder, Keys, Kind, Tag};
use serde::{Deserialize, Serialize};

/// TollGate advertisement information (parsed from kind 10021 events)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TollGateAdvertisement {
    pub metric: String, // "milliseconds" or "bytes"
    pub step_size: u64, // Step size from advertisement
    pub pricing_options: Vec<PricingOption>,
    pub tips: Vec<String>,       // Supported TIP numbers
    pub tollgate_pubkey: String, // TollGate's public key
}

/// Pricing option from advertisement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingOption {
    pub asset_type: String,  // "cashu"
    pub price_per_step: u64, // Price per step in units
    pub price_unit: String,  // Price unit (e.g., "sat")
    pub mint_url: String,    // Accepted mint URL
    pub min_steps: u64,      // Minimum steps to purchase
}

/// Payment event to send to TollGate (kind 21000)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentEvent {
    pub tollgate_pubkey: String,
    pub mac_address: String,
    pub cashu_token: String,
    pub steps: u64,
}

/// Session response from TollGate (kind 21001 or similar)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResponse {
    pub session_id: String,
    pub allotment: u64,             // Granted usage amount
    pub session_end: DateTime<Utc>, // When session expires
    pub mac_address: String,
}

/// Main protocol handler
#[derive(Clone)]
pub struct TollGateProtocol {
    client: reqwest::Client,
}

impl TollGateProtocol {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Fetch TollGate advertisement from gateway using TIP-03 HTTP endpoint
    pub async fn fetch_advertisement(
        &self,
        gateway_ip: &str,
    ) -> TollGateResult<TollGateAdvertisement> {
        // Use TIP-03: GET / endpoint to get the advertisement (kind 10021)
        let advertisement_url = format!("http://{}:2121/", gateway_ip);

        let response = self
            .client
            .get(&advertisement_url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(TollGateError::TollGateNotFound);
        }

        let event_json: serde_json::Value = response.json().await?;

        // Parse the kind 10021 event
        self.parse_advertisement_event(&event_json)
    }

    /// Parse advertisement event (kind 10021) following TIP-01 and TIP-02
    fn parse_advertisement_event(
        &self,
        event: &serde_json::Value,
    ) -> TollGateResult<TollGateAdvertisement> {
        let kind = event["kind"]
            .as_u64()
            .ok_or_else(|| TollGateError::InvalidAdvertisement("Missing kind".to_string()))?;

        if kind != 10021 {
            return Err(TollGateError::InvalidAdvertisement(format!(
                "Expected kind 10021, got {}",
                kind
            )));
        }

        let tollgate_pubkey = event["pubkey"]
            .as_str()
            .ok_or_else(|| TollGateError::InvalidAdvertisement("Missing pubkey".to_string()))?
            .to_string();

        let tags = event["tags"]
            .as_array()
            .ok_or_else(|| TollGateError::InvalidAdvertisement("Missing tags".to_string()))?;

        let mut metric = None;
        let mut step_size = None;
        let mut pricing_options = Vec::new();
        let mut tips = Vec::new();

        for tag in tags {
            let tag_array = tag.as_array().ok_or_else(|| {
                TollGateError::InvalidAdvertisement("Invalid tag format".to_string())
            })?;

            if tag_array.is_empty() {
                continue;
            }

            let tag_name = tag_array[0].as_str().ok_or_else(|| {
                TollGateError::InvalidAdvertisement("Invalid tag name".to_string())
            })?;

            match tag_name {
                "metric" => {
                    if tag_array.len() >= 2 {
                        metric = tag_array[1].as_str().map(|s| s.to_string());
                    }
                }
                "step_size" => {
                    if tag_array.len() >= 2 {
                        step_size = tag_array[1].as_str().and_then(|s| s.parse::<u64>().ok());
                    }
                }
                "price_per_step" => {
                    // TIP-02: ["price_per_step", "cashu", "210", "sat", "https://mint.url", "1"]
                    if tag_array.len() >= 6 {
                        let asset_type = tag_array[1].as_str().unwrap_or("").to_string();
                        let price_per_step = tag_array[2]
                            .as_str()
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(0);
                        let price_unit = tag_array[3].as_str().unwrap_or("").to_string();
                        let mint_url = tag_array[4].as_str().unwrap_or("").to_string();
                        let min_steps = tag_array[5]
                            .as_str()
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(1);

                        pricing_options.push(PricingOption {
                            asset_type,
                            price_per_step,
                            price_unit,
                            mint_url,
                            min_steps,
                        });
                    }
                }
                "tips" => {
                    // Collect all TIP numbers
                    for item in tag_array.iter().skip(1) {
                        if let Some(tip) = item.as_str() {
                            tips.push(tip.to_string());
                        }
                    }
                }
                _ => {} // Ignore unknown tags
            }
        }

        let advertisement = TollGateAdvertisement {
            metric: metric
                .ok_or_else(|| TollGateError::InvalidAdvertisement("Missing metric".to_string()))?,
            step_size: step_size.ok_or_else(|| {
                TollGateError::InvalidAdvertisement("Missing step_size".to_string())
            })?,
            pricing_options,
            tips,
            tollgate_pubkey,
        };

        Ok(advertisement)
    }

    /// Get device identifier using TIP-04 /whoami endpoint
    pub async fn get_device_identifier(
        &self,
        gateway_ip: &str,
    ) -> TollGateResult<(String, String)> {
        let whoami_url = format!("http://{}:2121/whoami", gateway_ip);

        let response = self
            .client
            .get(&whoami_url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(TollGateError::protocol("Failed to get device identifier"));
        }

        let body = response.text().await?;

        // Parse format: "type=value" (e.g., "mac=00:1A:2B:3C:4D:5E")
        if let Some((device_type, device_value)) = body.trim().split_once('=') {
            Ok((device_type.to_string(), device_value.to_string()))
        } else {
            Err(TollGateError::protocol("Invalid device identifier format"))
        }
    }

    /// Create a payment event (kind 21000) following TIP-01
    pub async fn create_payment_event(
        &self,
        payment: &PaymentEvent,
        customer_keys: &Keys,
        device_type: &str,
        device_value: &str,
    ) -> TollGateResult<Event> {
        let tags = vec![
            Tag::parse(vec!["p".to_string(), payment.tollgate_pubkey.clone()])
                .map_err(|e| TollGateError::protocol(format!("Invalid pubkey tag: {}", e)))?,
            Tag::parse(vec![
                "device-identifier".to_string(),
                device_type.to_string(),
                device_value.to_string(),
            ])
            .map_err(|e| {
                TollGateError::protocol(format!("Invalid device-identifier tag: {}", e))
            })?,
            Tag::parse(vec!["payment".to_string(), payment.cashu_token.clone()])
                .map_err(|e| TollGateError::protocol(format!("Invalid payment tag: {}", e)))?,
        ];

        let event = EventBuilder::new(
            Kind::Custom(21000),
            "", // Empty content for payment events
        )
        .tags(tags)
        .sign_with_keys(customer_keys)
        .map_err(|e| TollGateError::protocol(format!("Failed to create event: {}", e)))?;

        Ok(event)
    }

    /// Send payment using TIP-03 HTTP endpoint
    pub async fn send_payment(
        &self,
        gateway_ip: &str,
        payment_event: &Event,
    ) -> TollGateResult<SessionResponse> {
        let payment_url = format!("http://{}:2121/", gateway_ip);

        let response = self
            .client
            .post(&payment_url)
            .header("Content-Type", "application/json")
            .json(payment_event)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        match response.status().as_u16() {
            200 => {
                // Success - parse session response (kind 1022)
                let session_event: serde_json::Value = response.json().await?;
                self.parse_session_event(&session_event)
            }
            402 => {
                // Payment Required - get error message
                let error_msg = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Payment required".to_string());
                Err(TollGateError::protocol(format!(
                    "Payment rejected: {}",
                    error_msg
                )))
            }
            _ => {
                let status = response.status();
                let error_msg = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                Err(TollGateError::protocol(format!(
                    "HTTP error {}: {}",
                    status, error_msg
                )))
            }
        }
    }

    /// Parse session event (kind 1022) following TIP-01
    fn parse_session_event(&self, event: &serde_json::Value) -> TollGateResult<SessionResponse> {
        let kind = event["kind"]
            .as_u64()
            .ok_or_else(|| TollGateError::protocol("Missing kind in session response"))?;

        if kind != 1022 {
            return Err(TollGateError::protocol(format!(
                "Expected kind 1022, got {}",
                kind
            )));
        }

        let tags = event["tags"]
            .as_array()
            .ok_or_else(|| TollGateError::protocol("Missing tags in session response"))?;

        let mut _customer_pubkey = None;
        let mut device_identifier = None;
        let mut allotment = None;
        let mut _metric = None;

        for tag in tags {
            let tag_array = tag
                .as_array()
                .ok_or_else(|| TollGateError::protocol("Invalid tag format in session response"))?;

            if tag_array.is_empty() {
                continue;
            }

            let tag_name = tag_array[0]
                .as_str()
                .ok_or_else(|| TollGateError::protocol("Invalid tag name in session response"))?;

            match tag_name {
                "p" => {
                    if tag_array.len() >= 2 {
                        _customer_pubkey = tag_array[1].as_str().map(|s| s.to_string());
                    }
                }
                "device-identifier" => {
                    if tag_array.len() >= 3 {
                        device_identifier = Some(format!(
                            "{}={}",
                            tag_array[1].as_str().unwrap_or(""),
                            tag_array[2].as_str().unwrap_or("")
                        ));
                    }
                }
                "allotment" => {
                    if tag_array.len() >= 2 {
                        allotment = tag_array[1].as_str().and_then(|s| s.parse::<u64>().ok());
                    }
                }
                "metric" => {
                    if tag_array.len() >= 2 {
                        _metric = tag_array[1].as_str().map(|s| s.to_string());
                    }
                }
                _ => {} // Ignore unknown tags
            }
        }

        // Generate session ID from event ID or create one
        let session_id = event["id"]
            .as_str()
            .unwrap_or(&uuid::Uuid::new_v4().to_string())
            .to_string();

        // For now, set session end to 1 hour from now (this should come from the session event)
        let session_end = chrono::Utc::now() + chrono::Duration::hours(1);

        let session_response = SessionResponse {
            session_id,
            allotment: allotment
                .ok_or_else(|| TollGateError::protocol("Missing allotment in session response"))?,
            session_end,
            mac_address: device_identifier
                .and_then(|id| id.split_once('=').map(|(_, value)| value.to_string()))
                .ok_or_else(|| TollGateError::protocol("Missing or invalid device identifier"))?,
        };

        Ok(session_response)
    }


    /// Validate TollGate advertisement
    pub fn validate_advertisement(&self, ad: &TollGateAdvertisement) -> TollGateResult<()> {
        if ad.metric != "milliseconds" && ad.metric != "bytes" {
            return Err(TollGateError::InvalidAdvertisement(format!(
                "Invalid metric: {}",
                ad.metric
            )));
        }

        if ad.step_size == 0 {
            return Err(TollGateError::InvalidAdvertisement(
                "Step size cannot be zero".to_string(),
            ));
        }

        if ad.pricing_options.is_empty() {
            return Err(TollGateError::InvalidAdvertisement(
                "No pricing options available".to_string(),
            ));
        }

        for option in &ad.pricing_options {
            if option.asset_type != "cashu" {
                return Err(TollGateError::InvalidAdvertisement(format!(
                    "Unsupported asset type: {}",
                    option.asset_type
                )));
            }

            if option.price_per_step == 0 {
                return Err(TollGateError::InvalidAdvertisement(
                    "Price per step cannot be zero".to_string(),
                ));
            }

            if option.min_steps == 0 {
                return Err(TollGateError::InvalidAdvertisement(
                    "Minimum steps cannot be zero".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Calculate total cost for a purchase
    pub fn calculate_cost(&self, option: &PricingOption, steps: u64) -> u64 {
        steps * option.price_per_step
    }

    /// Calculate allotment for given steps
    pub fn calculate_allotment(&self, steps: u64, step_size: u64) -> u64 {
        steps * step_size
    }
}

impl Default for TollGateProtocol {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_advertisement() {
        let protocol = TollGateProtocol::new();

        let valid_ad = TollGateAdvertisement {
            metric: "milliseconds".to_string(),
            step_size: 1000,
            pricing_options: vec![PricingOption {
                asset_type: "cashu".to_string(),
                price_per_step: 1,
                price_unit: "sat".to_string(),
                mint_url: "https://mint.example.com".to_string(),
                min_steps: 60,
            }],
            tips: vec!["01".to_string()],
            tollgate_pubkey: "test_pubkey".to_string(),
        };

        assert!(protocol.validate_advertisement(&valid_ad).is_ok());
    }

    #[test]
    fn test_calculate_cost() {
        let protocol = TollGateProtocol::new();
        let option = PricingOption {
            asset_type: "cashu".to_string(),
            price_per_step: 5,
            price_unit: "sat".to_string(),
            mint_url: "https://mint.example.com".to_string(),
            min_steps: 10,
        };

        assert_eq!(protocol.calculate_cost(&option, 100), 500);
    }

    #[test]
    fn test_calculate_allotment() {
        let protocol = TollGateProtocol::new();
        assert_eq!(protocol.calculate_allotment(60, 1000), 60000); // 60 seconds
    }
}
