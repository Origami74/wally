import type {TollgatePricing} from "$lib/tollgate/types/TollgatePricing";

export interface Tollgate {
    ssid: string;
    bssid: string;
    rssi: number;
    frequency: string;
    version: string;
    pubkey: string;
    gatewayIp: string;
    pricing: TollgatePricing
}