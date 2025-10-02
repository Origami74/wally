use tauri::{AppHandle, command, Runtime};

use crate::models::*;
use crate::Result;
use crate::AndroidwifiExt;

#[command]
pub(crate) async fn get_wifi_details<R: Runtime>(
    app: AppHandle<R>,
    payload: Empty,
) -> Result<WifiDetailsResponse> {
    app.androidwifi().get_wifi_details(payload)
}

#[command]
pub(crate) async fn connect_wifi<R: Runtime>(
    app: AppHandle<R>,
    payload: ConnectWifiPayload,
) -> Result<ConnectWifiResponse> {
    app.androidwifi().connect_wifi(payload)
}

#[command]
pub(crate) async fn get_current_wifi_details<R: Runtime>(app: AppHandle<R>, payload: Empty) -> crate::Result<CurrentWifiResponse> {
    app.androidwifi().get_current_wifi_details(payload)
}

#[command]
pub(crate) async fn get_mac_address<R: Runtime>(app: AppHandle<R>, payload: GetMacAddressPayload) -> crate::Result<MacAddressResponse> {
    app.androidwifi().get_mac_address(payload)
}

#[command]
pub(crate) async fn get_gateway_ip<R: Runtime>(app: AppHandle<R>, payload: Empty) -> crate::Result<GatewayIpResponse> {
    app.androidwifi().get_gateway_ip(payload)
}

#[command]
pub(crate) async fn detect_tollgate<R: Runtime>(app: AppHandle<R>, payload: Empty) -> crate::Result<TollgateDetectionResponse> {
    app.androidwifi().detect_tollgate(payload)
}

#[command]
pub(crate) async fn get_network_status<R: Runtime>(app: AppHandle<R>, payload: Empty) -> crate::Result<NetworkStatusResponse> {
    app.androidwifi().get_network_status(payload).await
}
