use serde::de::DeserializeOwned;
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::models::*;

pub fn init<R: Runtime, C: DeserializeOwned>(
  app: &AppHandle<R>,
  _api: PluginApi<R, C>,
) -> crate::Result<Androidwifi<R>> {
  Ok(Androidwifi(app.clone()))
}

/// Access to the androidwifi APIs.
pub struct Androidwifi<R: Runtime>(AppHandle<R>);

impl<R: Runtime> Androidwifi<R> {
  pub fn ping(&self, payload: Empty) -> crate::Result<PingResponse> {
    Ok(PingResponse {
      value: payload.value,
    })
  }
}
