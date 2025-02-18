import {Buffer} from "buffer";

export function hexDecode(hex: string): string {
    return Buffer.from(hex, 'hex').toString()
}

export const nostrNow = () => Math.floor(Date.now() / 1e3);