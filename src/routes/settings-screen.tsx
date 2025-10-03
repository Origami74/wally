import { Screen } from "@/components/layout/screen";
import { CopyButton } from "@/components/copy-button";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { ServiceStatus } from "@/lib/tollgate/types";
import { useNetworkDebugInfo } from "@/lib/tollgate/use-network-debug-info";
import type { NetworkDebugInfo } from "@/lib/tollgate/use-network-debug-info";

import type { FeatureState, Period, PeriodMetaFn } from "./types";
import { periods } from "./types";

type SettingsScreenProps = {
  status: ServiceStatus | null;
  features: FeatureState[];
  mintInput: string;
  npubInput: string;
  setMintInput: (value: string) => void;
  setNpubInput: (value: string) => void;
  savingMint: boolean;
  onSaveMint: () => void;
  onReset: () => void;
  handleFeatureUpdate: (
    id: FeatureState["id"],
    updater: (feature: FeatureState) => FeatureState
  ) => void;
  copyToClipboard: (value: string) => Promise<void> | void;
  periodMeta: PeriodMetaFn;
};

export function SettingsScreen({
  status,
  features,
  mintInput,
  npubInput,
  setMintInput,
  setNpubInput,
  savingMint,
  onSaveMint,
  onReset,
  handleFeatureUpdate,
  copyToClipboard,
  periodMeta,
}: SettingsScreenProps) {
  const { networkInfo } = useNetworkDebugInfo();

  return (
    <Screen className="min-h-screen gap-8 overflow-y-auto pt-6">

      <div className="grid gap-4">
        <h2 className="text-lg font-semibold uppercase tracking-[0.2em] text-muted-foreground pt-2">
          Wallet Settings
        </h2>
        {/* Spacer */}
        <div className="h-4"></div>
        <Card className="space-y-4 border border-dashed border-primary/20 bg-background/90 p-4">
          <div className="grid gap-3">
            <div className="grid gap-2">
              <Label htmlFor="mint-url">Mint URL</Label>
              <Input
                id="mint-url"
                value={mintInput}
                onChange={(event) => setMintInput(event.target.value)}
                placeholder="https://mint.example.com"
              />
            </div>
            <div className="grid gap-2">
              <Label htmlFor="wallet-npub">npub</Label>
              <Input
                id="wallet-npub"
                value={npubInput}
                onChange={(event) => setNpubInput(event.target.value)}
                placeholder="npub..."
              />
            </div>
            <div className="flex gap-2">
              <Button
                onClick={onSaveMint}
                disabled={!mintInput.trim() || savingMint}
              >
                {savingMint ? "Saving…" : "Save mint"}
              </Button>
              <Button variant="outline" onClick={onReset}>
                Reset
              </Button>
            </div>
          </div>
        </Card>
      </div>

      <div className="grid gap-4 pb-2">
        <h2 className="text-lg font-semibold uppercase tracking-[0.2em] text-muted-foreground">
          Features
        </h2>

        {features.map((feature) => (
          <Card
            key={feature.id}
            className="space-y-4 border border-dashed border-primary/20 bg-background/90 p-4"
          >
            <div className="flex items-start gap-3">
              <Checkbox
                id={`${feature.id}-checkbox`}
                checked={feature.enabled}
                onCheckedChange={() =>
                  handleFeatureUpdate(feature.id, (current) => ({
                    ...current,
                    enabled: !current.enabled,
                  }))
                }
                className="h-5 w-5 rounded-md border-border"
              />
              <div className="space-y-1">
                <Label
                  htmlFor={`${feature.id}-checkbox`}
                  className="text-base font-semibold"
                >
                  {feature.title}
                </Label>
                <p className="text-sm text-muted-foreground">
                  {feature.description}
                </p>
              </div>
              <Button
                variant="outline"
                size="sm"
                className="ml-auto h-auto rounded-full px-3 py-1 text-xs"
                onClick={() =>
                  handleFeatureUpdate(feature.id, (current) => ({
                    ...current,
                    enabled: !current.enabled,
                  }))
                }
              >
                {feature.enabled ? "Disable" : "Enable"}
              </Button>
            </div>

            <FeatureBudgetControls
              featureId={feature.id}
              budget={feature.budget}
              period={feature.period}
              spent={feature.spent}
              onBudgetChange={(value) =>
                handleFeatureUpdate(feature.id, (current) => ({
                  ...current,
                  budget: value,
                }))
              }
              onPeriodChange={(value) =>
                handleFeatureUpdate(feature.id, (current) => ({
                  ...current,
                  period: value,
                }))
              }
              periodMeta={periodMeta}
            />

            {feature.id === "nwc" ? (
              <CopyButton
                onCopy={() => copyToClipboard("nwc-example-123")}
                label="Copy NWC string"
                copiedLabel="Copied"
                variant="outline"
                className="border-dashed"
              />
            ) : null}

            <FeatureInfo
              featureId={feature.id}
              open={feature.infoOpen}
              onOpenChange={(open) =>
                handleFeatureUpdate(feature.id, (current) => ({
                  ...current,
                  infoOpen: open,
                }))
              }
              status={status}
              networkInfo={networkInfo}
            />
          </Card>
        ))}
      </div>
    </Screen>
  );
}

function FeatureBudgetControls({
  featureId,
  budget,
  period,
  spent,
  onBudgetChange,
  onPeriodChange,
  periodMeta,
}: {
  featureId: FeatureState["id"];
  budget: string;
  period: Period;
  spent: number;
  onBudgetChange: (value: string) => void;
  onPeriodChange: (value: Period) => void;
  periodMeta: PeriodMetaFn;
}) {
  return (
    <div className="rounded-2xl border border-dashed border-border/70 bg-muted/10 p-4 text-sm">
      <div className="grid gap-3 sm:grid-cols-[1fr_auto] sm:items-end">
        <div className="grid gap-2">
          <Label htmlFor={`${featureId}-budget`}>Budget</Label>
          <Input
            id={`${featureId}-budget`}
            type="number"
            min={0}
            value={budget}
            onChange={(event) => onBudgetChange(event.target.value)}
          />
        </div>
        <div className="grid gap-2">
          <Label htmlFor={`${featureId}-period`}>Per</Label>
          <Select
            value={period}
            onValueChange={(value) => onPeriodChange(value as Period)}
          >
            <SelectTrigger id={`${featureId}-period`}>
              <SelectValue placeholder="day" />
            </SelectTrigger>
            <SelectContent>
              {periods.map((option) => (
                <SelectItem key={option.value} value={option.value}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>
      <div className="mt-3 text-xs text-muted-foreground">
        Spent so far {periodMeta(period).human}: {spent} sats
      </div>
    </div>
  );
}

function FeatureInfo({
  featureId,
  open,
  onOpenChange,
  status,
  networkInfo,
}: {
  featureId: FeatureState["id"];
  open: boolean;
  onOpenChange: (open: boolean) => void;
  status: ServiceStatus | null;
  networkInfo: NetworkDebugInfo;
}) {
  const currentNetwork = status?.current_network ?? null;
  const fallbackAdvertisement = currentNetwork?.advertisement ?? null;

  const gatewayIp = networkInfo.gateway_ip ?? currentNetwork?.gateway_ip ?? null;
  const macAddress = networkInfo.mac_address ?? currentNetwork?.mac_address ?? null;
  const tollgatePubkey =
    networkInfo.tollgate_pubkey ?? fallbackAdvertisement?.tollgate_pubkey ?? null;
  const supportedTips =
    networkInfo.supported_tips.length > 0
      ? networkInfo.supported_tips
      : fallbackAdvertisement?.tips ?? [];
  const metricValue =
    networkInfo.metric ?? (fallbackAdvertisement?.metric != null
      ? String(fallbackAdvertisement.metric)
      : null);
  const stepSizeValue =
    networkInfo.step_size ?? (fallbackAdvertisement?.step_size != null
      ? String(fallbackAdvertisement.step_size)
      : null);
  const pricingOptions =
    networkInfo.pricing_options.length > 0
      ? networkInfo.pricing_options
      : (fallbackAdvertisement?.pricing_options ?? []).map((option) => ({
          mint_url: option.mint_url,
          price: String(option.price_per_step),
          unit: option.price_unit,
        }));

  return (
    <Collapsible open={open} onOpenChange={onOpenChange}>
      <CollapsibleTrigger asChild>
        <button className="text-left text-sm font-medium text-primary">
          More info {open ? "▲" : "▼"}
        </button>
      </CollapsibleTrigger>
      <CollapsibleContent className="mt-3 space-y-2 text-xs text-muted-foreground">
        {featureId === "tollgate" ? (
          <>
            <div>
              Network status: {networkInfo.is_tollgate ? "Tollgate detected" : "Standard network"}
            </div>
            <div>Gateway: {gatewayIp ?? "--"}</div>
            <div>MAC address: {macAddress ?? "--"}</div>
            {networkInfo.current_wifi ? (
              <>
                <div>WiFi SSID: {networkInfo.current_wifi.ssid}</div>
                <div>WiFi BSSID: {networkInfo.current_wifi.bssid}</div>
              </>
            ) : null}
            <div>Tollgate pubkey: {tollgatePubkey ?? "--"}</div>
            <div>
              Supported TIPs: {supportedTips.length ? supportedTips.join(", ") : "--"}
            </div>
            <div>Metric: {metricValue ?? "--"}</div>
            <div>Step size: {stepSizeValue ?? "--"}</div>
            {pricingOptions.length ? (
              <div className="space-y-1 pt-1">
                <div>Pricing options:</div>
                {pricingOptions.map((option, index) => (
                  <div key={`${option.mint_url}-${index}`} className="ml-4 text-[11px]">
                    {option.price} {option.unit || "sats"}
                    {option.mint_url ? ` via ${option.mint_url}` : ""}
                  </div>
                ))}
              </div>
            ) : null}
          </>
        ) : featureId === "402" ? (
          <>
            <div>Proxy endpoint: 402 mesh service</div>
            <div>
              Status: {status?.auto_tollgate_enabled ? "Active" : "Idle"}
            </div>
          </>
        ) : featureId === "routstr" ? (
          <>
            <div>Routes observed: 8</div>
            <div>Incentives pending: 320 sats</div>
          </>
        ) : (
          <>
            <div>NWC relay: wss://relay.example.com</div>
            <div>Allowance remaining: 640 sats</div>
          </>
        )}
      </CollapsibleContent>
    </Collapsible>
  );
}
