use anyhow::Result;
use chrono::{DateTime, Utc};
use nostr_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

const PROVIDER_ANNOUNCEMENT_KIND: u16 = 38421;

const DEFAULT_RELAYS: &[&str] = &[
    "wss://relay.damus.io",
    "wss://relay.snort.social",
    "wss://nos.lol",
    "wss://relay.nostr.band",
    "wss://nostr.wine",
    "wss://relay.primal.net",
    "wss://relay.routstr.com",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderContent {
    pub name: String,
    pub about: String,
    pub urls: Option<Vec<String>>,
    pub mints: Option<Vec<String>>,
    pub version: Option<String>,
    pub use_onion: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NostrProvider {
    pub id: String,
    pub pubkey: String,
    pub name: String,
    pub about: String,
    pub urls: Vec<String>,
    pub mints: Vec<String>,
    pub version: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub followers: i32,
    pub zaps: i32,
    pub use_onion: bool,
    pub is_online: bool,
    pub is_official: bool,
}

pub struct NostrProviderDiscovery {
    relays: Vec<String>,
    client: Client,
    http_client: reqwest::Client,
}

fn parse_tags_to_map(tags: &Tags) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();

    for tag in tags.iter() {
        let tag_str = format!("{:?}", tag);
        if let Some(start) = tag_str.find('[') {
            if let Some(end) = tag_str.find(']') {
                let inner = &tag_str[start + 1..end];
                let parts: Vec<&str> = inner
                    .split(',')
                    .map(|s| s.trim().trim_matches('"'))
                    .collect();

                if parts.len() >= 2 {
                    let key = parts[0].to_string();
                    let value = parts[1].to_string();
                    map.entry(key).or_default().push(value);
                }
            }
        }
    }

    map
}

impl NostrProviderDiscovery {
    pub async fn new() -> Result<Self> {
        let relays: Vec<String> = DEFAULT_RELAYS.iter().map(|&s| s.to_string()).collect();
        let client = Client::default();
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        for relay in relays.iter() {
            if let Err(e) = client.add_relay(relay).await {
                log::warn!("Failed to add relay {}: {}", relay, e);
            }
        }

        Ok(Self {
            relays,
            client,
            http_client,
        })
    }

    pub async fn discover_providers(&self) -> Result<Vec<NostrProvider>> {
        log::info!("Starting provider discovery from Nostr relays...");

        self.client.connect().await;
        tokio::time::sleep(Duration::from_secs(2)).await;

        let filter = Filter::new()
            .kind(Kind::Custom(PROVIDER_ANNOUNCEMENT_KIND))
            .limit(100);

        log::info!(
            "Fetching provider events from {} relays...",
            self.relays.len()
        );

        let events = self
            .client
            .fetch_events(filter, Duration::from_secs(10))
            .await?;

        log::info!("Retrieved {} provider events", events.len());

        let mut providers = Vec::new();

        for event in events {
            match self.parse_provider_from_event(&event).await {
                Ok(provider) => {
                    if provider.is_online {
                        log::debug!("Successfully parsed online provider: {}", provider.name);
                        providers.push(provider);
                    }
                }
                Err(e) => {
                    log::debug!("Failed to parse provider event {}: {}", event.id, e);
                    continue;
                }
            }
        }

        log::info!("Discovered {} valid providers", providers.len());
        Ok(providers)
    }

    async fn parse_provider_from_event(&self, event: &Event) -> Result<NostrProvider> {
        let content: ProviderContent = serde_json::from_str(&event.content)?;

        let mut urls = content.urls.unwrap_or_default();
        let mut mints = content.mints.unwrap_or_default();
        let mut version = content.version;
        let mut use_onion = content.use_onion.unwrap_or(false);

        let tag_map = parse_tags_to_map(&event.tags);

        if let Some(tag_urls) = tag_map.get("u") {
            for url in tag_urls {
                urls.push(url.clone());
                if url.contains(".onion") {
                    use_onion = true;
                }
            }
        }

        if let Some(tag_mints) = tag_map.get("mint") {
            for mint in tag_mints {
                mints.push(mint.clone());
            }
        }

        if let Some(tag_version) = tag_map.get("version") {
            if let Some(v) = tag_version.first() {
                version = Some(v.clone());
            }
        }

        if urls.is_empty() {
            return Err(anyhow::anyhow!("Provider must have at least one URL"));
        }

        if !use_onion {
            use_onion = urls.iter().any(|url| url.contains(".onion"));
        }

        let is_online = self.check_provider_online(&urls[0]).await;
        let is_official = urls.iter().any(|url| url.contains("api.routstr.com"));

        Ok(NostrProvider {
            id: event.id.to_hex(),
            pubkey: event.pubkey.to_hex(),
            name: content.name,
            about: content.about,
            urls,
            mints,
            version,
            created_at: DateTime::from_timestamp(event.created_at.as_u64() as i64, 0)
                .unwrap_or_else(Utc::now),
            updated_at: Utc::now(),
            followers: 0,
            zaps: 0,
            use_onion,
            is_online,
            is_official,
        })
    }

    fn is_valid_public_url(&self, url: &str) -> bool {
        if url.ends_with(".onion") || url.contains("localhost") || url.contains("127.0.0.1") {
            return false;
        }

        if url.contains("192.168.") || url.contains("10.") || url.contains("172.16.") {
            return false;
        }

        if url.contains(":3000") || url.contains(":8000") || url.contains(":8080") {
            return false;
        }

        if url.contains(".") && !url.starts_with("http://192.") && !url.starts_with("https://192.")
        {
            return true;
        }

        false
    }

    async fn check_provider_online(&self, url: &str) -> bool {
        if !self.is_valid_public_url(url) {
            return false;
        }

        let formatted_url = if url.starts_with("http://") || url.starts_with("https://") {
            url.trim_end_matches('/').to_string()
        } else {
            format!("https://{}", url.trim_end_matches('/'))
        };

        let info_url = format!("{}/v1/info", formatted_url);

        match self.http_client.get(&info_url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }
}

pub async fn discover_providers() -> Result<Vec<NostrProvider>> {
    let discovery = NostrProviderDiscovery::new().await?;
    discovery.discover_providers().await
}
