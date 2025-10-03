import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { BudgetControls, BudgetUsage } from "@/components/budget";
import { Screen } from "@/components/layout/screen";
import { SectionHeader } from "@/components/layout/section-header";
import { Card } from "@/components/ui/card";
import { CopyButton } from "@/components/copy-button";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

type NwcRenewalPeriod = "daily" | "weekly" | "monthly" | "yearly" | "never";

type NwcConnection = {
  pubkey: string;
  pubkey_hex: string;
  budget_msats: number;
  used_budget_msats: number;
  renewal_period: NwcRenewalPeriod;
  name: string;
};

type NwcConnectionView = NwcConnection & {
  budgetInput: string;
  nameInput: string;
};

type BudgetUpdateResponse = {
  budget_msats: number;
  used_budget_msats: number;
  renewal_period: NwcRenewalPeriod;
};

type ConnectionsScreenProps = {
  copyToClipboard: (value: string) => Promise<void> | void;
};

const msatsToSats = (value: number) => Math.floor(value / 1_000);

const renewalPeriodOptions: { value: NwcRenewalPeriod; label: string }[] = [
  { value: "daily", label: "Daily" },
  { value: "weekly", label: "Weekly" },
  { value: "monthly", label: "Monthly" },
  { value: "yearly", label: "Yearly" },
  { value: "never", label: "Never" },
];

const renewalPeriodLabel = (period: NwcRenewalPeriod) =>
  renewalPeriodOptions.find((option) => option.value === period)?.label.toUpperCase() ?? period.toUpperCase();

export function ConnectionsScreen({ copyToClipboard }: ConnectionsScreenProps) {
  const [connections, setConnections] = useState<NwcConnectionView[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [newNwcUri, setNewNwcUri] = useState<string | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [connectionToRemove, setConnectionToRemove] = useState<NwcConnectionView | null>(null);
  const [useLocalRelay, setUseLocalRelay] = useState(false);
  const [lastCreatedRelay, setLastCreatedRelay] = useState<string | null>(null);

  const loadConnections = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await invoke<NwcConnection[]>("nwc_list_connections");
      setConnections(
        result.map((connection) => ({
          ...connection,
          budgetInput: String(msatsToSats(connection.budget_msats)),
          nameInput: connection.name,
        }))
      );
    } catch (err) {
      console.error("Failed to load connections:", err);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadConnections();
    const interval = setInterval(() => {
      void loadConnections();
    }, 5_000);
    return () => clearInterval(interval);
  }, [loadConnections]);

  const handleBudgetInputChange = (pubkeyHex: string, value: string) => {
    setConnections((prev) =>
      prev.map((connection) =>
        connection.pubkey_hex === pubkeyHex ? { ...connection, budgetInput: value } : connection
      )
    );
  };

  const handleNameChange = (pubkeyHex: string, value: string) => {
    setConnections((prev) =>
      prev.map((connection) =>
        connection.pubkey_hex === pubkeyHex ? { ...connection, nameInput: value } : connection
      )
    );
  };

  const persistBudget = useCallback(
    async (nextState: NwcConnectionView, previousState?: NwcConnectionView) => {
      const parsed = Number(nextState.budgetInput);
      const normalized = Number.isFinite(parsed) ? Math.max(0, Math.floor(parsed)) : 0;
      const baseline = previousState ?? nextState;
      const baselineBudgetSats = msatsToSats(baseline.budget_msats);

      if (baselineBudgetSats === normalized && baseline.renewal_period === nextState.renewal_period) {
        if (nextState.budgetInput !== String(normalized)) {
          setConnections((prev) =>
            prev.map((item) =>
              item.pubkey_hex === nextState.pubkey_hex
                ? {
                    ...item,
                    budgetInput: String(normalized),
                    renewal_period: nextState.renewal_period,
                  }
                : item
            )
          );
        }
        return;
      }

      try {
        setError(null);
        const response = await invoke<BudgetUpdateResponse>("nwc_update_connection_budget", {
          pubkey: nextState.pubkey_hex,
          budgetSats: normalized,
          renewalPeriod: nextState.renewal_period,
        });
        const updatedPeriod = response.renewal_period;
        setConnections((prev) =>
          prev.map((item) =>
            item.pubkey_hex === nextState.pubkey_hex
              ? {
                  ...item,
                  budget_msats: response.budget_msats,
                  used_budget_msats: response.used_budget_msats,
                  renewal_period: updatedPeriod,
                  budgetInput: String(msatsToSats(response.budget_msats)),
                }
              : item
          )
        );
      } catch (err) {
        console.error("Failed to update connection budget:", err);
        setError(String(err));
        setConnections((prev) =>
          prev.map((item) =>
            item.pubkey_hex === nextState.pubkey_hex
              ? {
                  ...item,
                  budget_msats: baseline.budget_msats,
                  renewal_period: baseline.renewal_period,
                  budgetInput: String(baselineBudgetSats),
                }
              : item
          )
        );
      }
    },
    [setConnections, setError]
  );

  const persistName = useCallback(
    async (connection: NwcConnectionView) => {
      const trimmed = connection.nameInput.trim();
      if (trimmed === connection.name.trim()) {
        if (connection.nameInput !== connection.name) {
          setConnections((prev) =>
            prev.map((item) =>
              item.pubkey_hex === connection.pubkey_hex
                ? { ...item, nameInput: item.name }
                : item
            )
          );
        }
        return;
      }

      try {
        setError(null);
        const updatedName = await invoke<string>("nwc_update_connection_name", {
          pubkey: connection.pubkey_hex,
          name: connection.nameInput,
        });
        setConnections((prev) =>
          prev.map((item) =>
            item.pubkey_hex === connection.pubkey_hex
              ? { ...item, name: updatedName, nameInput: updatedName }
              : item
          )
        );
      } catch (err) {
        console.error("Failed to update connection name:", err);
        setError(String(err));
        setConnections((prev) =>
          prev.map((item) =>
            item.pubkey_hex === connection.pubkey_hex
              ? { ...item, nameInput: item.name }
              : item
          )
        );
      }
    },
    [setConnections, setError]
  );

  const handleBudgetBlur = (connection: NwcConnectionView) => {
    void persistBudget(connection);
  };

  const handlePeriodChange = (connection: NwcConnectionView, period: NwcRenewalPeriod) => {
    const updated = { ...connection, renewal_period: period };
    setConnections((prev) =>
      prev.map((item) => (item.pubkey_hex === connection.pubkey_hex ? updated : item))
    );
    void persistBudget(updated, connection);
  };

  const handleNameBlur = (connection: NwcConnectionView) => {
    void persistName(connection);
  };

  const handleRemove = async () => {
    if (!connectionToRemove) return;
    try {
      setError(null);
      const hex = connectionToRemove.pubkey_hex;
      await invoke("nwc_remove_connection", { pubkey: hex });
      setConnections((prev) => prev.filter((connection) => connection.pubkey_hex !== hex));
      setConnectionToRemove(null);
      await loadConnections();
    } catch (err) {
      console.error("Failed to remove connection:", err);
      setError(String(err));
    }
  };

  const handleCreateConnection = async () => {
    setIsCreating(true);
    setError(null);
    try {
      const uri = await invoke<string>("nwc_create_standard_connection", {
        use_local_relay: useLocalRelay,
      });
      setNewNwcUri(uri);
      setLastCreatedRelay(useLocalRelay ? "ws://localhost:4869" : "wss://nostrue.com");
      await loadConnections();
    } catch (err) {
      console.error("Failed to create NWC URI:", err);
      setError(String(err));
    } finally {
      setIsCreating(false);
    }
  };

  return (
    <Screen className="min-h-screen gap-6 overflow-y-auto pt-6">
      <SectionHeader
        title="NWC Settings"
        description="Manage your NWC wallet connections"
      />

      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:gap-6">
        <div className="flex max-w-sm items-start gap-2">
          <Checkbox
            id="nwc-local-relay"
            checked={useLocalRelay}
            onCheckedChange={(checked) => setUseLocalRelay(checked === true)}
          />
          <label htmlFor="nwc-local-relay" className="text-sm leading-relaxed">
            <span className="font-medium">Use local relay</span>
            <span className="block text-xs text-muted-foreground">
              Only works with apps running on this device. Remote connections should use the public relay.
            </span>
          </label>
        </div>
        <Button onClick={handleCreateConnection} disabled={isCreating} className="w-max">
          {isCreating ? "Creating…" : "Create New Connection"}
        </Button>
      </div>

      {loading && connections.length === 0 ? (
        <Card className="border border-dashed border-primary/20 bg-background/90 p-6">
          <p className="text-center text-muted-foreground">Loading connections…</p>
        </Card>
      ) : error ? (
        <Card className="border border-dashed border-destructive/30 bg-background/90 p-6">
          <p className="text-center text-destructive">Failed to load connections: {error}</p>
        </Card>
      ) : connections.length === 0 ? (
        <Card className="border border-dashed border-primary/20 bg-background/90 p-6">
          <p className="text-center text-muted-foreground">No active connections</p>
          <p className="mt-2 text-center text-xs text-muted-foreground">
            Connections will appear here when apps connect to your wallet.
          </p>
        </Card>
      ) : (
        <div className="space-y-4 pb-2">
          {connections.map((connection, index) => {
            const totalSats = msatsToSats(connection.budget_msats);
            const usedSats = msatsToSats(connection.used_budget_msats);
            const nameInputId = `connection-name-${index}`;
            const controlsIdPrefix = `connection-${index}`;

            return (
              <Card
                key={connection.pubkey_hex}
                className="space-y-4 border border-dashed border-primary/20 bg-background/90 p-4"
              >
                <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                  <div className="flex-1 space-y-3">
                    <div className="grid gap-2">
                      <Label htmlFor={nameInputId}>Connection Name</Label>
                      <Input
                        id={nameInputId}
                        value={connection.nameInput}
                        onChange={(event) =>
                          handleNameChange(connection.pubkey_hex, event.target.value)
                        }
                        onBlur={() => handleNameBlur(connection)}
                        onKeyDown={(event) => {
                          if (event.key === "Enter") {
                            event.currentTarget.blur();
                          }
                        }}
                      />
                    </div>
                    <div className="space-y-1">
                      <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                        Public Key
                      </p>
                      <p className="break-all font-mono text-xs text-foreground">
                        {connection.pubkey}
                      </p>
                    </div>
                  </div>
                  <div className="flex items-center gap-2 sm:pt-[30px]">
                    <CopyButton
                      onCopy={() => copyToClipboard(connection.pubkey)}
                      label="Copy"
                      copiedLabel="✓"
                      variant="outline"
                      size="sm"
                    />
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => setConnectionToRemove(connection)}
                    >
                      Remove
                    </Button>
                  </div>
                </div>

                <div className="grid gap-4">
                  <BudgetUsage
                    used={usedSats}
                    total={totalSats}
                    periodLabel={renewalPeriodLabel(connection.renewal_period)}
                  />

                  <BudgetControls
                    idPrefix={controlsIdPrefix}
                    budgetValue={connection.budgetInput}
                    onBudgetChange={(value) =>
                      handleBudgetInputChange(connection.pubkey_hex, value)
                    }
                    onBudgetBlur={() => handleBudgetBlur(connection)}
                    periodValue={connection.renewal_period}
                    onPeriodChange={(value) =>
                      handlePeriodChange(connection, value as NwcRenewalPeriod)
                    }
                    periodOptions={renewalPeriodOptions}
                  />
                </div>
              </Card>
            );
          })}
        </div>
      )}

      <Dialog
        open={newNwcUri !== null}
        onOpenChange={(open) => {
          if (!open) {
            setNewNwcUri(null);
            setLastCreatedRelay(null);
          }
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>New NWC Connection</DialogTitle>
            <DialogDescription>
              Share this URI with the application you want to connect. Treat it like a password.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            {lastCreatedRelay ? (
              <div className="rounded-md bg-muted px-3 py-2 text-xs text-muted-foreground">
                Relay: {lastCreatedRelay}
              </div>
            ) : null}
            <p className="break-all rounded-md bg-muted p-3 font-mono text-xs text-muted-foreground">
              {newNwcUri}
            </p>
            <CopyButton
              onCopy={() => (newNwcUri ? copyToClipboard(newNwcUri) : Promise.resolve())}
              label="Copy URI"
              copiedLabel="Copied"
              variant="outline"
            />
          </div>
          <div className="flex justify-end">
            <Button
              variant="outline"
              onClick={() => {
                setNewNwcUri(null);
                setLastCreatedRelay(null);
              }}
            >
              Close
            </Button>
          </div>
        </DialogContent>
      </Dialog>

      <Dialog open={connectionToRemove !== null} onOpenChange={(open) => !open && setConnectionToRemove(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Remove Connection</DialogTitle>
            <DialogDescription>
              This will disconnect the selected application from your wallet.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="rounded-md bg-muted p-3 text-xs">
              <p className="font-semibold uppercase tracking-wide text-muted-foreground">Connection</p>
              <p className="mt-1 break-all font-mono text-xs text-foreground">
                {connectionToRemove?.name ?? ""}
              </p>
              <p className="mt-1 break-all font-mono text-[11px] text-muted-foreground">
                {connectionToRemove?.pubkey ?? ""}
              </p>
            </div>
            <p className="text-sm text-muted-foreground">
              Removing a connection means the app will no longer be able to issue wallet requests until you share a new NWC URI.
            </p>
          </div>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => setConnectionToRemove(null)}>
              Cancel
            </Button>
            <Button variant="destructive" onClick={handleRemove}>
              Remove Connection
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </Screen>
  );
}
