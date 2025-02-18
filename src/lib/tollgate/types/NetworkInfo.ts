import type {NetworkElement} from "$lib/tollgate/types/NetworkElement";

export interface NetworkInfo {
    ssid: string;
    bssid: string;
    rssi: number; // signal strenght in dB
    capabilities: string;
    frequency: string;
    informationElements: NetworkElement[];
}