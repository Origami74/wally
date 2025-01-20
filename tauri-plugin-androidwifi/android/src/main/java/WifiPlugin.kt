package com.plugin.androidwifi

import android.app.Activity
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import app.tauri.plugin.Invoke

import android.Manifest
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

@InvokeArg
class Empty {
  var value: String? = null
}

@TauriPlugin
class WifiPlugin(private val activity: Activity): Plugin(activity) {
    private val implementation = WifiDetails()

    @Command
    fun getWifiDetails(invoke: Invoke) {
        val ret = JSObject()
        val json = Json.encodeToString(implementation.getWifiDetails(activity.applicationContext))
        ret.put("wifis", json)
        invoke.resolve(ret)
    }
}
