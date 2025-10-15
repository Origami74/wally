use crate::models::*;
use reqwest;
use serde_json;
use std::time::Duration;

pub struct TollgateDetector;

impl TollgateDetector {
    pub fn new() -> Self {
        Self
    }

    pub async fn check_tollgate_at_gateway(
        &self,
        gateway_ip: &str,
    ) -> Result<TollgateDetectionResponse, Box<dyn std::error::Error>> {
        let url = format!("http://{}:2121/", gateway_ip);
        println!("[Tollgate Debug] Checking tollgate at URL: {}", url);

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;

        match client.get(&url).send().await {
            Ok(response) => {
                println!(
                    "[Tollgate Debug] HTTP response status: {}",
                    response.status()
                );
                if response.status().is_success() {
                    let text: String = response.text().await?;
                    println!("[Tollgate Debug] HTTP response body: {}", text);

                    if let Ok(advertisement_json) = serde_json::from_str::<serde_json::Value>(&text)
                    {
                        println!("[Tollgate Debug] Successfully parsed JSON advertisement");
                        let advertisement =
                            self.parse_tollgate_advertisement(&advertisement_json)?;
                        println!("[Tollgate Debug] Successfully parsed tollgate advertisement");

                        Ok(TollgateDetectionResponse {
                            is_tollgate: true,
                            advertisement: Some(advertisement),
                        })
                    } else {
                        println!(
                            "[Tollgate Debug] Failed to parse response as JSON: {}",
                            text
                        );
                        Ok(TollgateDetectionResponse {
                            is_tollgate: false,
                            advertisement: None,
                        })
                    }
                } else {
                    println!(
                        "[Tollgate Debug] HTTP request failed with status: {}",
                        response.status()
                    );
                    Ok(TollgateDetectionResponse {
                        is_tollgate: false,
                        advertisement: None,
                    })
                }
            }
            Err(e) => {
                println!("[Tollgate Debug] HTTP request error: {:?}", e);
                Ok(TollgateDetectionResponse {
                    is_tollgate: false,
                    advertisement: None,
                })
            }
        }
    }

    pub fn parse_tollgate_advertisement(
        &self,
        json: &serde_json::Value,
    ) -> Result<TollgateAdvertisement, Box<dyn std::error::Error>> {
        // Parse according to TIP-01 specification
        println!("[Tollgate Debug] Parsing advertisement JSON...");
        let kind = json.get("kind").and_then(|k| k.as_u64()).unwrap_or(0);
        println!("[Tollgate Debug] Event kind: {}", kind);

        if kind != 10021 {
            return Err("Not a tollgate advertisement (wrong kind)".into());
        }

        let pubkey = json
            .get("pubkey")
            .and_then(|p| p.as_str())
            .unwrap_or("")
            .to_string();
        println!("[Tollgate Debug] Pubkey: {}", pubkey);

        let empty_vec = vec![];
        let tags = json
            .get("tags")
            .and_then(|t| t.as_array())
            .unwrap_or(&empty_vec);
        println!("[Tollgate Debug] Found {} tags", tags.len());

        let mut tips = Vec::new();
        let mut metric = None;
        let mut step_size = None;
        let mut pricing_options = Vec::new();

        for (i, tag) in tags.iter().enumerate() {
            if let Some(tag_array) = tag.as_array() {
                if let Some(tag_name) = tag_array.get(0).and_then(|t| t.as_str()) {
                    println!(
                        "[Tollgate Debug] Tag {}: {} with {} elements",
                        i,
                        tag_name,
                        tag_array.len()
                    );
                    match tag_name {
                        "tips" => {
                            tips = tag_array
                                .iter()
                                .skip(1)
                                .filter_map(|v| v.as_str())
                                .map(|s| s.to_string())
                                .collect();
                            println!("[Tollgate Debug] Parsed tips: {:?}", tips);
                        }
                        "metric" => {
                            metric = tag_array
                                .get(1)
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            println!("[Tollgate Debug] Parsed metric: {:?}", metric);
                        }
                        "step_size" => {
                            step_size = tag_array
                                .get(1)
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            println!("[Tollgate Debug] Parsed step_size: {:?}", step_size);
                        }
                        "price_per_step" => {
                            // Parse pricing option: ["price_per_step", "<bearer_asset_type>", "<price>", "<unit>", "<mint_url>", "<min_steps>"]
                            if tag_array.len() >= 5 {
                                let mint_url = tag_array
                                    .get(4)
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let price = tag_array
                                    .get(2)
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let unit = tag_array
                                    .get(3)
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                println!("[Tollgate Debug] Parsed pricing option: price={}, unit={}, mint_url={}", price, unit, mint_url);

                                pricing_options.push(PricingOption {
                                    mint_url,
                                    price,
                                    unit,
                                });
                            }
                        }
                        _ => {
                            println!("[Tollgate Debug] Unknown tag: {}", tag_name);
                        }
                    }
                }
            }
        }

        let advertisement = TollgateAdvertisement {
            tollgate_pubkey: pubkey,
            tips,
            metric,
            step_size,
            pricing_options,
        };

        println!("[Tollgate Debug] Final advertisement: pubkey={}, tips={:?}, metric={:?}, step_size={:?}, pricing_count={}",
                 advertisement.tollgate_pubkey, advertisement.tips, advertisement.metric, advertisement.step_size, advertisement.pricing_options.len());

        Ok(advertisement)
    }
}
