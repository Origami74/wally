// import { NRelay1 } from '@nostrify/nostrify';
//
// export default class TollgateState {
//     private _isTollgate: boolean = false;
//     private _relay: NRelay1 | undefined;
//     private _capitvePortalActive: boolean = false;
//
//     public onConnected(){
//         this._connected = true;
//     }
//
//     public get isTollgate(): boolean {
//         return this._isTollgate;
//     }
//
//
//
//     public x(){
//         console.log("Setting up relay connection")
//         this._relay = new NRelay1(`http://${this.tollgate.gatewayIp}:3334`) // TODO 3334 -> 2121
//
//         this._relay.socket.addEventListener("open", () => {
//             console.log("Relay CONNECTED")
//             this.tollgateRelayReachable = true;
//         })
//         this._relay.socket.addEventListener('close', () => {
//             console.log("Relay DISCONNECTED")
//             this.tollgateRelayReachable = false;
//         })
//     }
// }