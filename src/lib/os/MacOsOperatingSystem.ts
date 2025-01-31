import type IOperatingSystem from "$lib/os/IOperatingSystem";
import type { ConnectedNetworkInfo } from "$lib/tollgate/types/ConnectedNetworkInfo";
import type { NetworkInfo } from "$lib/tollgate/types/NetworkInfo";
import {invoke} from "@tauri-apps/api/core";
import {Command} from "@tauri-apps/plugin-shell";

export default class MacOsOperatingSystem implements IOperatingSystem {
    async getAvailableNetworks(): Promise<NetworkInfo[]> {
        console.log("Getting available networks");
        // let response: any = await invoke("get_available_networks");
        // console.log("response: ", response);

        throw new Error("MacOS  getAvailableNetworks not implemented.");
    }

    async getCurrentNetwork(): Promise<ConnectedNetworkInfo> {
        console.log("Getting current network");

        try{
            let result = await Command.create('run-networksetup', [
                '-getairportnetwork',
                "en0"
            ]).execute();

            const ssidWithReturnChar = result.stdout.split("Current Wi-Fi Network: ")[1]
            const ssid = ssidWithReturnChar.substring(0, ssidWithReturnChar.length - 1);

            return {
                ssid: ssid
            };
        } catch (error) {
            console.error("error", error);
            throw new Error("Error retrieving current network");
        }

    }

    async getMacAddress(gatewayIp: string): Promise<string> {
        let macAddress: string = await invoke("get_mac_address");

        // TODO actual error handling based on rust errors
        if(macAddress == "null" || macAddress == "err") {
            console.error("Failed getting mac address");
            throw new Error("MacOS getMAC address failed.");
        }

        return macAddress;
    }

    async connectNetwork(ssid: string): Promise<void> {
        console.error("MacOS connectNetwork not implemented");
    }

}