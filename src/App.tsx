import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PluginListener } from "@tauri-apps/api/core";
import { History, Settings2, Wallet, Bug } from "lucide-react";
import { Route, Switch, useLocation } from "wouter";

import { registerListener } from "@/lib/tollgate/network/pluginCommands";
import type { ServiceStatus } from "@/lib/tollgate/types";
import { statusTone } from "@/lib/tollgate/utils";
import { Button } from "@/components/ui/button";
import { HomeScreen } from "@/routes/home-screen";
import { ReceiveScreen } from "@/routes/receive-screen";
import { SendScreen } from "@/routes/send-screen";
import { SettingsScreen } from "@/routes/settings-screen";
import { DebugScreen } from "@/routes/debug-screen";
import type { FeatureState, Period, StatusBadge } from "@/routes/types";
import { periods } from "@/routes/types";
import { HistoryScreen } from "@/routes/history-screen";
import {
  fetchWalletSummary,
  fetchWalletTransactions,
  type WalletSummary,
  type WalletTransactionEntry,
} from "@/lib/wallet/api";

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
  const [walletSummary, setWalletSummary] = useState<WalletSummary | null>(null);
  const [transactions, setTransactions] = useState<WalletTransactionEntry[]>([]);
  const [mintInput, setMintInput] = useState("");
  const [npubInput, setNpubInput] = useState("");
  const [savingMint, setSavingMint] = useState(false);
  const [sendRequest, setSendRequest] = useState("");
  const [features, setFeatures] = useState<FeatureState[]>(initialFeatures);

  const periodMeta = useCallback(
    (period: Period) =>
      periods.find((item) => item.value === period) ?? periods[0],
    []
  );

  const refreshStatus = useCallback(async () => {
    try {
      const [statusResult, summaryResult, transactionsResult] = await Promise.all([
        invoke<ServiceStatus>("get_tollgate_status"),
        fetchWalletSummary(),
        fetchWalletTransactions(),
      ]);

      setStatus(statusResult);
      setWalletSummary(summaryResult);
      setTransactions(transactionsResult);

      if (!mintInput) {
        if (summaryResult.default_mint) {
          setMintInput(summaryResult.default_mint);
        } else {
          const fallbackMint =
            statusResult.current_network?.advertisement?.pricing_options?.[0]
              ?.mint_url ?? "";
          if (fallbackMint) setMintInput(fallbackMint);
        }
      }

      if (!npubInput) {
        if (summaryResult.npub) {
          setNpubInput(summaryResult.npub);
        } else {
          const fallbackNpub =
            statusResult.current_network?.advertisement?.tollgate_pubkey ?? "";
          if (fallbackNpub) setNpubInput(fallbackNpub);
        }
      }

      setFeatures((prev) =>
        prev.map((feature) =>
          feature.id === "tollgate" && statusResult.active_sessions?.[0]
            ? { ...feature, spent: statusResult.active_sessions[0].total_spent }
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
        const connected = await registerListener(
          "network-connected",
          async () => {
            if (!mounted) return;
            await refreshStatus();
          }
        );
        listeners.push(connected);

        const disconnected = await registerListener(
          "network-disconnected",
          async () => {
            if (!mounted) return;
            await refreshStatus();
          }
        );
        listeners.push(disconnected);

        // Listen for network status changes from the new monitoring system
        const networkStatusChanged = await registerListener(
          "network-status-changed",
          async (networkStatus: any) => {
            if (!mounted) return;
            console.log("Network status changed:", networkStatus);
            await refreshStatus();
          }
        );
        listeners.push(networkStatusChanged);

        // Listen for tollgate detection events
        const tollgateDetected = await registerListener(
          "tollgate-detected",
          async (tollgateInfo: any) => {
            if (!mounted) return;
            console.log("Tollgate detected:", tollgateInfo);
            await refreshStatus();
          }
        );
        listeners.push(tollgateDetected);
      } catch (error) {
        console.warn("Failed to register androidwifi listeners", error);
      }
    };

    initialise();
    const interval = setInterval(refreshStatus, 5_000);

    return () => {
      mounted = false;
      clearInterval(interval);
      listeners.forEach((listener) => listener.unregister());
    };
  }, [refreshStatus]);

  const saveMintUrl = useCallback(async () => {
    if (!mintInput.trim()) return;
    setSavingMint(true);
    try {
      await invoke("add_mint", { mintUrl: mintInput.trim() });
      await refreshStatus();
    } catch (error) {
      console.error("Failed to add mint", error);
    } finally {
      setSavingMint(false);
    }
  }, [mintInput, refreshStatus]);

  const handleFeatureUpdate = useCallback(
    (
      id: FeatureState["id"],
      updater: (feature: FeatureState) => FeatureState
    ) => {
      setFeatures((prev) =>
        prev.map((feature) => (feature.id === id ? updater(feature) : feature))
      );
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

  const walletBalance =
    walletSummary?.total ?? status?.wallet_balance ?? 0;
  const currentSession = status?.active_sessions?.[0] ?? null;
  const currentNetwork = status?.current_network ?? null;

  const handlePaymentComplete = useCallback(async () => {
    setSendRequest("");
    await refreshStatus();
    setLocation("/");
  }, [refreshStatus, setLocation]);

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
      features.find((feature) => feature.id === featureId)?.enabled
        ? "Enabled"
        : "Idle";

    badges.push({
      id: "402",
      label: "402",
      value: featureState("402"),
      tone: features.find((feature) => feature.id === "402")?.enabled
        ? "info"
        : "default",
    });

    badges.push({
      id: "nwc",
      label: "NWC",
      value: featureState("nwc"),
      tone: features.find((feature) => feature.id === "nwc")?.enabled
        ? "info"
        : "default",
    });

    return badges;
  }, [currentSession, currentNetwork, features]);

  const goHome = () => setLocation("/");
  const goReceive = () => setLocation("/receive");
  const goSend = () => setLocation("/send");
  const goSettings = () => setLocation("/settings");
  const goHistory = () => setLocation("/history");
  const goDebug = () => setLocation("/debug");

  const sharedMainClasses =
    "relative mx-auto flex w-full max-w-md flex-col overflow-hidden bg-background";

  const mainClasses =
    location === "/settings" || location === "/history" || location === "/debug"
      ? `${sharedMainClasses} min-h-screen`
      : `${sharedMainClasses} h-screen`;

  const showSettingsButton =
    location === "/" || location === "/settings" || location === "/history" || location === "/debug";
  const showHistoryButton =
    location === "/" || location === "/settings" || location === "/history" || location === "/debug";
  const showDebugButton =
    location === "/" || location === "/settings" || location === "/history" || location === "/debug";

  const settingsButtonAction = location === "/settings" ? goHome : goSettings;
  const historyButtonAction = location === "/history" ? goHome : goHistory;
  const debugButtonAction = location === "/debug" ? goHome : goDebug;

  const settingsButtonIcon =
    location === "/settings" ? (
      <Wallet className="h-5 w-5" />
    ) : (
      <Settings2 className="h-5 w-5" />
    );

  const historyButtonIcon =
    location === "/history" ? (
      <Wallet className="h-5 w-5" />
    ) : (
      <History className="h-5 w-5" />
    );

  const debugButtonIcon =
    location === "/debug" ? (
      <Wallet className="h-5 w-5" />
    ) : (
      <Bug className="h-5 w-5" />
    );

  return (
    <div
      className="bg-background text-foreground"
      style={{ overscrollBehavior: "none" }}
    >
      <main className={mainClasses}>
        {showSettingsButton || showHistoryButton || showDebugButton ? (
          <div className="absolute right-4 top-4 z-20 flex flex-col items-end gap-2">
            {showDebugButton ? (
              <Button
                variant="outline"
                size="icon"
                className="h-10 w-10 rounded-full"
                onClick={debugButtonAction}
                aria-label={
                  location === "/debug" ? "Back to wallet" : "Open debug"
                }
              >
                {debugButtonIcon}
              </Button>
            ) : null}
            {showSettingsButton ? (
              <Button
                variant="outline"
                size="icon"
                className="h-10 w-10 rounded-full"
                onClick={settingsButtonAction}
                aria-label={
                  location === "/settings" ? "Back to wallet" : "Open settings"
                }
              >
                {settingsButtonIcon}
              </Button>
            ) : null}
            {showHistoryButton ? (
              <Button
                variant="outline"
                size="icon"
                className="h-10 w-10 rounded-full"
                onClick={historyButtonAction}
                aria-label={
                  location === "/history" ? "Back to wallet" : "Open history"
                }
              >
                {historyButtonIcon}
              </Button>
            ) : null}
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
              copyToClipboard={copyToClipboard}
              defaultMint={walletSummary?.default_mint ?? ""}
            />
          </Route>

          <Route path="/send">
            <SendScreen
              onBack={goHome}
              request={sendRequest}
              onChangeRequest={setSendRequest}
              onPaymentComplete={handlePaymentComplete}
            />
          </Route>

          <Route path="/settings">
            <SettingsScreen
              status={status}
              summary={walletSummary}
              features={features}
              mintInput={mintInput}
              npubInput={npubInput}
              setMintInput={setMintInput}
              setNpubInput={setNpubInput}
              savingMint={savingMint}
              onSaveMint={saveMintUrl}
              onReset={() => {
                if (walletSummary?.default_mint) {
                  setMintInput(walletSummary.default_mint);
                } else {
                  setMintInput(
                    status?.current_network?.advertisement?.pricing_options?.[0]
                      ?.mint_url ?? ""
                  );
                }
                if (walletSummary?.npub) {
                  setNpubInput(walletSummary.npub);
                } else {
                  setNpubInput(
                    status?.current_network?.advertisement?.tollgate_pubkey ?? ""
                  );
                }
              }}
              handleFeatureUpdate={handleFeatureUpdate}
              copyToClipboard={copyToClipboard}
              periodMeta={periodMeta}
            />
          </Route>

          <Route path="/history">
            <HistoryScreen transactions={transactions} />
          </Route>

          <Route path="/debug">
            <DebugScreen
              status={status}
              copyToClipboard={copyToClipboard}
            />
          </Route>
        </Switch>
      </main>
    </div>
  );
}
