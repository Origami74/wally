use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Empty {
    pub value: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WifiDetails {
    pub ssid: String,
    pub bssid: String,
    pub rssi: String,
    pub capabilities: String,
    pub frequency: String,
    pub information_elements: Vec<InformationElement>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InformationElement {
    pub id: i64,
    pub id_ext: i64,
    pub bytes: Vec<u16>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WifiDetailsResponse {
    pub wifis: Option<Vec<WifiDetails>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentWifiResponse {
    pub wifi: Option<CurrentWifi>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentWifi {
    pub ssid: String,
    pub bssid: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MacAddressResponse {
    pub mac_address: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayIpResponse {
    pub gateway_ip: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectWifiPayload {
    pub ssid: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMacAddressPayload {
    pub gateway_ip: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectWifiResponse {
    pub response: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TollgateAdvertisement {
    pub tollgate_pubkey: String,
    pub tips: Vec<String>,
    pub metric: Option<String>,
    pub step_size: Option<String>,
    pub pricing_options: Vec<PricingOption>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PricingOption {
    pub mint_url: String,
    pub price: String,
    pub unit: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TollgateDetectionResponse {
    pub is_tollgate: bool,
    pub advertisement: Option<TollgateAdvertisement>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatusResponse {
    pub gateway_ip: Option<String>,
    pub mac_address: Option<String>,
    pub current_wifi: Option<CurrentWifi>,
    pub is_tollgate: bool,
    pub tollgate_advertisement: Option<TollgateAdvertisement>,
}
