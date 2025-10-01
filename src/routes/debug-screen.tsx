import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Screen } from "@/components/layout/screen";
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
      // Get comprehensive network status from androidwifi plugin
      console.log("Debug screen: Calling get_network_status...");
      const networkStatus = await invoke("plugin:androidwifi|get_network_status", { payload: {} }) as any;
      console.log("Debug screen: Network status response:", networkStatus);
      
      setNetworkInfo({
        gateway_ip: networkStatus?.gatewayIp || networkStatus?.gateway_ip || null,
        mac_address: networkStatus?.macAddress || networkStatus?.mac_address || null,
        tollgate_pubkey: networkStatus?.tollgateAdvertisement?.tollgatePubkey || networkStatus?.tollgate_advertisement?.tollgate_pubkey || null,
        supported_tips: networkStatus?.tollgateAdvertisement?.tips || networkStatus?.tollgate_advertisement?.tips || [],
        metric: networkStatus?.tollgateAdvertisement?.metric || networkStatus?.tollgate_advertisement?.metric || null,
        step_size: networkStatus?.tollgateAdvertisement?.stepSize || networkStatus?.tollgate_advertisement?.step_size || null,
        pricing_options: networkStatus?.tollgateAdvertisement?.pricingOptions || networkStatus?.tollgate_advertisement?.pricing_options || [],
        current_wifi: networkStatus?.currentWifi || networkStatus?.current_wifi || null,
        is_tollgate: networkStatus?.isTollgate || networkStatus?.is_tollgate || false,
        advertisement_raw: networkStatus?.tollgateAdvertisement || networkStatus?.tollgate_advertisement,
      });
    } catch (error) {
      console.error("Failed to refresh network info:", error);
      // Fallback to individual calls if the new command fails
      try {
        const [gatewayResult, macResult, wifiResult] = await Promise.all([
          invoke("plugin:androidwifi|get_gateway_ip", { payload: {} }).catch(() => ({ gateway_ip: null })),
          invoke("plugin:androidwifi|get_mac_address", { payload: { gateway_ip: "" } }).catch(() => ({ mac_address: null })),
          invoke("plugin:androidwifi|get_current_wifi_details", { payload: {} }).catch(() => ({ wifi: null })),
        ]);

        // Extract tollgate info from current status
        const tollgateInfo = status?.current_network?.advertisement;
        
        setNetworkInfo({
          gateway_ip: (gatewayResult as any)?.gateway_ip || status?.current_network?.gateway_ip || null,
          mac_address: (macResult as any)?.mac_address || status?.current_network?.mac_address || null,
          tollgate_pubkey: tollgateInfo?.tollgate_pubkey || null,
          supported_tips: tollgateInfo?.tips || [],
          metric: tollgateInfo?.metric || null,
          step_size: tollgateInfo?.step_size?.toString() || null,
          pricing_options: tollgateInfo?.pricing_options?.map((option: any) => ({
            mint_url: option.mint_url || option.mintUrl || '',
            price: option.price || '',
            unit: option.unit || ''
          })) || [],
          current_wifi: (wifiResult as any)?.wifi || null,
          is_tollgate: status?.current_network?.is_tollgate || false,
          advertisement_raw: tollgateInfo,
        });
      } catch (fallbackError) {
        console.error("Fallback network info fetch also failed:", fallbackError);
      }
    } finally {
      setRefreshing(false);
    }
  };

  useEffect(() => {
    refreshNetworkInfo();
    
    // Listen for real-time network events
    const setupEventListeners = async () => {
      try {
        // Listen for network status changes
        const networkStatusUnlisten = await listen('network-status-changed', (event: any) => {
          console.log('Debug screen: Network status changed', event.payload);
          const networkStatus = event.payload;
          setNetworkInfo({
            gateway_ip: networkStatus?.gatewayIp || networkStatus?.gateway_ip || null,
            mac_address: networkStatus?.macAddress || networkStatus?.mac_address || null,
            tollgate_pubkey: networkStatus?.tollgateAdvertisement?.tollgatePubkey || networkStatus?.tollgate_advertisement?.tollgate_pubkey || null,
            supported_tips: networkStatus?.tollgateAdvertisement?.tips || networkStatus?.tollgate_advertisement?.tips || [],
            metric: networkStatus?.tollgateAdvertisement?.metric || networkStatus?.tollgate_advertisement?.metric || null,
            step_size: networkStatus?.tollgateAdvertisement?.stepSize || networkStatus?.tollgate_advertisement?.step_size || null,
            pricing_options: networkStatus?.tollgateAdvertisement?.pricingOptions || networkStatus?.tollgate_advertisement?.pricing_options || [],
            current_wifi: networkStatus?.currentWifi || networkStatus?.current_wifi || null,
            is_tollgate: networkStatus?.isTollgate || networkStatus?.is_tollgate || false,
            advertisement_raw: networkStatus?.tollgateAdvertisement || networkStatus?.tollgate_advertisement,
          });
        });

        // Listen for tollgate detection events
        const tollgateUnlisten = await listen('tollgate-detected', (event: any) => {
          console.log('Debug screen: Tollgate detected', event.payload);
          const tollgateInfo = event.payload;
          setNetworkInfo(prev => ({
            ...prev,
            tollgate_pubkey: tollgateInfo?.advertisement?.tollgate_pubkey || null,
            supported_tips: tollgateInfo?.advertisement?.tips || [],
            metric: tollgateInfo?.advertisement?.metric || null,
            step_size: tollgateInfo?.advertisement?.step_size?.toString() || null,
            pricing_options: tollgateInfo?.advertisement?.pricing_options?.map((option: any) => ({
              mint_url: option.mint_url || option.mintUrl || '',
              price: option.price || '',
              unit: option.unit || ''
            })) || [],
            is_tollgate: tollgateInfo?.is_tollgate || false,
            advertisement_raw: tollgateInfo?.advertisement,
          }));
        });

        return () => {
          networkStatusUnlisten();
          tollgateUnlisten();
        };
      } catch (error) {
        console.error('Failed to setup event listeners:', error);
      }
    };

    const cleanup = setupEventListeners();
    
    return () => {
      cleanup.then(fn => fn && fn());
    };
  }, [status]);

  const formatValue = (value: any): string => {
    if (value === null || value === undefined) return "--";
    if (Array.isArray(value)) return value.length > 0 ? value.join(", ") : "--";
    return String(value);
  };

  return (
    <Screen className="min-h-screen gap-6 overflow-y-auto">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold">Tollgate Debug</h1>
        <Button 
          onClick={refreshNetworkInfo} 
          disabled={refreshing}
          variant="outline"
          size="sm"
        >
          {refreshing ? "Refreshing..." : "Refresh"}
        </Button>
      </div>

      {/* Network Status Overview */}
      <Card className="space-y-4 border border-dashed border-primary/20 bg-background/90 p-4">
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">Network Status</h2>
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

      {/* Tollgate Information */}
      <Card className="space-y-4 border border-dashed border-primary/20 bg-background/90 p-4">
        <h2 className="text-lg font-semibold">Tollgate Information</h2>
        
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
                <div key={index} className="ml-4 p-2 bg-muted/50 rounded text-xs space-y-1">
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

      {/* Service Status */}
      {status && (
        <Card className="space-y-4 border border-dashed border-primary/20 bg-background/90 p-4">
          <h2 className="text-lg font-semibold">Service Status</h2>
          
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

      {/* Raw Advertisement Data */}
      {networkInfo.advertisement_raw && (
        <Card className="space-y-4 border border-dashed border-primary/20 bg-background/90 p-4">
          <div className="flex items-center justify-between">
            <h2 className="text-lg font-semibold">Raw Advertisement Data</h2>
            <CopyButton
              onCopy={() => copyToClipboard(JSON.stringify(networkInfo.advertisement_raw, null, 2))}
              label="Copy JSON"
              copiedLabel="Copied"
              variant="outline"
            />
          </div>
          
          <pre className="text-xs bg-muted/20 p-3 rounded overflow-x-auto">
            {JSON.stringify(networkInfo.advertisement_raw, null, 2)}
          </pre>
        </Card>
      )}
    </Screen>
  );
}