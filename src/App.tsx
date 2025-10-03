import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { History, Settings2, Wallet, Bug, Link } from "lucide-react";
import { Route, Switch, useLocation } from "wouter";
import type { ServiceStatus } from "@/lib/tollgate/types";
import { statusTone } from "@/lib/tollgate/utils";
import { Button } from "@/components/ui/button";
import { HomeScreen } from "@/routes/home-screen";
import { ReceiveScreen } from "@/routes/receive-screen";
import { SendScreen } from "@/routes/send-screen";
import { SettingsScreen } from "@/routes/settings-screen";
import { DebugScreen } from "@/routes/debug-screen";
import { ConnectionsScreen } from "@/routes/connections-screen";
import type { FeatureState, Period, StatusBadge } from "@/routes/types";
import { periods } from "@/routes/types";
import { HistoryScreen } from "@/routes/history-screen";
import {
  fetchWalletSummary,
  fetchWalletTransactions,
  type WalletSummary,
  type WalletTransactionEntry,
} from "@/lib/wallet/api";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

type PendingConnectionRequest = {
  request_id: string;
  nwa_request: {
    app_pubkey: string;
    relays: string[];
    secret: string;
    required_commands: string[];
    optional_commands: string[];
    budget: string | null;
    identity: string | null;
  };
  received_at: number;
};

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
  const [pendingConnection, setPendingConnection] = useState<PendingConnectionRequest | null>(null);

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
            statusResult.current_network?.advertisement?.pricing_options?.[0]?.mint_url ?? "";
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
    const listeners: UnlistenFn[] = [];

    const initialise = async () => {
      await refreshStatus();
      try {
        const connected = await listen(
          "network-connected",
          async () => {
            if (!mounted) return;
            await refreshStatus();
          }
        );
        listeners.push(connected);

        const disconnected = await listen(
          "network-disconnected",
          async () => {
            if (!mounted) return;
            await refreshStatus();
          }
        );
        listeners.push(disconnected);

        const networkStatusChanged = await listen(
          "network-status-changed",
          async (event: any) => {
            if (!mounted) return;
            console.log("App: Network status changed:", event.payload);
            await refreshStatus();
          }
        );
        listeners.push(networkStatusChanged);

        const tollgateDetected = await listen(
          "tollgate-detected",
          async (event: any) => {
            if (!mounted) return;
            console.log("App: Tollgate detected:", event.payload);
            await refreshStatus();
          }
        );
        listeners.push(tollgateDetected);

        const nwcConnectionRequest = await listen(
          "nwc-connection-request",
          async (event: any) => {
            if (!mounted) return;
            console.log("App: NWC connection request received:", event.payload);
            setPendingConnection(event.payload as PendingConnectionRequest);
          }
        );
        listeners.push(nwcConnectionRequest);
      } catch (error) {
        console.warn("Failed to register androidwifi listeners", error);
      }
    };

    initialise();
    const interval = setInterval(refreshStatus, 5_000);

    return () => {
      mounted = false;
      clearInterval(interval);
      listeners.forEach((listener) => listener());
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

  const walletBalance = walletSummary?.total ?? status?.wallet_balance ?? 0;
  const currentSession = status?.active_sessions?.[0] ?? null;
  const currentNetwork = status?.current_network ?? null;

  const handlePaymentComplete = useCallback(async () => {
    setSendRequest("");
    await refreshStatus();
    setLocation("/");
  }, [refreshStatus, setLocation]);

  const handleApproveConnection = async () => {
    if (!pendingConnection) return;

    try {
      await invoke("nwc_approve_connection", { requestId: pendingConnection.request_id });
      setPendingConnection(null);
      await refreshStatus();
      setLocation("/");
    } catch (error) {
      console.error("Failed to approve connection:", error);
      alert(`Failed to approve connection: ${error}`);
      setPendingConnection(null);
      setLocation("/");
    }
  };

  const handleRejectConnection = async () => {
    if (!pendingConnection) return;

    try {
      await invoke("nwc_reject_connection", { requestId: pendingConnection.request_id });
      setPendingConnection(null);
      setLocation("/");
    } catch (error) {
      console.error("Failed to reject connection:", error);
      setPendingConnection(null);
      setLocation("/");
    }
  };

  const statusBadges: StatusBadge[] = useMemo(() => {
    const badges: StatusBadge[] = [];

    const tollgateFeatureEnabled = features.find((feature) => feature.id === "tollgate")?.enabled;
    if (tollgateFeatureEnabled) {
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
    }

    const featureEnabled = (featureId: FeatureState["id"]) =>
      features.find((feature) => feature.id === featureId)?.enabled ?? false;

    if (featureEnabled("402")) {
      badges.push({
        id: "402",
        label: "402",
        value: "Enabled",
        tone: "info",
      });
    }

    if (featureEnabled("nwc")) {
      badges.push({
        id: "nwc",
        label: "NWC",
        value: "Enabled",
        tone: "info",
      });
    }

    return badges;
  }, [currentSession, currentNetwork, features]);

  const goHome = () => setLocation("/");
  const goReceive = () => setLocation("/receive");
  const goSend = () => setLocation("/send");
  const goSettings = () => setLocation("/settings");
  const goHistory = () => setLocation("/history");
  const goDebug = () => setLocation("/debug");
  const goConnections = () => setLocation("/connections");

  const sharedMainClasses =
    "relative mx-auto flex w-full max-w-md flex-col overflow-hidden bg-background";

  const mainClasses =
    location === "/settings" ||
    location === "/history" ||
    location === "/debug" ||
    location === "/connections"
      ? `${sharedMainClasses} min-h-screen`
      : `${sharedMainClasses} h-screen`;

  const showSettingsButton =
    location === "/" ||
    location === "/settings" ||
    location === "/history" ||
    location === "/debug" ||
    location === "/connections";
  const showHistoryButton = showSettingsButton;
  const showDebugButton = showSettingsButton;
  const showConnectionsButton = showSettingsButton;

  const settingsButtonAction = location === "/settings" ? goHome : goSettings;
  const historyButtonAction = location === "/history" ? goHome : goHistory;
  const debugButtonAction = location === "/debug" ? goHome : goDebug;
  const connectionsButtonAction = location === "/connections" ? goHome : goConnections;

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

  const connectionsButtonIcon =
    location === "/connections" ? (
      <Wallet className="h-5 w-5" />
    ) : (
      <Link className="h-5 w-5" />
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
        {showSettingsButton || showHistoryButton || showConnectionsButton || showDebugButton ? (
          <div className="absolute right-4 top-4 z-20 flex flex-col items-end gap-2">
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
            {showConnectionsButton ? (
              <Button
                variant="outline"
                size="icon"
                className="h-10 w-10 rounded-full"
                onClick={connectionsButtonAction}
                aria-label={
                  location === "/connections" ? "Back to wallet" : "View connections"
                }
              >
                {connectionsButtonIcon}
              </Button>
            ) : null}
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
                    status?.current_network?.advertisement?.pricing_options?.[0]?.mint_url ?? ""
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

          <Route path="/connections">
            <ConnectionsScreen copyToClipboard={copyToClipboard} />
          </Route>
        </Switch>
      </main>

      <Dialog open={!!pendingConnection} onOpenChange={(open) => !open && setPendingConnection(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Wallet Connection Request</DialogTitle>
            <DialogDescription>
              An application wants to connect to your wallet
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            <div>
              <p className="mb-1 text-sm font-medium">App Public Key</p>
              <p className="break-all font-mono text-xs text-muted-foreground">
                {pendingConnection?.nwa_request.app_pubkey}
              </p>
            </div>

            {pendingConnection?.nwa_request.identity ? (
              <div>
                <p className="mb-1 text-sm font-medium">Identity</p>
                <p className="break-all font-mono text-xs text-muted-foreground">
                  {pendingConnection.nwa_request.identity}
                </p>
              </div>
            ) : null}

            <div>
              <p className="mb-1 text-sm font-medium">Required Commands</p>
              <p className="text-xs text-muted-foreground">
                {pendingConnection?.nwa_request.required_commands.join(", ") || "None"}
              </p>
            </div>

            {pendingConnection?.nwa_request.optional_commands.length ? (
              <div>
                <p className="mb-1 text-sm font-medium">Optional Commands</p>
                <p className="text-xs text-muted-foreground">
                  {pendingConnection.nwa_request.optional_commands.join(", ")}
                </p>
              </div>
            ) : null}

            {pendingConnection?.nwa_request.budget ? (
              <div>
                <p className="mb-1 text-sm font-medium">Budget</p>
                <p className="text-xs text-muted-foreground">
                  {pendingConnection.nwa_request.budget}
                </p>
              </div>
            ) : null}

            <div>
              <p className="mb-1 text-sm font-medium">Relays</p>
              <div className="space-y-1 text-xs text-muted-foreground">
                {pendingConnection?.nwa_request.relays.map((relay, idx) => (
                  <p key={idx} className="font-mono">
                    {relay}
                  </p>
                ))}
              </div>
            </div>
          </div>

          <DialogFooter>
            <Button variant="outline" onClick={handleRejectConnection}>
              Reject
            </Button>
            <Button onClick={handleApproveConnection}>Approve</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
