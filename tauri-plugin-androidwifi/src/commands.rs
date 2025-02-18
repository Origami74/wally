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
pub(crate) async fn get_mac_address<R: Runtime>(app: AppHandle<R>, payload: Empty) -> crate::Result<MacAddressResponse> {
    app.androidwifi().get_mac_address(payload)
}
