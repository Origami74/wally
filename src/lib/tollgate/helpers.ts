import type {NetworkElement, NetworkInfo, Tollgate} from "$lib/tollgate/ConnectionStatus";
import {fetch} from "@tauri-apps/plugin-http";
import {Buffer} from "buffer";
import {invoke} from "@tauri-apps/api/core";

// Checks if the passed array matches the tollgate vendor_elements bytes (212121).
// This is useful to avoid having to parse everything from hex to string first.
export function getTollgateVendorElement(network: NetworkInfo): NetworkElement | undefined {
    const tollgateIdentifierBytes = ["50","49","50","49","50","49"]

    for (const element of network.informationElements) {

        const x = element.bytes.slice(0, 6);
        if(tollgateIdentifierBytes.every((val, index) => val == x[index])){
            return element;
        }
    }

    return undefined;
}

export function isTollgateNetwork(network: NetworkInfo): boolean {
    // All tollgates have to identify as tollgate
    if(!isTollgateSsid(network.ssid)) {
        return false;
    }

    // Check if any of the information elements contains the tollgate info we're looking for
    if(getTollgateVendorElement(network) != undefined) {
        return true
    }

    console.log(`network ${network.ssid} does not contain TollGate element`);
    return false;
}

export function isTollgateSsid(ssid: string): boolean {
    const lowerCaseSsid = ssid.toLowerCase();
    return lowerCaseSsid.startsWith("tollgate") || lowerCaseSsid.startsWith("openwrt");
}

// Depending on the OS we can or cannot get the mac from our device, in case of android we have to call the whoami service.
// Ideally this happens inside the android plugin. Due to time constraints I've done it in two steps now. The 'androidwifi|getMacAddress' currently
// only rebinds the network to the wifi interface, sho that our actual web request will go to the router and not use our cellular connection.
export async function getMacAddress(gatewayIp: string): Promise<string|undefined> {
    // Step 1, rebind the app to the wifi network, so the whoami request won't be sent over any active cellular connection
    try{
        await invoke("plugin:androidwifi|getMacAddress", { }); // only rebinds
    } catch (e) {
        console.error("could not get mac native", e);
    }

    // Step 2, call the whoami service
    const whoamiResponse = await fetch(`http://${gatewayIp}:2122/`, {connectTimeout: 350}).catch((reason) => {
        console.error(reason);
        return undefined
    }) // Universal endpoint for whoami

    let whoami = await whoamiResponse.json();

    if(whoami.Success === false) {
        console.error(`Failed to determine MAC address, reason: ${whoami.ErrorMessage}`);
        return undefined;
    }

    return whoami.Mac
}

export function hexDecode(hex: string): string {
    return Buffer.from(hex, 'hex').toString()
}

export function toTollgate(network: NetworkInfo) {
    const vendorElements = hexDecode(getTollgateVendorElement(network).bytes)

    const tollgateInfo = vendorElements
        .slice(8) // drop vendor identifier
        .split('|');

    const tollgate: Tollgate = {
        ssid: network.ssid,
        bssid: network.bssid,
        rssi: network.rssi,
        frequency: network.frequency,
        pubkey: tollgateInfo[1],
        version: tollgateInfo[0],
        pricing: {
            allocationType: tollgateInfo[2],
            allocationPer1024: tollgateInfo[3],
            unit: tollgateInfo[4],
        },
    }

    return tollgate;
}

export const nostrNow = () => Math.floor(Date.now() / 1e3);