import {getMacAddress} from "$lib/tollgate/network/pluginCommands";

export type OnConnectedInfo = {
    gatewayIp: string
}

export default class NetworkState {
    private _connected: boolean = false;
    private _gatewayIp: string | undefined;
    private _clientMacAddress: string | undefined;
    private _ssid: string | undefined;
    private _capitvePortalActive: boolean = false;

    public async onConnected(data: OnConnectedInfo){

        this._connected = true;
        this._gatewayIp = data.gatewayIp;
        this._clientMacAddress = await getMacAddress(this._gatewayIp);

        console.log(`Network connected, gateway: '${this._gatewayIp}', macAddress: '${this._clientMacAddress}'`);
    }

    public get isConnected(): boolean {
        return this._connected;
    }

    public reset(): void {
        console.log("NetworkState::reset");
        this._connected = false;
        this._gatewayIp = undefined;
        this._ssid = undefined;
        this._capitvePortalActive = false;
    }
}