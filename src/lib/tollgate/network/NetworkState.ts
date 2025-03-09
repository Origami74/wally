import {getClientMacAddress, getGatewayIp} from "$lib/tollgate/network/pluginCommands";
import {BehaviorSubject, combineLatest, distinctUntilChanged, map, Observable, Subject} from 'rxjs';

export type OnConnectedInfo = {
    gatewayIp: string
}

export default class NetworkState {
    private _connected= new BehaviorSubject<boolean>(false);
    public _gatewayIp= new BehaviorSubject<string | undefined>(undefined);
    public _clientMacAddress = new BehaviorSubject<string | undefined>(undefined);

    public async performNetworkCheck(){
        this._connected.next(true)
        this._gatewayIp.next(await getGatewayIp())
        this._clientMacAddress.next(await getClientMacAddress(this._gatewayIp.value));

        console.log(`Network connected, gateway: '${this._gatewayIp.value}', macAddress: '${this._clientMacAddress.value}'`);
    }

    public networkIsReady = combineLatest([this._connected, this._gatewayIp, this._clientMacAddress])
        .pipe(map(([connected, gatewayIp, clientMacAddress]) => {
            return (connected === true && !!gatewayIp && !!clientMacAddress)
        }), distinctUntilChanged());

    public reset(): void {
        console.log("NetworkState::reset");
        this._connected.next(false);
        this._gatewayIp.next(undefined);
        this._clientMacAddress.next(undefined);
    }
}