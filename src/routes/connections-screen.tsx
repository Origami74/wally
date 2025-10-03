import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Screen } from "@/components/layout/screen";
import { SectionHeader } from "@/components/layout/section-header";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { CopyButton } from "@/components/copy-button";
import { Button } from "@/components/ui/button";

type NwcConnection = {
  pubkey: string;
  budget_msats: number;
  used_budget_msats: number;
  renewal_period: "daily" | "weekly" | "monthly" | "yearly" | "never";
};

type ConnectionsScreenProps = {
  copyToClipboard: (value: string) => Promise<void> | void;
};

export function ConnectionsScreen({ copyToClipboard }: ConnectionsScreenProps) {
  const [connections, setConnections] = useState<NwcConnection[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [copying, setCopying] = useState(false);

  useEffect(() => {
    const loadConnections = async () => {
      try {
        setLoading(true);
        setError(null);
        const result = await invoke<NwcConnection[]>("nwc_list_connections");
        setConnections(result);
      } catch (err) {
        console.error("Failed to load connections:", err);
        setError(String(err));
      } finally {
        setLoading(false);
      }
    };

    loadConnections();
    
    // Refresh every 5 seconds
    const interval = setInterval(loadConnections, 5000);
    
    return () => clearInterval(interval);
  }, []);

  const handleRemove = async (pubkey: string) => {
    if (window.confirm("Are you sure you want to remove this connection?")) {
      try {
        await invoke("nwc_remove_connection", { pubkey });
        // Refresh connections list
        setConnections((prev) => prev.filter((c) => c.pubkey !== pubkey));
      } catch (err) {
        console.error("Failed to remove connection:", err);
        setError(String(err)); // Show error to the user
      }
    }
  };

  const formatBudget = (msats: number) => {
    const sats = Math.floor(msats / 1000);
    return sats.toLocaleString();
  };

  const getBudgetPercentage = (used: number, total: number) => {
    if (total === 0) return 0;
    return Math.min(100, Math.round((used / total) * 100));
  };

  const formatPeriod = (period: string) => {
    return period.charAt(0).toUpperCase() + period.slice(1);
  };

  return (
    <Screen className="min-h-screen gap-6 overflow-y-auto pt-6">
      <SectionHeader
        title="NWC Settings"
        description="Manage your NWC wallet connections"
        actions={
          <CopyButton
            onCopy={async () => {
              try {
                setCopying(true);
                const pubkey = await invoke<string>("nwc_get_service_pubkey");
                await copyToClipboard(pubkey);
              } finally {
                setCopying(false);
              }
            }}
            label={copying ? "Copying…" : "Copy NWC string"}
            copiedLabel="Copied"
            variant="outline"
            className="w-max"
          />
        }
      />

      {loading && connections.length === 0 ? (
        <Card className="border border-dashed border-primary/20 bg-background/90 p-6">
          <p className="text-center text-muted-foreground">Loading connections...</p>
        </Card>
      ) : error ? (
        <Card className="border border-dashed border-destructive/30 bg-background/90 p-6">
          <p className="text-center text-destructive">
            Failed to load connections: {error}
          </p>
        </Card>
      ) : connections.length === 0 ? (
        <Card className="border border-dashed border-primary/20 bg-background/90 p-6">
          <p className="text-center text-muted-foreground">
            No active connections
          </p>
          <p className="mt-2 text-center text-xs text-muted-foreground">
            Connections will appear here when apps connect to your wallet
          </p>
        </Card>
      ) : (
        <div className="space-y-4 pb-2">
          {connections.map((connection) => {
            const budgetPercentage = getBudgetPercentage(
              connection.used_budget_msats,
              connection.budget_msats
            );
            const remainingSats = formatBudget(
              connection.budget_msats - connection.used_budget_msats
            );

            return (
              <Card
                key={connection.pubkey}
                className="space-y-4 border border-dashed border-primary/20 bg-background/90 p-4"
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="flex-1 space-y-1">
                    <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                      Public Key
                    </p>
                    <p className="break-all font-mono text-xs text-foreground">
                      {connection.pubkey}
                    </p>
                  </div>
                  <CopyButton
                    onCopy={() => copyToClipboard(connection.pubkey)}
                    label="Copy"
                    copiedLabel="✓"
                    variant="outline"
                    className="shrink-0"
                  />
                </div>

                <div className="space-y-2">
                  <div className="flex items-center justify-between text-xs">
                    <span className="uppercase tracking-wide text-muted-foreground">
                      Budget Usage
                    </span>
                    <span className="font-semibold text-primary">
                      {budgetPercentage}%
                    </span>
                  </div>
                  <div className="h-2 rounded-full bg-muted">
                    <div
                      className="h-full rounded-full bg-primary transition-all"
                      style={{ width: `${budgetPercentage}%` }}
                    />
                  </div>
                  <div className="grid grid-cols-2 gap-3 text-xs">
                    <div>
                      <span className="block text-[10px] uppercase tracking-wide text-muted-foreground">
                        Used
                      </span>
                      <span className="text-sm font-medium text-foreground">
                        {formatBudget(connection.used_budget_msats)} sats
                      </span>
                    </div>
                    <div className="text-right">
                      <span className="block text-[10px] uppercase tracking-wide text-muted-foreground">
                        Remaining
                      </span>
                      <span className="text-sm font-medium text-foreground">
                        {remainingSats} sats
                      </span>
                    </div>
                  </div>
                </div>

                <div className="flex items-center justify-between">
                  <div>
                    <p className="text-[10px] uppercase tracking-wide text-muted-foreground">
                      Total Budget
                    </p>
                    <p className="text-sm font-medium text-foreground">
                      {formatBudget(connection.budget_msats)} sats
                    </p>
                  </div>
                  <Badge tone="info" className="uppercase">
                    {formatPeriod(connection.renewal_period)}
                  </Badge>
                </div>
                <Button
                  variant="destructive"
                  size="sm"
                  onClick={() => handleRemove(connection.pubkey)}
                  className="w-full"
                >
                  Remove Connection
                </Button>
              </Card>
            );
          })}
        </div>
      )}
    </Screen>
  );
}
