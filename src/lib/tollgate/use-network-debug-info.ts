import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type EventCallback, type UnlistenFn } from "@tauri-apps/api/event";

export type NetworkDebugInfo = {
  gateway_ip: string | null;
  mac_address: string | null;
  tollgate_pubkey: string | null;
  supported_tips: string[];
  metric: string | null;
  step_size: string | null;
  pricing_options: Array<{
    mint_url: string;
    price: string;
    unit: string;
  }>;
  current_wifi: {
    ssid: string;
    bssid: string;
  } | null;
  is_tollgate: boolean;
  advertisement_raw?: unknown;
};

const emptyNetworkInfo: NetworkDebugInfo = {
  gateway_ip: null,
  mac_address: null,
  tollgate_pubkey: null,
  supported_tips: [],
  metric: null,
  step_size: null,
  pricing_options: [],
  current_wifi: null,
  is_tollgate: false,
  advertisement_raw: undefined,
};

function toNetworkInfo(payload: any): NetworkDebugInfo {
  if (!payload || typeof payload !== "object") {
    return emptyNetworkInfo;
  }

  const tollgateAd = payload.tollgateAdvertisement ?? payload.advertisement ?? null;

  return {
    gateway_ip: payload.gatewayIp ?? payload.gateway_ip ?? null,
    mac_address: payload.macAddress ?? payload.mac_address ?? null,
    tollgate_pubkey: tollgateAd?.tollgatePubkey ?? tollgateAd?.tollgate_pubkey ?? null,
    supported_tips: tollgateAd?.tips ?? [],
    metric: tollgateAd?.metric ?? null,
    step_size: tollgateAd?.stepSize ?? tollgateAd?.step_size ?? null,
    pricing_options:
      tollgateAd?.pricingOptions?.map((option: any) => ({
        mint_url: option.mintUrl ?? option.mint_url ?? "",
        price: option.price ?? "",
        unit: option.unit ?? "",
      })) ?? [],
    current_wifi: payload.currentWifi ?? payload.current_wifi ?? null,
    is_tollgate: Boolean(payload.isTollgate ?? payload.is_tollgate ?? tollgateAd),
    advertisement_raw: tollgateAd ?? undefined,
  };
}

function mergeTollgateEvent(previous: NetworkDebugInfo, payload: any): NetworkDebugInfo {
  const next = toNetworkInfo(payload);

  return {
    gateway_ip: next.gateway_ip ?? previous.gateway_ip,
    mac_address: next.mac_address ?? previous.mac_address,
    tollgate_pubkey: next.tollgate_pubkey ?? previous.tollgate_pubkey,
    supported_tips: next.supported_tips.length ? next.supported_tips : previous.supported_tips,
    metric: next.metric ?? previous.metric,
    step_size: next.step_size ?? previous.step_size,
    pricing_options: next.pricing_options.length ? next.pricing_options : previous.pricing_options,
    current_wifi: next.current_wifi ?? previous.current_wifi,
    is_tollgate: next.is_tollgate || previous.is_tollgate,
    advertisement_raw: next.advertisement_raw ?? previous.advertisement_raw,
  };
}

export function useNetworkDebugInfo() {
  const [networkInfo, setNetworkInfo] = useState<NetworkDebugInfo>(emptyNetworkInfo);
  const [refreshing, setRefreshing] = useState(false);

  const refreshNetworkInfo = useCallback(async () => {
    setRefreshing(true);
    try {
      const networkStatus = await invoke("plugin:androidwifi|get_network_status", { payload: {} });
      setNetworkInfo(toNetworkInfo(networkStatus));
    } catch (error) {
      console.error("Failed to refresh network info", error);
    } finally {
      setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    let mounted = true;
    const listeners: UnlistenFn[] = [];

    refreshNetworkInfo();

    const setupListeners = async () => {
      try {
        const handleStatus: EventCallback<any> = (event) => {
          if (!mounted) return;
          setNetworkInfo(toNetworkInfo(event.payload));
        };
        const statusUnlisten = await listen("network-status-changed", handleStatus);
        listeners.push(statusUnlisten);

        const handleTollgate: EventCallback<any> = (event) => {
          if (!mounted) return;
          setNetworkInfo((prev) => mergeTollgateEvent(prev, event.payload));
        };
        const tollgateUnlisten = await listen("tollgate-detected", handleTollgate);
        listeners.push(tollgateUnlisten);
      } catch (error) {
        console.error("Failed to setup network debug listeners", error);
      }
    };

    setupListeners();

    return () => {
      mounted = false;
      listeners.forEach((off) => off());
    };
  }, [refreshNetworkInfo]);

  return useMemo(
    () => ({
      networkInfo,
      refreshNetworkInfo,
      refreshing,
    }),
    [networkInfo, refreshNetworkInfo, refreshing]
  );
}
