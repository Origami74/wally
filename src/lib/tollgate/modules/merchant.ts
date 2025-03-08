import {nostrNow} from "$lib/util/helpers";
import { NRelay1, NSecSigner } from '@nostrify/nostrify';

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
}