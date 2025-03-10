import { BehaviorSubject, combineLatest, map, Observable} from "rxjs";
import type TollgateState from "$lib/tollgate/network/TollgateState";
import {markCaptivePortalDismissed} from "$lib/tollgate/network/pluginCommands";
import { generateSecretKey, getPublicKey } from 'nostr-tools/pure'
import {nostrNow} from "$lib/util/helpers";
import {getTag} from "$lib/util/nostr";
import { NSecSigner } from '@nostrify/nostrify';

export default class TollgateSessionState {
    private _tollgateState: TollgateState;

    public _sessionSecretKey = new BehaviorSubject<Uint8Array<ArrayBufferLike> | undefined>(undefined);
    public _sessionIsActive: Observable<boolean>;
    private _sessionConfirmedByTollgate = new BehaviorSubject<boolean>(false);

    constructor(tollgateState: TollgateState) {
        this._tollgateState = tollgateState;

        // Create session state when tollGate becomes ready
        this._tollgateState._tollgateIsReady.subscribe(async (tollgateIsReady) => {
            if(tollgateIsReady) {
                await this.makePurchase();
            } else{
                console.log("TollgateState no longer ready, resetting tollgateSessionState (TODO)")
                this.reset()
            }
        })

        this._sessionIsActive = combineLatest([this._tollgateState._tollgateIsReady, this._sessionSecretKey, this._sessionConfirmedByTollgate])
            .pipe(map(([tollgateIsReady, sessionSecretKey, sessionConfirmedByTollgate]) => {
               return (tollgateIsReady === true && !!sessionSecretKey && sessionConfirmedByTollgate === true)
            }))

        this._sessionSecretKey.subscribe(async (secretKey) => {
            if(!!secretKey){
                await this.listenForSessionConfirmation()
            }
        })

    }

    private reset() {
        this._sessionConfirmedByTollgate.next(false);
        this._sessionSecretKey.next(undefined);
    }

    private async makePurchase(): Promise<void> {
        console.log("cycling private key")
        this._sessionSecretKey.next(generateSecretKey());

        console.log("purchasing data")

        let randomPrivateKey = "4e007801c927832ebfe06e57ef08dba5aefe44076a0add96b1700c9061313490"
        const signer = new NSecSigner(randomPrivateKey);

        const note = {
            kind: 21000,
            pubkey: signer.getPublicKey(),
            content: "cashuAbcde",
            created_at: nostrNow(),
            tags: [
                ["p", this._tollgateState._tollgatePubkey.value!],
                ["mac", this._tollgateState._networkState._clientMacAddress.value!],
            ],
        };
        const event = await signer.signEvent(note);

        console.log(`sending: ${JSON.stringify(event)}`);
        await this._tollgateState._relay!.event(event);
    }

    private async listenForSessionConfirmation() {
        console.log("listening for Tollgate session confirmation");
        const sessionFilter = {
            kinds: [2200, 22000, 66666],
            since: nostrNow() - 5
            // "#mac": [clientMacAddress]
        }

        for await (const msg of this._tollgateState._relay!.req([sessionFilter])) {
            // console.log(msg);
            if (msg[0] === 'EVENT') {
                console.log(msg[2]);

                const event = msg[2]

                const macAddress = getTag(event, "mac")?.[1]
                const sessionEnd = getTag(event, "session-end")?.[1]

                if(!macAddress || !sessionEnd) {
                    console.log("mac/session-end missing from tags: ", macAddress, sessionEnd)
                    continue;
                }

                if(macAddress != this._tollgateState._networkState._clientMacAddress.value) {
                    continue;
                }

                console.log("sessionEnd", sessionEnd)
                console.log("session left:", Number(sessionEnd) - nostrNow())

                // TODO: wait for other kind of event from the valve that confirms opened
                setTimeout(async () => {
                    await markCaptivePortalDismissed()
                }, 1000)
                this._sessionConfirmedByTollgate.next(true)
                return
            }
            // if (msg[0] === 'EOSE') continue; // Sends a `CLOSE` message to the relay.
        }
    }

    sessionExpired() {
        this.reset()
    }
}