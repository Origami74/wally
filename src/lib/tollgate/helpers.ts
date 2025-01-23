import type {NetworkElement, NetworkInfo, Tollgate} from "$lib/tollgate/ConnectionStatus";
import {fetch} from "@tauri-apps/plugin-http";
import {Buffer} from "buffer";

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
    if(network.ssid != "OpenWrt"){
        return false; // DEBUG
    }

    // All tollgates have to identify as tollgate (openwrt for debugging purposes)
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

export async function getMacAddress(): Promise<string|undefined> {

    const whoamiResponse = await fetch("http://192.168.1.1:2122/", {connectTimeout: 350}).catch((reason) => {
        console.error(reason);
        return undefined
    }) // Universal endpoint for whoami

    if(!whoamiResponse == undefined) {
        return "x"
    }

    let whoami = await whoamiResponse.json();

    if(whoami.Success === false) {
        const msg = `Failed to determine MAC address, reason: ${whoami.ErrorMessage}`
        console.error(msg);
        return "err";
        // throw new Error(msg);
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