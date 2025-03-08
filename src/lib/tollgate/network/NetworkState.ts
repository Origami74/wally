import {getMacAddress} from "$lib/tollgate/network/pluginCommands";
import {BehaviorSubject, combineLatest, distinctUntilChanged, map, Observable, Subject} from 'rxjs';

export type OnConnectedInfo = {
    gatewayIp: string
}

export default class NetworkState {
    private _connected= new BehaviorSubject<boolean>(false);
    public _gatewayIp= new BehaviorSubject<string | undefined>(undefined);
    public _clientMacAddress = new BehaviorSubject<string | undefined>(undefined);

    public async onConnected(data: OnConnectedInfo){
        this._connected.next(true)
        this._gatewayIp.next(data.gatewayIp)
        this._clientMacAddress.next(await getMacAddress(this._gatewayIp.value));

        console.log(`Network connected, gateway: '${this._gatewayIp.value}', macAddress: '${this._clientMacAddress.value}'`);
    }

    public networkIsReady = combineLatest([this._connected, this._gatewayIp, this._clientMacAddress])
        .pipe( map(([connected, gatewayIp, clientMacAddress]) => {
            return (connected && gatewayIp && clientMacAddress);
        }), distinctUntilChanged());

    public reset(): void {
        console.log("NetworkState::reset");
        this._connected.next(false);
        this._gatewayIp.next(undefined);
    }

    public get clientMacAddress() {
        return this._clientMacAddress.value;
    }
}