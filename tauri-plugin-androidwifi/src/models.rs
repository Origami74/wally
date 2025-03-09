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
  pub mac_address: String,
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
