import type { ConnectedNetworkInfo } from "$lib/tollgate/types/ConnectedNetworkInfo";
import type {NetworkInfo} from "$lib/tollgate/types/NetworkInfo";
import {invoke} from "@tauri-apps/api/core";
import {fetch} from "@tauri-apps/plugin-http";

export async function getMacAddress(gatewayIp: string | undefined): Promise<string | undefined> {
    if(gatewayIp === undefined){
        return undefined;
    }

    // Step 1, rebind the app to the wifi network, so the whoami request won't be sent over any active cellular connection
    try{
        await invoke("plugin:androidwifi|get_mac_address", { }); // only rebinds
    } catch (e) {
        console.error("could not get mac native", e);
    }

    // Step 2, call the whoami service
    const whoamiResponse = await fetch(`http://${gatewayIp}:2122/`, {connectTimeout: 350}).catch((reason) => {
        throw new Error(`Failed to determine MAC address, reason: ${reason}`);
    }) // Universal endpoint for whoami

    let whoami = await whoamiResponse.json();

    if(whoami.Success === false) {
        console.error(`Failed to determine MAC address, reason: ${whoami.ErrorMessage}`)
        return undefined;
    }

    return whoami.Mac
}

export async function getCurrentNetwork(): Promise<ConnectedNetworkInfo> {
    const currentNetworkInfo = await invoke("plugin:androidwifi|get_current_wifi_details", { payload: { value: "" } })
    const details = currentNetworkInfo.wifi;

    return details
}

export async function getAvailableNetworks(): Promise<NetworkInfo[]> {
    let response = await invoke("plugin:androidwifi|get_wifi_details", { payload: { value: "" } });
    const networks: NetworkInfo[] = response.wifis;

    return networks;
}

export async function connectNetwork(ssid: string): Promise<void> {
    let response = await invoke("plugin:androidwifi|connect_wifi", { ssid: ssid });
    console.log("response for connecting to " + ssid + " = " + JSON.stringify(response));
}