import {Buffer} from "buffer";

export function hexDecode(hex: string): string {
    return Buffer.from(hex, 'hex').toString()
}

export const nostrNow = () => Math.floor(Date.now() / 1e3);

export const shortenString = (str: string, num: number = 4): string =>
    str.length > num * 2 ? `${str.slice(0, num)}...${str.slice(-num)}` : str;
