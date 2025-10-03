import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { Screen } from "@/components/layout/screen";
import { SectionHeader } from "@/components/layout/section-header";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { CopyButton } from "@/components/copy-button";
import type { ServiceStatus } from "@/lib/tollgate/types";

type NetworkDebugInfo = {
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
  advertisement_raw?: any;
};

type DebugScreenProps = {
  status: ServiceStatus | null;
  copyToClipboard: (value: string) => Promise<void> | void;
};

export function DebugScreen({ status, copyToClipboard }: DebugScreenProps) {
  const [networkInfo, setNetworkInfo] = useState<NetworkDebugInfo>({
    gateway_ip: null,
    mac_address: null,
    tollgate_pubkey: null,
    supported_tips: [],
    metric: null,
    step_size: null,
    pricing_options: [],
    current_wifi: null,
    is_tollgate: false,
  });
  const [refreshing, setRefreshing] = useState(false);

  const refreshNetworkInfo = async () => {
    setRefreshing(true);
    try {
      const networkStatus = await invoke("plugin:androidwifi|get_network_status", { payload: {} });
      const statusObj = networkStatus as any;
      const tollgateAd = statusObj?.tollgateAdvertisement;

      setNetworkInfo({
        gateway_ip: statusObj?.gatewayIp || null,
        mac_address: statusObj?.macAddress || null,
        tollgate_pubkey: tollgateAd?.tollgatePubkey || null,
        supported_tips: tollgateAd?.tips || [],
        metric: tollgateAd?.metric || null,
        step_size: tollgateAd?.stepSize || null,
        pricing_options:
          tollgateAd?.pricingOptions?.map((option: any) => ({
            mint_url: option.mintUrl || "",
            price: option.price || "",
            unit: option.unit || "",
          })) || [],
        current_wifi: statusObj?.currentWifi || null,
        is_tollgate: statusObj?.isTollgate || false,
        advertisement_raw: tollgateAd,
      });
    } catch (error) {
      console.error("Failed to refresh network info:", error);
    } finally {
      setRefreshing(false);
    }
  };

  useEffect(() => {
    refreshNetworkInfo();

    const setupEventListeners = async () => {
      try {
        const networkStatusUnlisten = await listen("network-status-changed", (event: any) => {
          const networkStatus = event.payload;
          const tollgateAd = networkStatus?.tollgateAdvertisement;

          setNetworkInfo({
            gateway_ip: networkStatus?.gatewayIp || null,
            mac_address: networkStatus?.macAddress || null,
            tollgate_pubkey: tollgateAd?.tollgatePubkey || null,
            supported_tips: tollgateAd?.tips || [],
            metric: tollgateAd?.metric || null,
            step_size: tollgateAd?.stepSize || null,
            pricing_options:
              tollgateAd?.pricingOptions?.map((option: any) => ({
                mint_url: option.mintUrl || "",
                price: option.price || "",
                unit: option.unit || "",
              })) || [],
            current_wifi: networkStatus?.currentWifi || null,
            is_tollgate: networkStatus?.isTollgate || false,
            advertisement_raw: tollgateAd,
          });
        });

        const tollgateUnlisten = await listen("tollgate-detected", (event: any) => {
          const tollgateInfo = event.payload;
          const tollgateAd = tollgateInfo?.tollgateAdvertisement;

          setNetworkInfo((prev) => ({
            ...prev,
            gateway_ip: tollgateInfo?.gatewayIp || prev.gateway_ip,
            mac_address: tollgateInfo?.macAddress || prev.mac_address,
            tollgate_pubkey: tollgateAd?.tollgatePubkey || prev.tollgate_pubkey,
            supported_tips: tollgateAd?.tips || prev.supported_tips,
            metric: tollgateAd?.metric || prev.metric,
            step_size: tollgateAd?.stepSize || prev.step_size,
            pricing_options:
              tollgateAd?.pricingOptions?.map((option: any) => ({
                mint_url: option.mintUrl || "",
                price: option.price || "",
                unit: option.unit || "",
              })) || prev.pricing_options,
            is_tollgate: tollgateInfo?.isTollgate || prev.is_tollgate,
            advertisement_raw: tollgateAd || prev.advertisement_raw,
          }));
        });

        return () => {
          networkStatusUnlisten();
          tollgateUnlisten();
        };
      } catch (error) {
        console.error("Failed to setup event listeners:", error);
      }
    };

    const cleanup = setupEventListeners();

    return () => {
      cleanup.then((fn) => fn && fn());
    };
  }, []);

  const formatValue = (value: any): string => {
    if (value === null || value === undefined) return "--";
    if (Array.isArray(value)) return value.length > 0 ? value.join(", ") : "--";
    return String(value);
  };

  return (
    <Screen className="min-h-screen gap-6 overflow-y-auto pt-6">
      <SectionHeader
        title="Tollgate Settings"
        description="Inspect live network data and service status."
        actions={
          <Button onClick={refreshNetworkInfo} disabled={refreshing} variant="outline" size="sm">
            {refreshing ? "Refreshing…" : "Refresh"}
          </Button>
        }
      />

      <Card className="space-y-4 border border-dashed border-primary/20 bg-background/90 p-4">
        <div className="flex items-center justify-between">
          <h3 className="text-base font-semibold">Network Status</h3>
          <Badge tone={networkInfo.is_tollgate ? "success" : "default"}>
            {networkInfo.is_tollgate ? "Tollgate Network" : "Standard Network"}
          </Badge>
        </div>

        <div className="grid gap-3 text-sm">
          <div className="flex justify-between">
            <span className="text-muted-foreground">Gateway:</span>
            <div className="flex items-center gap-2">
              <span className="font-mono">{formatValue(networkInfo.gateway_ip)}</span>
              {networkInfo.gateway_ip && (
                <CopyButton
                  onCopy={() => copyToClipboard(networkInfo.gateway_ip!)}
                  label=""
                  copiedLabel="✓"
                  variant="ghost"
                  className="h-6 w-6 p-0"
                />
              )}
            </div>
          </div>

          <div className="flex justify-between">
            <span className="text-muted-foreground">MAC Address:</span>
            <div className="flex items-center gap-2">
              <span className="font-mono">{formatValue(networkInfo.mac_address)}</span>
              {networkInfo.mac_address && (
                <CopyButton
                  onCopy={() => copyToClipboard(networkInfo.mac_address!)}
                  label=""
                  copiedLabel="✓"
                  variant="ghost"
                  className="h-6 w-6 p-0"
                />
              )}
            </div>
          </div>

          {networkInfo.current_wifi && (
            <>
              <div className="flex justify-between">
                <span className="text-muted-foreground">WiFi SSID:</span>
                <span className="font-mono">{networkInfo.current_wifi.ssid}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">WiFi BSSID:</span>
                <span className="font-mono">{networkInfo.current_wifi.bssid}</span>
              </div>
            </>
          )}
        </div>
      </Card>

      <Card className="space-y-4 border border-dashed border-primary/20 bg-background/90 p-4">
        <h3 className="text-base font-semibold">Tollgate Information</h3>

        <div className="grid gap-3 text-sm">
          <div className="flex justify-between">
            <span className="text-muted-foreground">Tollgate Pubkey:</span>
            <div className="flex items-center gap-2">
              <span className="font-mono text-xs max-w-[200px] truncate">
                {formatValue(networkInfo.tollgate_pubkey)}
              </span>
              {networkInfo.tollgate_pubkey && (
                <CopyButton
                  onCopy={() => copyToClipboard(networkInfo.tollgate_pubkey!)}
                  label=""
                  copiedLabel="✓"
                  variant="ghost"
                  className="h-6 w-6 p-0"
                />
              )}
            </div>
          </div>

          <div className="flex justify-between">
            <span className="text-muted-foreground">Supported TIPs:</span>
            <span className="font-mono">{formatValue(networkInfo.supported_tips)}</span>
          </div>

          <div className="flex justify-between">
            <span className="text-muted-foreground">Metric:</span>
            <span className="font-mono">{formatValue(networkInfo.metric)}</span>
          </div>

          <div className="flex justify-between">
            <span className="text-muted-foreground">Step Size:</span>
            <span className="font-mono">{formatValue(networkInfo.step_size)}</span>
          </div>

          {networkInfo.pricing_options && networkInfo.pricing_options.length > 0 && (
            <div className="space-y-2">
              <span className="text-muted-foreground">Pricing Options:</span>
              {networkInfo.pricing_options.map((option, index) => (
                <div key={index} className="ml-4 rounded bg-muted/50 p-2 text-xs space-y-1">
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">Price:</span>
                    <span className="font-mono">{option.price} {option.unit}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">Mint URL:</span>
                    <div className="flex items-center gap-2">
                      <span className="font-mono text-xs max-w-[150px] truncate">{option.mint_url}</span>
                      {option.mint_url && (
                        <CopyButton
                          onCopy={() => copyToClipboard(option.mint_url)}
                          label=""
                          copiedLabel="✓"
                          variant="ghost"
                          className="h-4 w-4 p-0"
                        />
                      )}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </Card>

      {status && (
        <Card className="space-y-4 border border-dashed border-primary/20 bg-background/90 p-4">
          <h3 className="text-base font-semibold">Service Status</h3>

          <div className="grid gap-3 text-sm">
            <div className="flex justify-between">
              <span className="text-muted-foreground">Auto Tollgate:</span>
              <Badge tone={status.auto_tollgate_enabled ? "success" : "default"}>
                {status.auto_tollgate_enabled ? "Enabled" : "Disabled"}
              </Badge>
            </div>

            <div className="flex justify-between">
              <span className="text-muted-foreground">Active Sessions:</span>
              <span className="font-mono">{status.active_sessions?.length || 0}</span>
            </div>

            <div className="flex justify-between">
              <span className="text-muted-foreground">Wallet Balance:</span>
              <span className="font-mono">{status.wallet_balance || 0} sats</span>
            </div>

            {status.active_sessions?.[0] && (
              <>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Session Status:</span>
                  <span className="font-mono">{status.active_sessions[0].status}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Total Spent:</span>
                  <span className="font-mono">{status.active_sessions[0].total_spent} sats</span>
                </div>
              </>
            )}
          </div>
        </Card>
      )}

      {networkInfo.advertisement_raw && (
        <Card className="space-y-4 border border-dashed border-primary/20 bg-background/90 p-4">
          <div className="flex items-center justify-between">
            <h3 className="text-base font-semibold">Raw Advertisement Data</h3>
            <CopyButton
              onCopy={() => copyToClipboard(JSON.stringify(networkInfo.advertisement_raw, null, 2))}
              label="Copy JSON"
              copiedLabel="Copied"
              variant="outline"
            />
          </div>

          <pre className="overflow-x-auto rounded bg-muted/20 p-3 text-xs">
            {JSON.stringify(networkInfo.advertisement_raw, null, 2)}
          </pre>
        </Card>
      )}
    </Screen>
  );
}
