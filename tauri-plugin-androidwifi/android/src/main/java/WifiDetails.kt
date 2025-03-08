package com.plugin.androidwifi

import android.annotation.SuppressLint
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Context.CONNECTIVITY_SERVICE
import android.content.Intent
import android.content.IntentFilter
import android.net.CaptivePortal
import android.net.ConnectivityManager
import android.net.LinkProperties
import android.net.Network
import android.net.wifi.ScanResult
import android.net.wifi.WifiManager
import android.net.wifi.WifiNetworkSuggestion
import android.os.Build
import android.os.RemoteException
import androidx.annotation.RequiresApi
import app.tauri.Logger
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject
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
        return json
    }

    @SuppressLint("NewApi")
    fun getMacAddress(context: Context, gatewayIp: String): String? {
        bindToWifiNetwork(context)

        Logger.info("getMacaddress: gatewayIp ", gatewayIp)

        val client = OkHttpClient.Builder()
            .connectTimeout(250, TimeUnit.MILLISECONDS)
            .readTimeout(250, TimeUnit.MILLISECONDS)
            .writeTimeout(250, TimeUnit.MILLISECONDS)
            .build()

        val request = Request.Builder()
            .url("http:${gatewayIp}:2122")
            .build()

        try {
            client.newCall(request).execute().use { response ->
                if (response.isSuccessful) {
                    val responseBody = response.body?.string()
                    if (responseBody != null) {
                        val responseJson = JSONObject(responseBody)
                        if (responseJson.optBoolean("Success", false)) {
                            val macAddress = responseJson.optString("Mac")

                            if(macAddress == ""){
                                return null
                            }

                            return macAddress
                        }
                    }
                }
            }
        } catch (e: Exception) {
            e.printStackTrace()
        }

        return null
    }

    private fun isWifiNetwork(context: Context, network: Network): Boolean {
        val connectivityManager =
            context.applicationContext.getSystemService(CONNECTIVITY_SERVICE) as ConnectivityManager
        val info = connectivityManager.getNetworkInfo(network)

        if (info == null) {
            Logger.error("Could not get network info for network ${network}")
            return false;
        }

        return info?.type == ConnectivityManager.TYPE_WIFI
    }

    private fun bindToWifiNetwork(context: Context): Network? {
        val connectivityManager =
            context.applicationContext.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager

        val networks = connectivityManager.allNetworks;

        networks.forEach { network ->
            if (isWifiNetwork(context, network)) {
                Logger.info("Binding application process to wifi network")
                connectivityManager.bindProcessToNetwork(network)
                return network
            }
        }

        return null
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

    var mCaptivePortal: CaptivePortal? = null;
    private val lock = Any()

    @RequiresApi(Build.VERSION_CODES.TIRAMISU)
    fun handleCaptivePortalIntent(context: Context, intent: Intent): String? {

        Logger.info("Handling captive portal")
        val network = intent.getParcelableExtra(ConnectivityManager.EXTRA_NETWORK, Network::class.java)

        synchronized (lock) {
            mCaptivePortal = intent.getParcelableExtra(ConnectivityManager.EXTRA_CAPTIVE_PORTAL, CaptivePortal::class.java)

            Logger.error("captive network: ${network}")
            if(mCaptivePortal == null) {
                Logger.error("Could not retrieve captive portal object from intent")
                return null;
            }
        }


        try {
            // TODO: Pass on to native app if it's not a Tollgate network
            // It is possible to get info about the network we're connecting to.
            // We can get a parcableExtra from the intent (EXTRA_NETWORK) to determine this.
//            mCaptivePortal?.reportCaptivePortalDismissed()
            return getGatewayIp(context)
        } catch (exc: RemoteException) {
            throw RuntimeException(exc)
        }
    }

    @RequiresApi(Build.VERSION_CODES.Q)
    private fun getGatewayIp(context: Context): String? {
        val network = bindToWifiNetwork(context)
        val connectivityManager =
            context.applicationContext.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val linkProperties: LinkProperties? =
            connectivityManager.getLinkProperties(network)

        if(linkProperties == null){
            Logger.error("No linkProperties!")
            return null
        }

        for (routeInfo in linkProperties.routes) {
            if (routeInfo.isDefaultRoute && routeInfo.hasGateway()) {
                val gatewayIp = routeInfo.gateway?.hostAddress
                Logger.info("gatewayIp: ${gatewayIp}")
                if(isValidIPv4Address(gatewayIp)){
                    return gatewayIp
                }
            }
        }

        return null
    }

    @RequiresApi(Build.VERSION_CODES.N)
    fun setupListeners(context: Context, triggerEvent: (String, JSObject) -> Unit) {
        val connectivityManager: ConnectivityManager = context.getSystemService(CONNECTIVITY_SERVICE) as ConnectivityManager

        val networkCallback = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                if(isWifiNetwork(context, network)){
                    val gatewayIp = getGatewayIp(context)
                    Logger.error("CONNECTED TO NETWORK: ${network}, gateway = ${gatewayIp}")

                    val data = JSObject()
                    data.put("gatewayIp", gatewayIp)
                    triggerEvent("network-connected", data)
                }
            }

            override fun onLost(network: Network) {
                Logger.Companion.error("!! DISCONNECTED FROM NETWORK: ${network}")
                triggerEvent("network-disconnected", JSObject())
            }
        }

        connectivityManager.registerDefaultNetworkCallback(networkCallback)
    }

    @RequiresApi(Build.VERSION_CODES.M)
    fun markCaptivePortalDismissed() {

        synchronized (lock) {
            try{
                mCaptivePortal?.reportCaptivePortalDismissed()
            } catch (error: Exception){
                Logger.error("Error while marking captive portal dismissed: ${mCaptivePortal}")
            }

            mCaptivePortal = null
        }
    }
}

fun isValidIPv4Address(ip: String?): Boolean {
    if (ip == null) return false

    // Regular expression to match valid IPv4 address
    val ipv4Pattern = Regex("""^(\d{1,3}\.){3}\d{1,3}$""")

    // First, check if it matches the basic pattern
    if (!ipv4Pattern.matches(ip)) return false

    // Further validate each octet should be in the range of 0 to 255
    return ip.split(".").all {
        it.toIntOrNull()?.let { num ->
            num in 0..255
        } == true
    }
}