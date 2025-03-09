import type { ConnectedNetworkInfo } from "$lib/tollgate/types/ConnectedNetworkInfo";
import type {NetworkInfo} from "$lib/tollgate/types/NetworkInfo";
import {Channel, invoke, PluginListener} from "@tauri-apps/api/core";

export async function registerListener(eventName: string, onEvent: (data: unknown) => void): Promise<void> {
    const handler = new Channel();
    handler.onmessage = onEvent;
    invoke("plugin:androidwifi|register_listener", { event:eventName, handler }).then(
        () => new PluginListener("androidwifi", eventName, handler.id)
    );
}

export async function getClientMacAddress(gatewayIp: string | undefined): Promise<string | undefined> {
    if(gatewayIp === undefined){
        return undefined;
    }

    try{
        const macAddressResult: {macAddress: string | undefined} = await invoke("plugin:androidwifi|get_mac_address", { payload: { gatewayIp: gatewayIp } });

        let macAddress = macAddressResult.macAddress;

        // Convert 'null' string back to undefined
        if (macAddress === null || macAddress === 'null') {
            macAddress = undefined;
        }

        return macAddress
    } catch (e) {
        throw new Error(`Failed to determine MAC address, reason: ${e}`);
    }
}

export async function getGatewayIp(): Promise<string | undefined> {
    try{
        const gatewayIpResult: {gatewayIp: string | undefined} = await invoke("plugin:androidwifi|get_gateway_ip", { payload: { } });

        let gatewayIp = gatewayIpResult.gatewayIp;

        // Convert 'null' string back to undefined
        if (gatewayIp === null || gatewayIp === 'null') {
            gatewayIp = undefined;
        }

        return gatewayIp
    } catch (e) {
        throw new Error(`Failed to determine gatewayIp, reason: ${e}`);
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
    try{
        let response = await invoke("plugin:androidwifi|connect_wifi", {payload: { ssid: ssid } });
        console.log("response for connecting to " + ssid + " = " + JSON.stringify(response));
    } catch (e) {
        console.error(`Error connecting to network ${ssid}, reason: ${e}`)
    }
}

export async function markCaptivePortalDismissed(): Promise<void> {
    try{
        await invoke("plugin:androidwifi|mark_captive_portal_dismissed", {payload: {  } });
    } catch (e) {
        console.error(`Error marking captive portal dismissed, reason: ${e}`)
    }
}