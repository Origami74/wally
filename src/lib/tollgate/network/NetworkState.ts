
export default class NetworkState {
    private _connected: boolean = false;
    private _ssid: string | undefined;
    private _capitvePortalActive: boolean = false;

    public onConnected(){
        this._connected = true;
    }

    public get isConnected(): boolean {
        return this._connected;
    }
}