import type { ConnectedNetworkInfo } from "$lib/tollgate/types/ConnectedNetworkInfo";
import type {NetworkInfo} from "$lib/tollgate/types/NetworkInfo";
import {invoke} from "@tauri-apps/api/core";
import {fetch} from "@tauri-apps/plugin-http";

export async function getMacAddress(gatewayIp: string | undefined): Promise<string | undefined> {
    if(gatewayIp === undefined){
        return undefined;
    }

    try{
        const macAddressResult = await invoke("plugin:androidwifi|get_mac_address", { payload: { gatewayIp: gatewayIp } });

        console.log("macAddress", macAddressResult);

        return macAddressResult.macAddress
    } catch (e) {
        throw new Error(`Failed to determine MAC address, reason: ${e}`);
    }
}

export async function getCurrentNetwork(): Promise<ConnectedNetworkInfo> {
    const currentNetworkInfo = await invoke("plugin:androidwifi|get_current_wifi_details", { payload: { value: "" } })
    const details = currentNetworkInfo.wifi;

    return details
}

export async function getAvailableNetworks(): Promise<NetworkInfo[]> {
    try{
        let response = await invoke("plugin:androidwifi|get_wifi_details", { payload: { value: "" } });

        const networks: NetworkInfo[] = response.wifis;

        return networks;
    } catch (e) {
        console.error(`Failed to perform network scan, reason: ${e}`)
        return [];
    }
}

export async function connectNetwork(ssid: string): Promise<void> {
    let response = await invoke("plugin:androidwifi|connect_wifi", {payload: { ssid: ssid } });
    console.log("response for connecting to " + ssid + " = " + JSON.stringify(response));
}