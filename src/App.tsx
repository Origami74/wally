import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PluginListener } from "@tauri-apps/api/core";
import { Settings2, Wallet } from "lucide-react";
import { Route, Switch, useLocation } from "wouter";

import { registerListener } from "@/lib/tollgate/network/pluginCommands";
import type { ServiceStatus } from "@/lib/tollgate/types";
import { statusTone } from "@/lib/tollgate/utils";
import { Button } from "@/components/ui/button";
import { HomeScreen } from "@/routes/home-screen";
import { ReceiveScreen } from "@/routes/receive-screen";
import { SendScreen } from "@/routes/send-screen";
import { SettingsScreen } from "@/routes/settings-screen";
import type { FeatureState, Period, StatusBadge } from "@/routes/types";
import { periods } from "@/routes/types";

type SessionInfo = ServiceStatus["active_sessions"][number];

const initialFeatures: FeatureState[] = [
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
    description: "Enable local Routstr proxy.",
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
];

export default function App() {
  const [location, setLocation] = useLocation();
  const [status, setStatus] = useState<ServiceStatus | null>(null);
  const [autoLoading, setAutoLoading] = useState(false);
  const [mintInput, setMintInput] = useState("");
  const [npubInput, setNpubInput] = useState("");
  const [savingMint, setSavingMint] = useState(false);
  const [sendRequest, setSendRequest] = useState("");
  const [features, setFeatures] = useState<FeatureState[]>(initialFeatures);

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

  const statusBadges: StatusBadge[] = useMemo(() => {
    const badges: StatusBadge[] = [];
    const tollgateState = currentSession
      ? String(currentSession.status)
      : currentNetwork?.is_tollgate
      ? "Available"
      : "Idle";

    badges.push({
      id: "tollgate",
      label: "Tollgate",
      value: tollgateState,
      tone: statusTone(tollgateState),
    });

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

  const mainClasses =
    location === "/settings"
      ? "relative mx-auto flex min-h-screen w-full max-w-md flex-col overflow-hidden bg-background px-4 pb-6 pt-6"
      : "relative mx-auto flex h-screen w-full max-w-md flex-col overflow-hidden bg-background px-4 pb-6 pt-6";

  const showHeaderButton = location === "/" || location === "/settings";
  const primaryNavAction = location === "/settings" ? goHome : goSettings;
  const primaryNavIcon = location === "/settings" ? <Wallet className="h-5 w-5" /> : <Settings2 className="h-5 w-5" />;

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
            <HomeScreen
              statusBadges={statusBadges}
              walletBalance={walletBalance}
              currentSession={currentSession}
              currentNetwork={currentNetwork}
              onReceive={goReceive}
              onSend={goSend}
            />
          </Route>

          <Route path="/receive">
            <ReceiveScreen
              onBack={goHome}
              onCopy={() => copyToClipboard("mesh-invoice-123")}
            />
          </Route>

          <Route path="/send">
            <SendScreen
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
            <SettingsScreen
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
