package com.plugin.androidwifi

import android.annotation.SuppressLint
import android.content.Context
import android.net.wifi.ScanResult
import android.net.wifi.WifiConfiguration
import android.net.wifi.WifiManager
import android.net.wifi.WifiNetworkSuggestion
import app.tauri.Logger
import kotlinx.serialization.Serializable
import java.nio.ByteBuffer


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

    @SuppressLint("NewApi") // TODO: set minimum android api version to 30 (android 11)
    fun getWifiDetails(context: Context): List<WifiNetworkInfo> {
        val results = startWifiScan(context)
        return results.map { result ->
            WifiNetworkInfo(
                ssid = result.SSID ?: "",
                bssid = result.BSSID ?: "",
                rssi = result.level,           // signal strength in dBm
                capabilities = result.capabilities ?: "",
                frequency = result.frequency,
                informationElements = result.informationElements.map { informationElement ->
                    InformationElement(
                        id = informationElement.id,
                        idExt = informationElement.idExt,
                        bytes = toByteArray(informationElement.bytes)
                    )
                }
            )
        }
    }

    // We can only make suggestions to connect to a wifi network.
    // https://developer.android.com/develop/connectivity/wifi/wifi-suggest
    @SuppressLint("NewApi")
    fun connectWifi(context: Context, ssid: String): String {
        // TODO: check permission granted: https://issuetracker.google.com/issues/224071894


        val wifiManager = context.getSystemService(Context.WIFI_SERVICE) as WifiManager

       val suggestion = WifiNetworkSuggestion.Builder()
            .setSsid(ssid)
            .setIsAppInteractionRequired(false) // Optional (Needs location permission)
            .build();

        val status = wifiManager.addNetworkSuggestions(listOf(suggestion));

        if (status != WifiManager.STATUS_NETWORK_SUGGESTIONS_SUCCESS) {
            Logger.error("Could not connect to network: $status")
        }

        return status.toString()
    }

    fun toByteArray(buffer: ByteBuffer): ByteArray {
        val byteArray = ByteArray(buffer.capacity())
        buffer.get(byteArray)

        return byteArray
    }

    @Serializable
    data class WifiNetworkInfo(
        val ssid: String,
        val bssid: String,
        val rssi: Int,
        val capabilities: String,
        val frequency: Int,
        val informationElements: List<InformationElement>
    )

    @Serializable
    data class InformationElement(
        val id: Int,
        val idExt: Int,
        val bytes: ByteArray,
    )
}
