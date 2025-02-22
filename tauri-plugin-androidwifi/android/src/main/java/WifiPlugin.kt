package com.plugin.androidwifi

import android.app.Activity
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import app.tauri.plugin.Invoke

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.CaptivePortal
import android.net.ConnectivityManager
import android.net.Network
import android.util.Log
import android.webkit.WebView
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import app.tauri.Logger
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.encodeToJsonElement

@InvokeArg
class Empty {
  var value: String? = null
}

@TauriPlugin
class WifiPlugin(private val activity: Activity): Plugin(activity) {
    private val implementation = WifiDetails()

    override fun onNewIntent(intent: Intent) {
        Logger.info("intent: ${intent.action.toString()}")
        if (intent.action == "android.net.conn.CAPTIVE_PORTAL") {
            implementation.dismissCaptivePortal(intent)
        }
    }

    @Command
    fun getWifiDetails(invoke: Invoke) {
        val ret = JSObject()
        ret.put("wifis", implementation.getWifiDetails(activity.applicationContext))
        invoke.resolve(ret)
    }

    @Command
    fun connectWifi(invoke: Invoke) {
        val ssid = invoke.getArgs().get("ssid").toString()
        val ret = JSObject()
        ret.put("response", implementation.connectWifi(activity.applicationContext, ssid))
        invoke.resolve(ret)
    }

    @Command
    fun getMacAddress(invoke: Invoke) {
        val gatewayIp = invoke.getArgs().get("gatewayIp").toString()
        val ret = JSObject()
        ret.put("macAddress", implementation.getMacAddress(activity.applicationContext, gatewayIp))
        invoke.resolve(ret)
    }

    @Command
    fun getCurrentWifiDetails(invoke: Invoke) {
        val ret = JSObject()
        ret.put("wifi", implementation.getCurrentWifiDetails(activity.applicationContext))
        invoke.resolve(ret)
    }

    private val REQUEST_CODE_WIFI_PERMISSIONS = 1001

    private val requiredPermissions = arrayOf(
        Manifest.permission.ACCESS_WIFI_STATE,
        Manifest.permission.CHANGE_WIFI_STATE,
        Manifest.permission.ACCESS_FINE_LOCATION
    )

    override fun load(webView: WebView) {
        super.load(webView)
        // Find any permissions not yet granted.
        val notGrantedPermissions = requiredPermissions.filter {
            ContextCompat.checkSelfPermission(activity.applicationContext, it) != PackageManager.PERMISSION_GRANTED
        }

        // If there are any missing permissions, request them at runtime.
        if (notGrantedPermissions.isNotEmpty()) {
            ActivityCompat.requestPermissions(
                activity,
                notGrantedPermissions.toTypedArray(),
                REQUEST_CODE_WIFI_PERMISSIONS
            )
        } else {
            // Permissions already granted. You can safely use the Wi-Fi APIs.
        }
    }
}
