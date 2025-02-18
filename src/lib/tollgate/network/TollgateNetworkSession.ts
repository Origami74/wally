import {ConnectionStatus} from "$lib/tollgate/types/ConnectionStatus";
import { NRelay1 } from '@nostrify/nostrify';
import type {Tollgate} from "$lib/tollgate/types/Tollgate";

export default class TollgateNetworkSession {
    public readonly tollgate: Tollgate;
    private _userMacAddress: string | undefined;
    private _status: ConnectionStatus = ConnectionStatus.disconnected;
    private _tollgateRelay: NRelay1 | undefined;
    private _tollgateRelayReachable: boolean = false;

    constructor(tollgate: Tollgate) {
        this.tollgate = tollgate;
    }

    public get userMacAddress(): string | undefined {
        return this._userMacAddress;
    }

    public set userMacAddress(mac: string) {
        this._userMacAddress = mac;

        this.updateStatus()
    }

    private updateStatus() {
        if(this._userMacAddress && this._tollgateRelayReachable){
            this._status = ConnectionStatus.connected
            return
        }

        if(this._userMacAddress || this._tollgateRelayReachable){
            this._status = ConnectionStatus.initiating
            return
        }

        this._status = ConnectionStatus.disconnected
    }

    public get status(): ConnectionStatus {
        return this._status;
    }

    public get tollgateRelayReachable(): boolean {
        return this._tollgateRelayReachable;
    }

    private set tollgateRelayReachable(isReachable: boolean) {
        this._tollgateRelayReachable = isReachable;
        this.updateStatus()
    }

    public get tollgateRelay(): NRelay1 | undefined {
        try{
            if(!this._tollgateRelay){
                console.log("Setting up relay connection")
                this._tollgateRelay = new NRelay1(`http://${this.tollgate.gatewayIp}:3334`) // TODO 3334 -> 2121

                this._tollgateRelay.socket.addEventListener("open", () => {
                    console.log("Relay CONNECTED")
                    this.tollgateRelayReachable = true;
                })
                this._tollgateRelay.socket.addEventListener('close', () => {
                    console.log("Relay DISCONNECTED")
                    this.tollgateRelayReachable = false;
                })
            }
            return this._tollgateRelay;
        } catch (error) {
            console.log(`Error connecting to relay: ${error.message}`);
            return undefined;
        }
    }
}