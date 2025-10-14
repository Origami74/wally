use reqwest::Client;
use std::time::Instant;

pub fn is_onion_url(url: &str) -> bool {
    url.contains(".onion")
}

pub fn needs_tor_proxy(url: &str, use_onion: bool) -> bool {
    use_onion && is_onion_url(url)
}

pub fn construct_url_with_protocol(base_url: &str, path: &str) -> String {
    let formatted_base = if base_url.starts_with("http://") || base_url.starts_with("https://") {
        base_url.to_string()
    } else if base_url.contains(".onion") {
        format!("http://{}", base_url)
    } else {
        format!("https://{}", base_url)
    };

    format!("{}/{}", formatted_base, path)
}

pub fn configure_tor_proxy_url(endpoint_url: &str) -> String {
    let tor_proxy_url =
        std::env::var("TOR_SOCKS_PROXY").unwrap_or_else(|_| "socks5h://127.0.0.1:9050".to_string());

    if endpoint_url.contains(".onion") && tor_proxy_url.starts_with("socks5://") {
        tor_proxy_url.replace("socks5://", "socks5h://")
    } else if endpoint_url.contains(".onion") && !tor_proxy_url.contains("socks5h://") {
        format!(
            "socks5h://{}",
            tor_proxy_url.trim_start_matches("socks5://")
        )
    } else {
        tor_proxy_url
    }
}

pub fn configure_client_with_tor_proxy(
    mut client_builder: reqwest::ClientBuilder,
    endpoint_url: &str,
    use_onion: bool,
) -> Result<reqwest::ClientBuilder, String> {
    if needs_tor_proxy(endpoint_url, use_onion) {
        let proxy_url = configure_tor_proxy_url(endpoint_url);

        log::info!("Using Tor proxy URL: {}", proxy_url);

        match reqwest::Proxy::all(&proxy_url) {
            Ok(proxy) => {
                client_builder = client_builder.proxy(proxy);
                log::info!(
                    "Using Tor proxy for .onion request: {} (proxy: {})",
                    endpoint_url,
                    proxy_url
                );
                Ok(client_builder)
            }
            Err(e) => Err(format!(
                "Failed to configure Tor proxy for .onion request: {}",
                e
            )),
        }
    } else {
        Ok(client_builder)
    }
}

pub fn create_onion_client(
    endpoint_url: &str,
    use_onion: bool,
    timeout_secs: Option<u64>,
) -> Result<Client, String> {
    let mut client_builder = Client::builder();

    if let Some(timeout) = timeout_secs {
        client_builder = client_builder.timeout(std::time::Duration::from_secs(timeout));
    }

    client_builder = configure_client_with_tor_proxy(client_builder, endpoint_url, use_onion)?;

    client_builder
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))
}

pub fn start_onion_timing(endpoint_url: &str) -> Option<Instant> {
    if is_onion_url(endpoint_url) {
        Some(Instant::now())
    } else {
        None
    }
}

pub fn log_onion_timing(start_time: Option<Instant>, endpoint_url: &str, context: &str) {
    if let Some(start) = start_time {
        let duration = start.elapsed();
        log::info!(
            "Onion request timing for {}: {} completed in {:?}",
            context,
            endpoint_url,
            duration
        );
    }
}

pub fn get_onion_error_message(
    error: &reqwest::Error,
    endpoint_url: &str,
    context: &str,
) -> String {
    if is_onion_url(endpoint_url) {
        if error.is_timeout() {
            format!(
                "{}: Timeout connecting to .onion service (check Tor proxy)",
                context
            )
        } else if error.is_connect() {
            format!(
                "{}: Failed to connect to .onion service (check Tor proxy)",
                context
            )
        } else {
            format!("{}: .onion service error: {}", context, error)
        }
    } else {
        format!("{}: Request error: {}", context, error)
    }
}
