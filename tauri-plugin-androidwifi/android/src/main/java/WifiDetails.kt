package com.plugin.androidwifi

import android.annotation.SuppressLint
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.CaptivePortal
import android.net.ConnectivityManager
import android.net.wifi.ScanResult
import android.net.wifi.WifiManager
import android.net.wifi.WifiNetworkSuggestion
import android.os.Build
import androidx.annotation.RequiresApi
import app.tauri.Logger
import kotlinx.serialization.Serializable
import java.net.HttpURLConnection
import java.net.URL
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

    @SuppressLint("NewApi")
    fun getMacAddress(context: Context): String {
        val connectivityManager =
            context.applicationContext.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager

        val networks = connectivityManager.allNetworks;

        networks.forEach { n ->
            var info = connectivityManager.getNetworkInfo(n)

            if (info == null) {
                Logger.error("Could not get network info for network ${n}")
                return "no network info";
            }

            if(info?.type == ConnectivityManager.TYPE_WIFI) {
                Logger.info("Found wifi network, binding process")
                connectivityManager.bindProcessToNetwork(n)

                // TODO: do http call from here, currently doing it from front-end bc of threading stuff here that's to complicated for now.

                return "only rebinded network to WIFI"
            }
        }

        return "something went wrong (android)"
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
                if (!intent.action.equals(WifiManager.ACTION_WIFI_NETWORK_SUGGESTION_POST_CONNECTION)) {
                    return;
                }
                res += intent.toString()

                Logger.info(res)
                // do post connect processing here
            }
        };
        context.registerReceiver(broadcastReceiver, intentFilter);
        return res
    }

    fun toByteArray(buffer: ByteBuffer): ByteArray {
        val byteArray = ByteArray(buffer.capacity())
        buffer.get(byteArray)

        return byteArray
    }

    @RequiresApi(Build.VERSION_CODES.TIRAMISU)
    fun dismissCaptivePortal(intent: Intent) {
        Logger.info("Dismissing captive portal")
        val mCaptivePortal = intent.getParcelableExtra(ConnectivityManager.EXTRA_CAPTIVE_PORTAL, CaptivePortal::class.java)

        if(mCaptivePortal == null) {
            Logger.error("Could not retrieve captive portal object from intent")
        }

        // TODO: Pass on to native app if it's not a Tollgate network
        // It is possible to get info about the network we're connecting to.
        // We can get a parcableExtra from the intent (EXTRA_NETWORK) to determine this.

        mCaptivePortal?.reportCaptivePortalDismissed()
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
