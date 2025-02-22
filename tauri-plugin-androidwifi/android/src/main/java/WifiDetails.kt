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
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject
import java.nio.ByteBuffer
import java.nio.charset.StandardCharsets
import java.util.concurrent.TimeUnit

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
    fun getWifiDetails(context: Context): JSArray {
        val results = startWifiScan(context)
        val resultJson = JSArray()
        for (result in results) {
            val wifi = JSObject()
            wifi.put("ssid", result.SSID.toString() ?: "")
            wifi.put("bssid", result.BSSID.toString() ?: "")
            wifi.put("rssi", result.level.toString())
            wifi.put("capabilities", result.capabilities.toString() ?: "")
            wifi.put("frequency", result.frequency.toString())

            val informationElements = JSArray()

            for (il in result.informationElements) {

                // Convert ByteBuffer into Serializable byte array
                val charBuffer = StandardCharsets.US_ASCII.decode(il.bytes)
                val bytes = JSArray()
                charBuffer.forEach { char -> bytes.put(char.code) }

                informationElements.put(JSObject().apply {
                    put("id", il.id)
                    put("idExt", il.idExt)
                    put("bytes", bytes)
                })
            }
            wifi.put("informationElements", informationElements)
            resultJson.put(wifi)
        }

        return resultJson
    }

    @SuppressLint("NewApi")
    fun getCurrentWifiDetails(context: Context): JSObject {
        val wifiManager = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        val connectionInfo = wifiManager.connectionInfo

        val json = JSObject()
        if(connectionInfo == null){
            return json;
        }

        json.put("ssid", connectionInfo.ssid ?: "")
        json.put("bssid", connectionInfo.bssid ?: "")
        json.put("macAddress", connectionInfo.macAddress ?: "")
        return json
    }

    @SuppressLint("NewApi")
    fun getMacAddress(context: Context, gatewayIp: String): String {
        Logger.info("getMacaddress: gatewayIp ", gatewayIp)
        val url = "http://$gatewayIp:2122/"
        val client = OkHttpClient.Builder()
            .connectTimeout(250, TimeUnit.MILLISECONDS)
            .readTimeout(250, TimeUnit.MILLISECONDS)
            .writeTimeout(250, TimeUnit.MILLISECONDS)
            .build()
        
        val request = Request.Builder()
            .url(url)
            .build()

        return try {
            client.newCall(request).execute().use { response ->
                if (response.isSuccessful) {
                    val responseBody = response.body?.string()
                    if (responseBody != null) {
                        val json = JSONObject(responseBody)
                        if (json.optBoolean("Success", false)) {
                            return json.optString("Mac", "02:00:00:00:00:00")
                        }
                    }
                }
                "02:00:00:00:00:00"
            }
        } catch (e: Exception) {
            e.printStackTrace()
            "02:00:00:00:00:00"
        }
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
}
