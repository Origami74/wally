import type IOperatingSystem from "$lib/os/IOperatingSystem";
import type { ConnectedNetworkInfo } from "$lib/tollgate/types/ConnectedNetworkInfo";
import type { NetworkInfo } from "$lib/tollgate/types/NetworkInfo";
import {invoke} from "@tauri-apps/api/core";

export default class MacOsOperatingSystem implements IOperatingSystem {
    async getAvailableNetworks(): Promise<NetworkInfo[]> {
        throw new Error("MacOS  getAvailableNetworks not implemented.");
    }

    async getCurrentNetwork(): Promise<ConnectedNetworkInfo> {
        // let macAddress: string = await invoke("get_mac_address");
        //
        // console.log("response: ", response);
        // // const networks: NetworkInfo[] = JSON.parse(response.wifis);
        //
        // // TODO actual error handling
        // if(!macAddress) {}

        const info: ConnectedNetworkInfo = {
            ssid: ""
        }
        throw new Error("MacOS getCurrentNetwork not implemented.");
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