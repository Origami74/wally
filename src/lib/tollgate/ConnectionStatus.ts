export enum ConnectionStatus {
    disconnected = 0,
    initiating = 1,
    connected = 2,
}

export interface TollgatePricing {
    allocationType: string;
    allocationPer1024: number;
    unit: string;
}

export interface Tollgate {
    ssid: string;
    bssid: string;
    rssi: number;
    frequency: string;
    version: string;
    pubkey: string;
    pricing: TollgatePricing
}

export interface NetworkInfo {
    ssid: string;
    bssid: string;
    rssi: number; // signal strenght in dB
    capabilities: string;
    frequency: string;
    informationElements: NetworkElement[];
}

export interface NetworkElement {
    id: string;
    idExt: string;
    bytes: string;
}