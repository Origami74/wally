package com.plugin.androidwifi

import android.util.Log

import android.net.wifi.WifiManager 
import android.content.Context
import android.net.wifi.WifiInfo
import android.net.wifi.ScanResult
import kotlinx.serialization.Serializable

class WifiDetails {
    fun startWifiScan(context: Context): List<ScanResult> {
        val wifiManager = context.getSystemService(Context.WIFI_SERVICE) as WifiManager

        // Initiate scan (results won't be immediately available).
        // On modern Android, you may need to register a BroadcastReceiver to be
        // notified when the scan is complete. For simplicity, we assume we can just call
        // wifiManager.scanResults after a short delay or once the system has completed scanning.
        wifiManager.startScan()

        // Retrieve the last known scan results
        val scanResults = wifiManager.scanResults
        return scanResults
    }

    fun getWifiDetails(context: Context): List<WifiNetworkInfo> {
        val results = startWifiScan(context)
        return results.map { result ->
            WifiNetworkInfo(
                ssid = result.SSID ?: "",
                bssid = result.BSSID ?: "",
                rssi = result.level,           // signal strength in dBm
                capabilities = result.capabilities ?: "",
                frequency = result.frequency   // channel frequency in MHz
            )
        }
    }

    @Serializable
    data class WifiNetworkInfo(
        val ssid: String,
        val bssid: String,
        val rssi: Int,
        val capabilities: String,
        val frequency: Int
    )
}
