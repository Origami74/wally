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
import android.os.Build
import android.webkit.WebView
import androidx.annotation.RequiresApi
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

@InvokeArg
class Empty {
  var value: String? = null
}

private val backgroundScope = CoroutineScope(Dispatchers.Default + SupervisorJob())

@RequiresApi(Build.VERSION_CODES.N)
@TauriPlugin
class WifiPlugin(private val activity: Activity): Plugin(activity) {
    private val implementation = WifiDetails()

    init {
        backgroundScope.launch {
            implementation.setupListeners(activity.applicationContext, ::triggerEvent)
        }
    }

    fun triggerEvent(eventName: String, data: JSObject) {
        trigger(eventName, data)
    }


    override fun onNewIntent(intent: Intent) {
        if (intent.action == "android.net.conn.CAPTIVE_PORTAL") {

            backgroundScope.launch {
                val gatewayIp = implementation.handleCaptivePortalIntent(activity.applicationContext, intent)
                val data = JSObject()
                data.put("gatewayIp", gatewayIp)
                trigger("network-connected", data)
            }
        }
    }

    @Command
    fun getWifiDetails(invoke: Invoke) {
        backgroundScope.launch {
            getWifiDetailsInner(invoke)
        }
    }

    fun getWifiDetailsInner(invoke: Invoke) {
        val ret = JSObject()
        ret.put("wifis", implementation.getWifiDetails(activity.applicationContext))
        invoke.resolve(ret)
    }

    @Command
    fun connectWifi(invoke: Invoke) {
        backgroundScope.launch {
            connectWifiInner(invoke)
        }
    }

    private suspend fun connectWifiInner(invoke: Invoke) {
        val ssid = invoke.getArgs().get("ssid").toString()
        val ret = JSObject()
        ret.put("response", implementation.connectWifi(activity.applicationContext, ssid))
        invoke.resolve(ret)
    }

    @Command
    fun getMacAddress(invoke: Invoke) {
        backgroundScope.launch {
            getMacAddressInner(invoke)
        }
    }

    private suspend fun getMacAddressInner(invoke: Invoke) {
        val gatewayIp = invoke.getArgs().get("gatewayIp").toString()
        val ret = JSObject()
        val macAddress = implementation.getMacAddress(activity.applicationContext, gatewayIp)
        ret.put("macAddress", macAddress)

        invoke.resolve(ret)
    }

    @Command
    fun getCurrentWifiDetails(invoke: Invoke) {
        backgroundScope.launch {
            getCurrentWifiDetailsInner(invoke)
        }
    }

    private suspend fun getCurrentWifiDetailsInner(invoke: Invoke) {
        val ret = JSObject()
        ret.put("wifi", implementation.getCurrentWifiDetails(activity.applicationContext))
        invoke.resolve(ret)
    }

    private val REQUEST_CODE_WIFI_PERMISSIONS = 1001

    private val requiredPermissions = arrayOf(
        Manifest.permission.ACCESS_WIFI_STATE,
        Manifest.permission.CHANGE_WIFI_STATE,
        Manifest.permission.ACCESS_FINE_LOCATION,
        Manifest.permission.NEARBY_WIFI_DEVICES
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
