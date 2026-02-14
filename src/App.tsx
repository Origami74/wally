import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { History, Settings2, Wallet } from "lucide-react";
import { Route, Switch, useLocation } from "wouter";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { HomeScreen } from "@/routes/home-screen";
import { ReceiveScreen } from "@/routes/receive-screen";
import { SendScreen } from "@/routes/send-screen";
import { SettingsScreen } from "@/routes/settings-screen";
import { ConnectionsScreen } from "@/routes/connections-screen";
import { ScannerPage } from "@/routes/scanner-screen";
import type { FeatureState, Period, StatusBadge } from "@/routes/types";
import { periods } from "@/routes/types";
import { HistoryScreen } from "@/routes/history-screen";
import { cn } from "@/lib/utils";
import {
  fetchWalletSummary,
  fetchWalletTransactions,
  receiveCashuToken,
  type WalletSummary,
  type WalletTransactionEntry,
} from "@/lib/wallet/api";
import { identifyRequest } from "@/lib/wallet/utils";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { RoutstrScreen } from "./routes/routstr-screen";

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
  } | null;
  received_at: number;
  nwc_uri: string | null;
  approved: boolean;
  rejected: boolean;
};

const initialFeatures: FeatureState[] = [
  {
    id: "routstr",
    title: "Proxy",
    description: "Enable Routstr proxy for privacy and payments.",
    enabled: true,
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

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 5 * 60 * 1000,
      gcTime: 10 * 60 * 1000,
      refetchOnWindowFocus: false,
    },
  },
});

function AppContent() {
  const [location, setLocation] = useLocation();
  const [walletSummary, setWalletSummary] = useState<WalletSummary | null>(
    null,
  );
  const [transactions, setTransactions] = useState<WalletTransactionEntry[]>(
    [],
  );
  const [mintInput, setMintInput] = useState("");
  const [savingMint, setSavingMint] = useState(false);
  const [sendRequest, setSendRequest] = useState("");
  const [features, setFeatures] = useState<FeatureState[]>(initialFeatures);
  const [pendingConnection, setPendingConnection] =
    useState<PendingConnectionRequest | null>(null);

  const periodMeta = useCallback(
    (period: Period) =>
      periods.find((item) => item.value === period) ?? periods[0],
    [],
  );

  const refreshStatus = useCallback(async () => {
    try {
      const [summaryResult, transactionsResult] = await Promise.all([
        fetchWalletSummary(),
        fetchWalletTransactions(),
      ]);

      setWalletSummary(summaryResult);
      setTransactions(transactionsResult);

      if (!mintInput && summaryResult.default_mint) {
        setMintInput(summaryResult.default_mint);
      }
    } catch (error) {
      console.error("Failed to refresh wallet status", error);
    }
  }, [mintInput]);

  useEffect(() => {
    let mounted = true;
    const listeners: UnlistenFn[] = [];

    const initialise = async () => {
      await refreshStatus();
      try {
        const nwcConnectionRequest = await listen(
          "nwc-connection-request",
          async (event: any) => {
            if (!mounted) return;
            console.log("App: Connection request received:", event.payload);
            setPendingConnection(event.payload as PendingConnectionRequest);
          },
        );
        listeners.push(nwcConnectionRequest);
      } catch (error) {
        console.warn("Failed to register listeners", error);
      }
    };

    initialise();
    const interval = setInterval(refreshStatus, 10_000);

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
      await invoke("set_default_mint", { mintUrl: mintInput.trim() });
      await refreshStatus();
    } catch (error) {
      console.error("Failed to set default mint", error);
      alert(`Failed to set default mint: ${error}`);
    } finally {
      setSavingMint(false);
    }
  }, [mintInput, refreshStatus]);

  const handleFeatureUpdate = useCallback(
    (
      id: FeatureState["id"],
      updater: (feature: FeatureState) => FeatureState,
    ) => {
      setFeatures((prev) =>
        prev.map((feature) => (feature.id === id ? updater(feature) : feature)),
      );
    },
    [],
  );

  const copyToClipboard = useCallback(async (value: string) => {
    try {
      await navigator.clipboard?.writeText(value);
    } catch (error) {
      console.warn("Copy failed", error);
    }
  }, []);

  const walletBalance = walletSummary?.total ?? 0;

  const handlePaymentComplete = useCallback(async () => {
    setSendRequest("");
    await refreshStatus();
    setLocation("/");
  }, [refreshStatus, setLocation]);

  const handleApproveConnection = useCallback(async () => {
    if (!pendingConnection) return;

    try {
      await invoke("nwc_approve_connection", {
        requestId: pendingConnection.request_id,
      });
      setPendingConnection(null);
      await refreshStatus();
      setLocation("/");
    } catch (error) {
      console.error("Failed to approve connection:", error);
      alert(`Failed to approve connection: ${error}`);
      setPendingConnection(null);
      setLocation("/");
    }
  }, [pendingConnection, refreshStatus, setLocation]);

  const handleRejectConnection = useCallback(async () => {
    if (!pendingConnection) return;

    try {
      await invoke("nwc_reject_connection", {
        requestId: pendingConnection.request_id,
      });
      setPendingConnection(null);
      setLocation("/");
    } catch (error) {
      console.error("Failed to reject connection:", error);
      setPendingConnection(null);
      setLocation("/");
    }
  }, [pendingConnection, setLocation]);

  const handleScanResult = useCallback(
    async (content: string) => {
      let trimmed = content.trim();
      const type = identifyRequest(trimmed);

      if (type === "cashu-token") {
        try {
          const result = await receiveCashuToken(trimmed);
          alert(`Successfully received ${result.amount} sats!`);
          await refreshStatus();
        } catch (err) {
          console.error("Failed to receive token", err);
          alert("Failed to receive token. It might be already redeemed.");
        }
      } else if (type === "cashu-request" || type === "lightning-invoice") {
        if (trimmed.toLowerCase().startsWith("lightning:")) {
          trimmed = trimmed.substring(10);
        }
        setSendRequest(trimmed);
        setLocation("/send");
      } else {
        alert("Unknown QR code format.");
      }
    },
    [refreshStatus, setLocation],
  );

  const statusBadges: StatusBadge[] = useMemo(() => {
    const badges: StatusBadge[] = [];

    const featureEnabled = (featureId: FeatureState["id"]) =>
      features.find((feature) => feature.id === featureId)?.enabled ?? false;

    const nwcEnabled = featureEnabled("nwc");
    badges.push({
      id: "connections",
      label: "NWC",
      value: nwcEnabled ? "Enabled" : "Available",
      tone: nwcEnabled ? "info" : "default",
      onClick: () => setLocation("/connections"),
    });

    if (featureEnabled("routstr")) {
      badges.push({
        id: "routstr",
        label: "Proxy",
        value: "Enabled",
        tone: "info",
        onClick: () => setLocation("/routstr"),
      });
    }

    return badges;
  }, [features, setLocation]);

  const goHome = () => setLocation("/");
  const goReceive = () => setLocation("/receive");
  const goSend = () => setLocation("/send");
  const goSettings = () => setLocation("/settings");
  const goHistory = () => setLocation("/history");
  const goScanner = () => setLocation("/scanner");

  const sharedMainClasses =
    "relative mx-auto flex w-full max-w-md flex-col overflow-hidden";

  const mainClasses = cn(
    sharedMainClasses,
    location === "/settings" ||
      location === "/history" ||
      location === "/connections"
      ? "min-h-screen"
      : "h-[100dvh]",
  );

  // Apply transparent background for scanner route
  useEffect(() => {
    if (location === "/scanner") {
      document.body.style.backgroundColor = "transparent";
      document.documentElement.style.backgroundColor = "transparent";
    } else {
      document.body.style.backgroundColor = "";
      document.documentElement.style.backgroundColor = "";
    }

    return () => {
      document.body.style.backgroundColor = "";
      document.documentElement.style.backgroundColor = "";
    };
  }, [location]);

  const isHome = location === "/";

  const navButtons =
    location === "/receive" || location === "/send"
      ? []
      : isHome
        ? [
            {
              id: "settings",
              icon: <Settings2 className="h-5 w-5" />,
              action: goSettings,
              label: "Open settings",
            },
            {
              id: "history",
              icon: <History className="h-5 w-5" />,
              action: goHistory,
              label: "View history",
            },
          ]
        : [
            {
              id: "home",
              icon: <Wallet className="h-5 w-5" />,
              action: goHome,
              label: "Back to wallet",
            },
          ];

  return (
    <div
      className={cn(
        "text-foreground",
        location === "/scanner" ? "bg-transparent" : "bg-background",
      )}
      style={{ overscrollBehavior: "none" }}
    >
      <main className={mainClasses}>
        {navButtons.length ? (
          <div
            className="absolute right-4 z-20 flex flex-col items-end gap-2"
            style={{ top: "calc(env(safe-area-inset-top) + 1rem)" }}
          >
            {navButtons.map((button) => (
              <Button
                key={button.id}
                variant="outline"
                size="icon"
                className="h-10 w-10 rounded-full"
                onClick={button.action}
                aria-label={button.label}
              >
                {button.icon}
              </Button>
            ))}
          </div>
        ) : null}

        <Switch>
          <Route path="/">
            <HomeScreen
              statusBadges={statusBadges}
              walletBalance={walletBalance}
              walletSummary={walletSummary}
              onReceive={goReceive}
              onSend={goSend}
              onScan={goScanner}
            />
          </Route>

          <Route path="/receive">
            <ReceiveScreen
              onBack={goHome}
              copyToClipboard={copyToClipboard}
              defaultMint={walletSummary?.default_mint ?? ""}
              onNavigateToSend={(req) => {
                setSendRequest(req);
                setLocation("/send");
              }}
              onScan={goScanner}
            />
          </Route>

          <Route path="/send">
            <SendScreen
              onBack={goHome}
              request={sendRequest}
              onChangeRequest={setSendRequest}
              onPaymentComplete={handlePaymentComplete}
              onScan={goScanner}
              walletSummary={walletSummary}
            />
          </Route>

          <Route path="/settings">
            <SettingsScreen
              status={null}
              features={features}
              mintInput={mintInput}
              setMintInput={setMintInput}
              savingMint={savingMint}
              onSaveMint={saveMintUrl}
              onReset={() => {
                if (walletSummary?.default_mint) {
                  setMintInput(walletSummary.default_mint);
                }
              }}
              handleFeatureUpdate={handleFeatureUpdate}
              periodMeta={periodMeta}
              copyToClipboard={copyToClipboard}
              walletSummary={walletSummary}
              onRefresh={refreshStatus}
            />
          </Route>

          <Route path="/history">
            <HistoryScreen transactions={transactions} />
          </Route>

          <Route path="/connections">
            <ConnectionsScreen copyToClipboard={copyToClipboard} />
          </Route>

          <Route path="/routstr">
            <RoutstrScreen copyToClipboard={copyToClipboard} />
          </Route>

          <Route path="/scanner">
            <ScannerPage
              onResult={(content) => {
                handleScanResult(content);
              }}
              onCancel={() => {
                // If we came from Home, go Home, else try to go back
                setLocation("/");
              }}
            />
          </Route>
        </Switch>
      </main>

      <Dialog
        open={!!pendingConnection}
        onOpenChange={(open) => !open && setPendingConnection(null)}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Wallet Connection Request</DialogTitle>
            <DialogDescription>
              {pendingConnection?.nwa_request
                ? "An application wants to connect to your wallet (NWA)"
                : "An application wants to connect to your wallet (Standard NWC)"}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-4">
            {pendingConnection?.nwa_request ? (
              <>
                <div>
                  <p className="mb-1 text-sm font-medium">App Public Key</p>
                  <p className="break-all font-mono text-xs text-muted-foreground">
                    {pendingConnection.nwa_request.app_pubkey}
                  </p>
                </div>

                {pendingConnection.nwa_request.identity ? (
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
                    {pendingConnection.nwa_request.required_commands.join(
                      ", ",
                    ) || "None"}
                  </p>
                </div>

                {pendingConnection.nwa_request.optional_commands.length ? (
                  <div>
                    <p className="mb-1 text-sm font-medium">
                      Optional Commands
                    </p>
                    <p className="text-xs text-muted-foreground">
                      {pendingConnection.nwa_request.optional_commands.join(
                        ", ",
                      )}
                    </p>
                  </div>
                ) : null}

                {pendingConnection.nwa_request.budget ? (
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
                    {pendingConnection.nwa_request.relays.map((relay, idx) => (
                      <p key={idx} className="font-mono">
                        {relay}
                      </p>
                    ))}
                  </div>
                </div>
              </>
            ) : (
              <div>
                <p className="text-sm text-muted-foreground">
                  A client is requesting a standard Nostr Wallet Connect
                  connection. Approving will generate a connection string the
                  client can use to interact with your wallet.
                </p>
                <div className="mt-4 rounded-md bg-muted p-3">
                  <p className="text-xs text-muted-foreground">
                    Default budget: 1,000 sats / day
                  </p>
                </div>
              </div>
            )}
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

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AppContent />
    </QueryClientProvider>
  );
}
