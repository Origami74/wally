import type IOperatingSystem from "$lib/os/IOperatingSystem";
import type { ConnectedNetworkInfo } from "$lib/tollgate/types/ConnectedNetworkInfo";
import type { NetworkInfo } from "$lib/tollgate/types/NetworkInfo";

export default class MacOsOperatingSystem implements IOperatingSystem {
    getAvailableNetworks(): Promise<NetworkInfo[]> {
        throw new Error("MacOS  getAvailableNetworks not implemented.");
    }

    getCurrentNetwork(): Promise<ConnectedNetworkInfo> {


        const info: ConnectedNetworkInfo = {
            ssid: ""
        }
        throw new Error("MacOS getCurrentNetwork not implemented.");
    }

    getMacAddress(gatewayIp: string): Promise<string> {
        throw new Error("MacOS getMacAddress not implemented.");
    }

    async connectNetwork(ssid: string): Promise<void> {
        console.error("MacOS connectNetwork not implemented");
    }

}