import {nostrNow} from "$lib/util/helpers";
import { NRelay1, NSecSigner } from '@nostrify/nostrify';
import {getTag} from "$lib/util/nostr";

export async function makePurchase(relay: NRelay1, tollgatePubkey: string, clientMacAddress: string): Promise<void> {
    console.log("purchasing data")

    let randomPrivateKey = "4e007801c927832ebfe06e57ef08dba5aefe44076a0add96b1700c9061313490"
    const signer = new NSecSigner(randomPrivateKey);

    const note = {
        kind: 21000,
        pubkey: signer.getPublicKey(),
        content: "cashuAbcde",
        created_at: nostrNow(),
        tags: [
            ["p", tollgatePubkey],
            ["mac", clientMacAddress],
        ],
    };
    const event = await signer.signEvent(note);

    console.log(`sending: ${JSON.stringify(event)}`);
    await relay.event(event);

    // const sessionFilter = {
    //     kinds: [2200, 22000, 66666],
    //     since: nostrNow() - 60000000
    //     // "#mac": [clientMacAddress]
    // }
    //
    // for await (const msg of relay.req([sessionFilter])) {
    //     // console.log(msg);
    //     if (msg[0] === 'EVENT') {
    //         console.log(msg[2]);
    //
    //         const event = msg[2]
    //
    //         const macAddress = getTag(event, "mac")?.[1]
    //         const sessionEnd = getTag(event, "session-end")?.[1]
    //
    //         if(!macAddress || !sessionEnd) {
    //             console.log("mac/session-end missing from tags: ", macAddress, sessionEnd)
    //             continue;
    //         }
    //
    //         if(macAddress != clientMacAddress) {
    //             continue;
    //         }
    //
    //         console.log("sessionEnd", sessionEnd)
    //         console.log("session left:", Number(sessionEnd) - nostrNow())
    //     }
    //     // if (msg[0] === 'EOSE') continue; // Sends a `CLOSE` message to the relay.
    // }

}