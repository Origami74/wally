package com.plugin.androidwifi

import android.annotation.SuppressLint
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.wifi.ScanResult
import android.net.wifi.WifiConfiguration
import android.net.wifi.WifiManager
import android.net.wifi.WifiNetworkSpecifier
import android.net.wifi.WifiNetworkSuggestion
import android.util.Log
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

    @SuppressLint("NewApi")
    fun getCurrentWifiDetails(context: Context): CurrentNetworkInfo? {
        val wifiManager = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        val connectionInfo = wifiManager.connectionInfo

        if(connectionInfo == null){
            return null;
        }

        return CurrentNetworkInfo(
            ssid = connectionInfo.ssid ?: "",
            bssid = connectionInfo.bssid ?: "",
            macAddress = connectionInfo.macAddress ?: ""
        )
    }

    // We can only make suggestions to connect to a wifi network.
    // https://developer.android.com/develop/connectivity/wifi/wifi-suggest
    @SuppressLint("NewApi")
    fun connectWifi(context: Context, ssid: String): String {
        // TODO: check permission granted: https://issuetracker.google.com/issues/224071894

        Logger.info("Connecting to ssid: $ssid")
        val wifiManager = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager

        val suggestion = WifiNetworkSuggestion.Builder()
            .setSsid(ssid)
            .setPriority(Int.MAX_VALUE)
            .setIsAppInteractionRequired(false) // Optional (Needs location permission)
//            .setIsMetered(false)
            .build();

        wifiManager.removeNetworkSuggestions(listOf(suggestion))
        val status = wifiManager.addNetworkSuggestions(listOf(suggestion));


        if (status != WifiManager.STATUS_NETWORK_SUGGESTIONS_SUCCESS) {
            Logger.error("Could not connect to network: $status")
        }

        val intentFilter = IntentFilter(WifiManager.ACTION_WIFI_NETWORK_SUGGESTION_POST_CONNECTION);

        var res = ""
        val broadcastReceiver = object : BroadcastReceiver() {
            override fun onReceive(context: Context, intent: Intent) {
                Log.i("connectWifi-res", "hoi")
                if (!intent.action.equals(WifiManager.ACTION_WIFI_NETWORK_SUGGESTION_POST_CONNECTION)) {
                    return;
                }
                res += intent.toString()

                Logger.info(res)
                // do post connect processing here
            }
        };
        context.registerReceiver(broadcastReceiver, intentFilter);

        res += "- status" + status.toString()
        Logger.info(res)
//        return res
//
//        // start experiment
//        val connectivityManager =
//            context.applicationContext.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
//
//        val networks = connectivityManager.allNetworks;
//
//        networks.forEach { n ->
//            var info = connectivityManager.getNetworkInfo(n)
//
//            if (info != null) {
//                Logger.error("Could not get network info for network ${n}")
//                return res;
//            }
//            Logger.info("network (${n}): ${info.toString()}")
//            if(info.type == ConnectivityManager.TYPE_WIFI) {
//                connectivityManager.bindProcessToNetwork(n)
//            }
//
//
//            }

//        connectivityManager.bindProcessToNetwork()

        return res
//        var wifiNetworkSpecifier =
//            WifiNetworkSpecifier.Builder().setSsid(ssid).build()
//
//        val networkRequest = NetworkRequest.Builder()
//            .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
//            .setNetworkSpecifier(wifiNetworkSpecifier)
//            .build()
//
//        val networkCallback = object : ConnectivityManager.NetworkCallback() {
//            override fun onUnavailable() {
//                super.onUnavailable()
//            }
//
//            override fun onLosing(network: Network, maxMsToLive: Int) {
//                super.onLosing(network, maxMsToLive)
//
//            }
//
//            override fun onAvailable(network: Network) {
//                super.onAvailable(network)
//                connectivityManager?.bindProcessToNetwork(network)
//            }
//
//            override fun onLost(network: Network) {
//                super.onLost(network)
//
//            }
//        }
//        connectivityManager?.requestNetwork(networkRequest, networkCallback)
//        return "ok"
//        // end experiment
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

    @Serializable
    data class CurrentNetworkInfo(
        val ssid: String,
        val bssid: String,
        val macAddress: String
    )
}
