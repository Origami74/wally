use tauri::{AppHandle, command, Runtime};

use crate::models::*;
use crate::Result;
use crate::AndroidwifiExt;

#[command]
pub(crate) async fn ping<R: Runtime>(
    app: AppHandle<R>,
    payload: Empty,
) -> Result<PingResponse> {
    app.androidwifi().get_wifi_details(payload)
}
