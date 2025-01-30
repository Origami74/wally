import type {NetworkInfo} from "$lib/tollgate/types/NetworkInfo";
import type {ConnectedNetworkInfo} from "$lib/tollgate/types/ConnectedNetworkInfo";

export default interface IOperatingSystem {
    connectNetwork(ssid: string): Promise<void>;
    getAvailableNetworks(): Promise<NetworkInfo[]>;
    getCurrentNetwork(): Promise<ConnectedNetworkInfo>;
    getMacAddress(gatewayIp: string): Promise<string>;
}