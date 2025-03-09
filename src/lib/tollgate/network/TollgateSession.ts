import {combineLatest, map, Observable} from "rxjs";
import type TollgateState from "$lib/tollgate/network/TollgateState";
import {makePurchase} from "$lib/tollgate/modules/merchant";
import {markCaptivePortalDismissed} from "$lib/tollgate/network/pluginCommands";

export default class TollgateSession {
    private _tollgateState: TollgateState;

    public _sessionIsActive: Observable<boolean>;

    constructor(tollgateState: TollgateState) {
        this._tollgateState = tollgateState;

        this._sessionIsActive = combineLatest([this._tollgateState])
            .pipe(map(([tollgateState]) => {
               return (false)
            }))

        // this._sessionIsActive.subscribe((value => console.log(`Tollgate session ${value ? "STARTED" : "STOPPED"}`)));
    }

    public async createSession(): Promise<void> {
        await makePurchase(
            this._tollgateState._relay!,
            this._tollgateState._tollgatePubkey.value!,
            this._tollgateState._networkState._clientMacAddress.value!
        );

        setTimeout(async () => {
            await markCaptivePortalDismissed()
        }, 1500)
    }
}