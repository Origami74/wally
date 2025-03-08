import TollgateNetworkSession from "$lib/tollgate/network/TollgateNetworkSession";
import {nostrNow} from "$lib/util/helpers";
import { NSecSigner } from '@nostrify/nostrify';

export async function makePurchase(session: TollgateNetworkSession) {
    console.log("purchasing data")
    const relay = session.tollgateRelay

    if(!relay) {
        console.error("No Tollgate relay found for session");
        return;
    }

    let randomPrivateKey = "4e007801c927832ebfe06e57ef08dba5aefe44076a0add96b1700c9061313490"
    const signer = new NSecSigner(randomPrivateKey);

    const note = {
        kind: 21000,
        pubkey: signer.getPublicKey(),
        content: "cashuAbcde",
        created_at: nostrNow(),
        tags: [
            ["p", session.tollgate.pubkey],
            ["mac", session.userMacAddress],
        ],
    };
    const event = await signer.signEvent(note);

    console.log(`sending: ${JSON.stringify(event)}`);
    await relay.event(event);
}