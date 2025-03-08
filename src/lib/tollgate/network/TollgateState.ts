import { NRelay1 } from '@nostrify/nostrify';
import type NetworkState from "$lib/tollgate/network/NetworkState";
import {BehaviorSubject, combineLatest, map, Observable} from "rxjs";
import {fetch} from "@tauri-apps/plugin-http";

export default class TollgateState {
    public _relay: NRelay1 | undefined;
    public _networkState: NetworkState;

    public _tollgatePubkey = new BehaviorSubject<string | undefined>(undefined);
    public _networkHasRelay = new BehaviorSubject<boolean>(false);
    public _relayActive = new BehaviorSubject<boolean>(false);
    public _tollgateIsReady: Observable<boolean>;

    constructor(networkState: NetworkState) {
        this._networkState = networkState;

        this._networkHasRelay.subscribe((value => console.log(`Network has nostr relay`)));
        this._relayActive.subscribe((value => console.log(`relay ${value ? "" : "DIS"}CONNECTED`)));
        this._networkHasRelay.subscribe((value => console.log(`Current network is a TollGate!`)));

        this._tollgateIsReady = combineLatest([this._networkState.networkIsReady, this._networkHasRelay, this._tollgatePubkey])
            .pipe(map(([networkStateConnected, networkHasRelay, tollgatePubKey]) => {
               return (!!networkStateConnected && !!networkHasRelay && !!tollgatePubKey)
            }))
    }

    public async connect() {
        this.connectRelay()
        this._tollgatePubkey.next(await this.getTollgatePubkey());
    }

    public async getTollgatePubkey(){
        const url = ` http://${this._networkState._gatewayIp.value}:2122/pubkey`

        try{
            const pubkey = await fetch(url).then(res => res.text());

            return pubkey
        } catch(err) {
            console.error("Error fetching TollGate's pubkey, reason: ", err)
            return undefined
        }
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

    // TODO
    public reset() {
        this._networkHasRelay.next(false);
    }
}