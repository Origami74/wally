import { invoke } from "@tauri-apps/api/core";

export interface NostrProvider {
  id: string;
  pubkey: string;
  name: string;
  about: string;
  urls: string[];
  mints: string[];
  version?: string;
  created_at: string;
  updated_at: string;
  followers: number;
  zaps: number;
  use_onion: boolean;
  is_online: boolean;
}

export async function discoverNostrProviders(): Promise<NostrProvider[]> {
  return invoke("discover_nostr_providers");
}