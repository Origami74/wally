import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PluginListener } from "@tauri-apps/api/core";
import { Copy, Settings2, Wallet } from "lucide-react";
import { Route, Switch, useLocation } from "wouter";

import { registerListener } from "@/lib/tollgate/network/pluginCommands";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";

const periods = [
  { value: "day", label: "day", human: "today" },
  { value: "week", label: "week", human: "this week" },
  { value: "month", label: "month", human: "this month" },
] as const;

const COPY_TIMEOUT_MS = 1500;

type Period = (typeof periods)[number]["value"];

type SessionStatusType = "Initializing" | "Active" | "Renewing" | "Expired" | "Error" | string;

type PricingOption = {
  asset_type: string;
  price_per_step: number;
  price_unit: string;
  mint_url: string;
  min_steps: number;
};

type TollgateAdvertisement = {
  metric: string;
  step_size: number;
  pricing_options: PricingOption[];
  tips: string[];
  tollgate_pubkey: string;
};

type NetworkInfo = {
  gateway_ip: string;
  mac_address: string;
  is_tollgate: boolean;
  advertisement?: TollgateAdvertisement | null;
};

type SessionInfo = {
  id: string;
  tollgate_pubkey: string;
  gateway_ip: string;
  status: SessionStatusType;
  usage_percentage: number;
  remaining_time_seconds: number | null;
  remaining_data_bytes: number | null;
  total_spent: number;
};

type ServiceStatus = {
  auto_tollgate_enabled: boolean;
  current_network: NetworkInfo | null;
  active_sessions: SessionInfo[];
  wallet_balance: number;
  last_check: string;
};

type FeatureState = {
  id: "tollgate" | "402" | "routstr" | "nwc";
  title: string;
  description: string;
  enabled: boolean;
  budget: string;
  period: Period;
  spent: number;
  infoOpen: boolean;
};

const formatBytes = (bytes?: number | null) => {
  if (!bytes || bytes <= 0) return "--";
  const sizes = ["B", "KB", "MB", "GB"];
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), sizes.length - 1);
  const value = bytes / Math.pow(1024, i);
  return `${value.toFixed(1)} ${sizes[i]}`;
};

const formatDuration = (seconds?: number | null) => {
  if (!seconds || seconds <= 0) return "--";
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${secs.toString().padStart(2, "0")}`;
};

const statusTone = (
  status: string
): "success" | "warning" | "danger" | "info" | "default" => {
  switch (status.toLowerCase()) {
    case "active":
      return "success";
    case "renewing":
      return "warning";
    case "expired":
    case "error":
      return "danger";
    case "available":
      return "info";
    default:
      return "default";
  }
};

const CopyButton = ({
  onCopy,
  label,
  copiedLabel,
  disabled,
}: {
  onCopy: () => Promise<void> | void;
  label: string;
  copiedLabel: string;
  disabled?: boolean;
}) => {
  const [copied, setCopied] = useState(false);

  const handleClick = async () => {
    await onCopy();
    setCopied(true);
    setTimeout(() => setCopied(false), COPY_TIMEOUT_MS);
  };

  return (
    <Button
      onClick={handleClick}
      disabled={disabled}
      className="flex items-center justify-center gap-2"
    >
      <Copy className="h-4 w-4" /> {copied ? copiedLabel : label}
    </Button>
  );
};

export default function App() {
  const [location, setLocation] = useLocation();
  const [status, setStatus] = useState<ServiceStatus | null>(null);
  const [autoLoading, setAutoLoading] = useState(false);
  const [mintInput, setMintInput] = useState("");
  const [npubInput, setNpubInput] = useState("");
  const [savingMint, setSavingMint] = useState(false);
  const [sendRequest, setSendRequest] = useState("");
  const [features, setFeatures] = useState<FeatureState[]>([
    {
      id: "tollgate",
      title: "Tollgate",
      description: "Automatically maintain Tollgate connectivity when available.",
      enabled: true,
      budget: "5000",
      period: "day",
      spent: 0,
      infoOpen: false,
    },
    {
      id: "402",
      title: "402",
      description: "Handle 402 payment required requests.",
      enabled: false,
      budget: "1000",
      period: "day",
      spent: 0,
      infoOpen: false,
    },
    {
      id: "routstr",
      title: "Routstr",
      description: "Enable local Roustr proxy.",
      enabled: false,
      budget: "2000",
      period: "week",
      spent: 0,
      infoOpen: false,
    },
    {
      id: "nwc",
      title: "NWC",
      description: "Enable Nostr Wallet Connect connection.",
      enabled: false,
      budget: "800",
      period: "day",
      spent: 0,
      infoOpen: false,
    },
  ]);

  const periodMeta = useCallback(
    (period: Period) => periods.find(item => item.value === period) ?? periods[0],
    []
  );

  const refreshStatus = useCallback(async () => {
    try {
      const result = await invoke<ServiceStatus>("get_tollgate_status");
      setStatus(result);
      if (!mintInput) {
        const fallbackMint = result.current_network?.advertisement?.pricing_options?.[0]?.mint_url ?? "";
        if (fallbackMint) setMintInput(fallbackMint);
      }
      if (!npubInput) {
        const fallbackNpub = result.current_network?.advertisement?.tollgate_pubkey ?? "";
        if (fallbackNpub) setNpubInput(fallbackNpub);
      }
      setFeatures(prev =>
        prev.map(feature =>
          feature.id === "tollgate" && result.active_sessions?.[0]
            ? { ...feature, spent: result.active_sessions[0].total_spent }
            : feature
        )
      );
    } catch (error) {
      console.error("Failed to refresh tollgate status", error);
    }
  }, [mintInput, npubInput]);

  useEffect(() => {
    let mounted = true;
    const listeners: PluginListener[] = [];

    const initialise = async () => {
      await refreshStatus();
      try {
        const connected = await registerListener("network-connected", async () => {
          if (!mounted) return;
          await refreshStatus();
        });
        listeners.push(connected);
        const disconnected = await registerListener("network-disconnected", async () => {
          if (!mounted) return;
          await refreshStatus();
        });
        listeners.push(disconnected);
      } catch (error) {
        console.warn("Failed to register androidwifi listeners", error);
      }
    };

    initialise();
    const interval = setInterval(refreshStatus, 5_000);

    return () => {
      mounted = false;
      clearInterval(interval);
      listeners.forEach(listener => listener.remove());
    };
  }, [refreshStatus]);

  const toggleAutoTollgate = useCallback(async () => {
    if (!status) return;
    setAutoLoading(true);
    try {
      await invoke("toggle_auto_tollgate", { enabled: !status.auto_tollgate_enabled });
      await refreshStatus();
    } catch (error) {
      console.error("Failed to toggle auto tollgate", error);
    } finally {
      setAutoLoading(false);
    }
  }, [status, refreshStatus]);

  const saveMintUrl = useCallback(async () => {
    if (!mintInput.trim()) return;
    setSavingMint(true);
    try {
      await invoke("add_mint", { mintUrl: mintInput.trim() });
    } catch (error) {
      console.error("Failed to add mint", error);
    } finally {
      setSavingMint(false);
    }
  }, [mintInput]);

  const handleFeatureUpdate = useCallback(
    (id: FeatureState["id"], updater: (feature: FeatureState) => FeatureState) => {
      setFeatures(prev => prev.map(feature => (feature.id === id ? updater(feature) : feature)));
    },
    []
  );

  const copyToClipboard = useCallback(async (value: string) => {
    try {
      await navigator.clipboard?.writeText(value);
    } catch (error) {
      console.warn("Copy failed", error);
    }
  }, []);

  const walletBalance = status?.wallet_balance ?? 0;
  const currentSession = status?.active_sessions?.[0] ?? null;
  const currentNetwork = status?.current_network ?? null;

  const statusBadges = useMemo(() => {
    const badges: { id: string; label: string; value: string; tone: React.ComponentProps<typeof Badge>["tone"] }[] = [];
    const tollgateState = currentSession
      ? String(currentSession.status)
      : currentNetwork?.is_tollgate
      ? "Available"
      : "Idle";
    badges.push({ id: "tollgate", label: "Tollgate", value: tollgateState, tone: statusTone(tollgateState) });
    const featureState = (featureId: FeatureState["id"]) =>
      features.find(feature => feature.id === featureId)?.enabled ? "Enabled" : "Idle";
    badges.push({
      id: "402",
      label: "402",
      value: featureState("402"),
      tone: features.find(feature => feature.id === "402")?.enabled ? "info" : "default",
    });
    badges.push({
      id: "nwc",
      label: "NWC",
      value: featureState("nwc"),
      tone: features.find(feature => feature.id === "nwc")?.enabled ? "info" : "default",
    });
    return badges;
  }, [currentSession, currentNetwork, features]);

  const goHome = () => setLocation("/");
  const goReceive = () => setLocation("/receive");
  const goSend = () => setLocation("/send");
  const goSettings = () => setLocation("/settings");

  const primaryNavIcon = location === "/settings" ? <Wallet className="h-5 w-5" /> : <Settings2 className="h-5 w-5" />;
  const primaryNavAction = location === "/settings" ? goHome : goSettings;

  const mainClasses =
    location === "/settings"
      ? "relative mx-auto flex min-h-screen w-full max-w-md flex-col overflow-hidden bg-background px-4 pb-6 pt-6"
      : "relative mx-auto flex h-screen w-full max-w-md flex-col overflow-hidden bg-background px-4 pb-6 pt-6";

  const showHeaderButton = location === "/" || location === "/settings";

  return (
    <div className="bg-background text-foreground" style={{ overscrollBehavior: "none" }}>
      <main className={mainClasses}>
        {showHeaderButton ? (
          <div className="absolute right-5 top-5">
            <Button
              variant="outline"
              size="icon"
              className="h-10 w-10 rounded-full"
              onClick={primaryNavAction}
              aria-label={location === "/settings" ? "Back to wallet" : "Open settings"}
            >
              {primaryNavIcon}
            </Button>
          </div>
        ) : null}

        <Switch>
          <Route path="/">
            <HomeView
              statusBadges={statusBadges}
              walletBalance={walletBalance}
              currentSession={currentSession}
              currentNetwork={currentNetwork}
              goReceive={goReceive}
              goSend={goSend}
            />
          </Route>

          <Route path="/receive">
            <ReceiveView
              onBack={goHome}
              onCopy={() => copyToClipboard("mesh-invoice-123")}
            />
          </Route>

          <Route path="/send">
            <SendView
              onBack={goHome}
              request={sendRequest}
              onChangeRequest={setSendRequest}
              onSubmit={() => {
                // TODO integrate real send logic
                setSendRequest("");
                goHome();
              }}
            />
          </Route>

          <Route path="/settings">
            <SettingsView
              status={status}
              features={features}
              mintInput={mintInput}
              npubInput={npubInput}
              setMintInput={setMintInput}
              setNpubInput={setNpubInput}
              savingMint={savingMint}
              onSaveMint={saveMintUrl}
              onReset={() => {
                setMintInput(status?.current_network?.advertisement?.pricing_options?.[0]?.mint_url ?? "");
                setNpubInput(status?.current_network?.advertisement?.tollgate_pubkey ?? "");
              }}
              autoLoading={autoLoading}
              toggleAutoTollgate={toggleAutoTollgate}
              handleFeatureUpdate={handleFeatureUpdate}
              copyToClipboard={copyToClipboard}
              periodMeta={periodMeta}
            />
          </Route>
        </Switch>
      </main>
    </div>
  );
}

type HomeViewProps = {
  statusBadges: { id: string; label: string; value: string; tone: React.ComponentProps<typeof Badge>["tone"] }[];
  walletBalance: number;
  currentSession: SessionInfo | null;
  currentNetwork: NetworkInfo | null;
  goReceive: () => void;
  goSend: () => void;
};

const HomeView = ({ statusBadges, walletBalance, currentSession, currentNetwork, goReceive, goSend }: HomeViewProps) => {
  return (
    <section className="flex h-full flex-col gap-8">
      <div className="flex flex-col gap-2 absolute top-5 left-4">
        {statusBadges.map(badge => (
          <Badge key={badge.id} tone={badge.tone} className="w-max px-4 py-1.5 text-xs">
            <span className="font-medium uppercase tracking-wide">{badge.label}</span>
            <span className="ml-2 text-xs capitalize">{badge.value.toLowerCase()}</span>
          </Badge>
        ))}
      </div>
      {/* Spacer about the height of the buttons */}
      <div className="h-16"></div>

      <div className="text-center flex-1 flex flex-col justify-center">
        <div className="text-4xl font-semibold leading-tight text-primary">
          {walletBalance.toLocaleString()} <span className="font-normal">sats</span>
          {/* 10,000 <span className="font-normal">sats</span> */}
        </div>
      </div>

      {currentSession ? (
        <Card className="space-y-4 border border-dashed border-primary/30 bg-background/80 p-4">
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <span className="uppercase tracking-wide">Session usage</span>
            <span className="font-semibold text-primary">{Math.round(currentSession.usage_percentage)}%</span>
          </div>
          <div className="h-2 rounded-full bg-muted">
            <div
              className="h-full rounded-full bg-primary"
              style={{ width: `${Math.min(100, Math.round(currentSession.usage_percentage))}%` }}
            />
          </div>
          <div className="grid grid-cols-2 gap-3 text-xs text-muted-foreground">
            <div>
              <span className="block text-[10px] uppercase tracking-wide">Time left</span>
              <span className="text-sm font-medium text-foreground">{formatDuration(currentSession.remaining_time_seconds)}</span>
            </div>
            <div className="text-right">
              <span className="block text-[10px] uppercase tracking-wide">Data remaining</span>
              <span className="text-sm font-medium text-foreground">{formatBytes(currentSession.remaining_data_bytes)}</span>
            </div>
          </div>
        </Card>
      ) : null}

      {currentNetwork ? (
        <Card className="space-y-3 border border-dashed border-primary/20 bg-background/90 p-4 text-xs text-muted-foreground">
          <div className="flex items-center justify-between text-foreground">
            <span className="uppercase tracking-wide">Network</span>
            <Badge tone={currentNetwork.is_tollgate ? "success" : "default"}>
              {currentNetwork.is_tollgate ? "Tollgate" : "Standard"}
            </Badge>
          </div>
          <div className="grid gap-1">
            <span>Gateway: {currentNetwork.gateway_ip}</span>
            <span>MAC: {currentNetwork.mac_address}</span>
          </div>
        </Card>
      ) : null}

      <div className="flex gap-3 pb-2">
        <Button onClick={goReceive} variant="outline" className="flex-1 py-5 text-base font-semibold">
          Receive
        </Button>
        <Button onClick={goSend} variant="outline" className="flex-1 py-5 text-base font-semibold">
          Send
        </Button>
      </div>
    </section>
  );
};

type ReceiveViewProps = {
  onBack: () => void;
  onCopy: () => Promise<void> | void;
};

const ReceiveView = ({ onBack, onCopy }: ReceiveViewProps) => {
  return (
    <section className="flex h-screen flex-col gap-6 overflow-hidden">
      <div className="text-left">
        <h1 className="text-3xl font-semibold">Receive</h1>
      </div>

      <div className="flex-1 py-2">
        <div className="grid gap-6">
          <div className="mx-auto grid h-56 w-56 place-items-center rounded-3xl border-2 border-dashed border-primary/40 bg-muted text-xs font-medium text-muted-foreground">
            {/* TODO mock data: replace with real QR code */}
            QR preview
          </div>
          <div className="grid gap-2">
            <Label htmlFor="receive-amount">Optional amount (sats)</Label>
            <Input id="receive-amount" type="number" min={0} placeholder="Add an amount" />
          </div>
        </div>
      </div>

      <div className="mt-auto grid gap-2 pb-2">
        <CopyButton onCopy={onCopy} label="Copy invoice" copiedLabel="Copied" />
        <Button variant="outline" onClick={onBack}>
          Cancel
        </Button>
      </div>
    </section>
  );
};

type SendViewProps = {
  onBack: () => void;
  request: string;
  onChangeRequest: (value: string) => void;
  onSubmit: () => void;
};

const SendView = ({ onBack, request, onChangeRequest, onSubmit }: SendViewProps) => {
  const canSend = request.trim().length > 0;
  return (
    <section className="flex h-screen flex-col gap-6 overflow-hidden">
      <div className="text-left">
        <h1 className="text-3xl font-semibold">Send</h1>
      </div>

      <div className="flex-1 py-2">
        <div className="grid gap-2">
          <Label htmlFor="send-request">Payment request</Label>
          <Textarea
            id="send-request"
            placeholder="Paste payment request here"
            value={request}
            onChange={event => onChangeRequest(event.target.value)}
          />
        </div>
      </div>

      <div className="mt-auto grid gap-2 pb-2">
        <Button onClick={onSubmit} disabled={!canSend}>
          Send payment
        </Button>
        <Button variant="outline" onClick={onBack}>
          Cancel
        </Button>
      </div>
    </section>
  );
};

type SettingsViewProps = {
  status: ServiceStatus | null;
  features: FeatureState[];
  mintInput: string;
  npubInput: string;
  setMintInput: (value: string) => void;
  setNpubInput: (value: string) => void;
  savingMint: boolean;
  onSaveMint: () => void;
  onReset: () => void;
  autoLoading: boolean;
  toggleAutoTollgate: () => void;
  handleFeatureUpdate: (id: FeatureState["id"], updater: (feature: FeatureState) => FeatureState) => void;
  copyToClipboard: (value: string) => Promise<void> | void;
  periodMeta: (period: Period) => (typeof periods)[number];
};

const SettingsView = ({
  status,
  features,
  mintInput,
  npubInput,
  setMintInput,
  setNpubInput,
  savingMint,
  onSaveMint,
  onReset,
  autoLoading,
  toggleAutoTollgate,
  handleFeatureUpdate,
  copyToClipboard,
  periodMeta,
}: SettingsViewProps) => {
  return (
    <section className="flex flex-1 flex-col gap-8 pb-4">
      <div className="grid gap-4">
        <h2 className="text-lg font-semibold uppercase tracking-[0.2em] text-muted-foreground">
          Wallet Settings
        </h2>
        <div className="grid gap-3">
          <div className="grid gap-2">
            <Label htmlFor="mint-url">Mint URL</Label>
            <Input id="mint-url" value={mintInput} onChange={event => setMintInput(event.target.value)} placeholder="https://mint.example.com" />
          </div>
          <div className="grid gap-2">
            <Label htmlFor="wallet-npub">npub</Label>
            <Input id="wallet-npub" value={npubInput} onChange={event => setNpubInput(event.target.value)} placeholder="npub..." />
          </div>
          <div className="flex gap-2">
            <Button onClick={onSaveMint} disabled={!mintInput.trim() || savingMint}>
              {savingMint ? "Saving…" : "Save mint"}
            </Button>
            <Button variant="ghost" onClick={onReset}>
              Reset
            </Button>
          </div>
        </div>
      </div>

      <div className="grid gap-4">
        <h2 className="text-lg font-semibold uppercase tracking-[0.2em] text-muted-foreground">
          Features
        </h2>

        <div className="space-y-4 pb-2">
          {features.map(feature => (
            <Card key={feature.id} className="space-y-4 border border-dashed border-primary/20 bg-background/90 p-4">
              <div className="flex items-start gap-3">
                <Checkbox
                  id={`${feature.id}-checkbox`}
                  checked={feature.enabled}
                  onCheckedChange={() =>
                    handleFeatureUpdate(feature.id, current => ({
                      ...current,
                      enabled: !current.enabled,
                    }))
                  }
                  className="h-5 w-5 rounded-md border-border"
                />
                <div className="space-y-1">
                  <Label htmlFor={`${feature.id}-checkbox`} className="text-base font-semibold">
                    {feature.title}
                  </Label>
                  <p className="text-sm text-muted-foreground">{feature.description}</p>
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  className="ml-auto h-auto rounded-full border border-border px-3 py-1 text-xs"
                  onClick={() =>
                    handleFeatureUpdate(feature.id, current => ({
                      ...current,
                      enabled: !current.enabled,
                    }))
                  }
                >
                  {feature.enabled ? "Disable" : "Enable"}
                </Button>
              </div>

              <div className="rounded-2xl border border-dashed border-border/70 bg-muted/10 p-4 text-sm">
                <div className="grid gap-3 sm:grid-cols-[1fr_auto] sm:items-end">
                  <div className="grid gap-2">
                    <Label htmlFor={`${feature.id}-budget`}>Budget</Label>
                    <Input
                      id={`${feature.id}-budget`}
                      type="number"
                      min={0}
                      value={feature.budget}
                      onChange={event =>
                        handleFeatureUpdate(feature.id, current => ({
                          ...current,
                          budget: event.target.value,
                        }))
                      }
                    />
                  </div>
                  <div className="grid gap-2">
                    <Label htmlFor={`${feature.id}-period`}>Per</Label>
                    <Select
                      value={feature.period}
                      onValueChange={value =>
                        handleFeatureUpdate(feature.id, current => ({
                          ...current,
                          period: value as Period,
                        }))
                      }
                    >
                      <SelectTrigger id={`${feature.id}-period`}>
                        <SelectValue placeholder="day" />
                      </SelectTrigger>
                      <SelectContent>
                        {periods.map(period => (
                          <SelectItem key={period.value} value={period.value}>
                            {period.label}
                          </SelectItem>
                        ))}
                      </SelectContent>
                    </Select>
                  </div>
                </div>
                <div className="mt-3 text-xs text-muted-foreground">
                  Spent so far {periodMeta(feature.period).human}: {feature.spent} sats
                </div>
              </div>

              {feature.id === "nwc" ? (
                <Button
                  variant="ghost"
                  className="flex w-full items-center justify-center gap-2 border border-dashed border-border py-2 text-sm"
                  onClick={() => copyToClipboard("nwc-example-123")}
                >
                  <Copy className="h-4 w-4" /> Copy NWC string
                </Button>
              ) : null}

              <Collapsible
                open={feature.infoOpen}
                onOpenChange={open =>
                  handleFeatureUpdate(feature.id, current => ({
                    ...current,
                    infoOpen: open,
                  }))
                }
              >
                <CollapsibleTrigger asChild>
                  <button className="text-left text-sm font-medium text-primary">
                    More info {feature.infoOpen ? "▲" : "▼"}
                  </button>
                </CollapsibleTrigger>
                <CollapsibleContent className="mt-3 space-y-2 text-xs text-muted-foreground">
                  {feature.id === "tollgate" ? (
                    <>
                      <div>Gateway: {status?.current_network?.gateway_ip ?? "--"}</div>
                      <div>MAC address: {status?.current_network?.mac_address ?? "--"}</div>
                      <div>Tollgate pubkey: {status?.current_network?.advertisement?.tollgate_pubkey ?? "--"}</div>
                      <div>Supported TIPs: {status?.current_network?.advertisement?.tips.join(", ") ?? "--"}</div>
                    </>
                  ) : feature.id === "402" ? (
                    <>
                      <div>Proxy endpoint: 402 mesh service</div>
                      <div>Status: {feature.enabled ? "Active" : "Idle"}</div>
                    </>
                  ) : feature.id === "routstr" ? (
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
            </Card>
          ))}
        </div>
      </div>
    </section>
  );
};
