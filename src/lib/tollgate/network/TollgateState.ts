import { NRelay1 } from '@nostrify/nostrify';
import type NetworkState from "$lib/tollgate/network/NetworkState";
import {BehaviorSubject, combineLatest, map, Observable, scan, tap} from "rxjs";

export default class TollgateState {
    private _relay: NRelay1 | undefined;
    private _networkState: NetworkState;

    public _networkHasRelay = new BehaviorSubject<boolean>(false);
    public _relayActive = new BehaviorSubject<boolean>(false);
    public _isTollgate: Observable<boolean>

    constructor(networkState: NetworkState) {
        this._networkState = networkState;

        this._networkHasRelay.subscribe((value => console.log(`Network has nostr relay`)));
        this._relayActive.subscribe((value => console.log(`relay ${value ? "" : "DIS"}CONNECTED`)));
        this._networkHasRelay.subscribe((value => console.log(`Current network is a TollGate!`)));

        this._isTollgate = combineLatest([this._networkState.networkIsReady, this._networkHasRelay])
            .pipe(map(([networkStateConnected, networkHasRelay]) => {
               return networkStateConnected && networkHasRelay
            }))
    }

    public connect() {
        this.connectRelay()
    }

    public connectRelay(){
        const url = ` http://${this._networkState._gatewayIp.value}:3334` // TODO 3334 -> 2121
        console.log(`Connecting to Tollgate relay ${url}`);
        this._relay = new NRelay1(url)

        this._relay.socket.addEventListener("open", () => {
            this._networkHasRelay.next(true);
            this._relayActive.next(true);
        })
        this._relay.socket.addEventListener('close', () => {
            this._relayActive.next(false);
        })
    }

    public reset() {
        this._networkHasRelay.next(false);
    }
}