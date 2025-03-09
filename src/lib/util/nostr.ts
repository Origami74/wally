import type { type NostrEvent } from "@nostrify/nostrify";

export function getTag(event: NostrEvent, tagName: string) : string[] | undefined {
    const tagArray =  event.tags.filter(item => item[0] === tagName)

    if(!tagArray || tagArray.length === 0) return undefined;

    return tagArray.reduce(item => item[0]);
}

export function getTags(event: NostrEvent, tagName: string) : string[][] | undefined {
    const tagArray =  event.tags.filter(item => item[0] === tagName)

    if(!tagArray || tagArray.length === 0) return undefined;

    return tagArray;
}