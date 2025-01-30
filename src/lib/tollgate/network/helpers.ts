import type {NetworkInfo} from "$lib/tollgate/types/NetworkInfo";
import type {Tollgate} from "$lib/tollgate/types/Tollgate";
import { hexDecode} from "$lib/tollgate/helpers";
import type {NetworkElement} from "$lib/tollgate/types/NetworkElement";


export function getTollgates(networks: NetworkInfo[]) {
    let tollgates: Tollgate[] = []

    networks.forEach(network => {
        if(!isTollgateNetwork(network)) {
            return;
        }

        tollgates.push(toTollgate(network));
    })

    return tollgates;
}

export function isTollgateSsid(ssid: string): boolean {
    const lowerCaseSsid = ssid.toLowerCase();
    return lowerCaseSsid.startsWith("tollgate") || lowerCaseSsid.startsWith("openwrt");
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
        gatewayIp: tollgateInfo[5],
        pricing: {
            allocationType: tollgateInfo[2],
            allocationPer1024: Number(tollgateInfo[3]),
            unit: tollgateInfo[4],
        },
    }

    return tollgate;
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