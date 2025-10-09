import { BudgetControls, BudgetUsage } from "@/components/budget";
import { Screen } from "@/components/layout/screen";
import { SectionHeader } from "@/components/layout/section-header";
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
import type { ServiceStatus } from "@/lib/tollgate/types";
import type { WalletSummary } from "@/lib/wallet/api";
import { addMint, removeMint } from "@/lib/wallet/api";
import { ChevronDown, ChevronRight, Trash2, Plus } from "lucide-react";
import { useCallback, useState } from "react";
import { useLocation } from "wouter";

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
    updater: (feature: FeatureState) => FeatureState,
  ) => void;
  copyToClipboard: (value: string) => Promise<void> | void;
  periodMeta: PeriodMetaFn;
  walletSummary: WalletSummary | null;
  onRefresh: () => Promise<void>;
};

export function SettingsScreen({
  status: _status,
  features,
  npubInput,
  setMintInput,
  setNpubInput,
  savingMint,
  onSaveMint,
  onReset,
  handleFeatureUpdate,
  copyToClipboard: _copyToClipboard,
  periodMeta,
  walletSummary,
  onRefresh,
}: SettingsScreenProps) {
  const [, setLocation] = useLocation();
  const [newMintUrl, setNewMintUrl] = useState("");
  const [addingMint, setAddingMint] = useState(false);
  const [removingMints, setRemovingMints] = useState<Set<string>>(new Set());
  const [mintsExpanded, setMintsExpanded] = useState(false);
  void _status;
  void _copyToClipboard;

  const handleAddMint = useCallback(async () => {
    if (!newMintUrl.trim()) return;

    setAddingMint(true);
    try {
      await addMint(newMintUrl.trim());
      setNewMintUrl("");
      await onRefresh();
    } catch (error) {
      console.error("Failed to add mint:", error);
      alert(`Failed to add mint: ${error}`);
    } finally {
      setAddingMint(false);
    }
  }, [newMintUrl, onRefresh]);

  const handleRemoveMint = useCallback(
    async (mintUrl: string) => {
      setRemovingMints((prev) => new Set([...prev, mintUrl]));
      try {
        await removeMint(mintUrl);
        await onRefresh();
      } catch (error) {
        console.error("Failed to remove mint:", error);
        alert(`Failed to remove mint: ${error}`);
      } finally {
        setRemovingMints((prev) => {
          const newSet = new Set(prev);
          newSet.delete(mintUrl);
          return newSet;
        });
      }
    },
    [onRefresh],
  );

  return (
    <Screen className="min-h-screen gap-4 overflow-y-auto pb-8">
      <div className="grid gap-4">
        <SectionHeader title="Wallet Settings" />
        {/* Mint Management */}
        <Card className="mt-2 space-y-4 border border-dashed border-primary/20 bg-background/90 p-4">
          <Collapsible open={mintsExpanded} onOpenChange={setMintsExpanded}>
            <CollapsibleTrigger className="flex w-full items-center justify-between text-left">
              <div>
                <h3 className="text-base font-semibold">Mint Management</h3>
                <p className="text-sm text-muted-foreground">
                  Manage your Cashu mints (
                  {walletSummary?.balances?.length || 0} configured)
                </p>
              </div>
              {mintsExpanded ? (
                <ChevronDown className="h-4 w-4" />
              ) : (
                <ChevronRight className="h-4 w-4" />
              )}
            </CollapsibleTrigger>

            <CollapsibleContent className="space-y-4 overflow-hidden">
              <div className="space-y-3 border-t pt-4">
                <Label htmlFor="new-mint-url">Add New Mint</Label>
                <div className="flex gap-2">
                  <Input
                    id="new-mint-url"
                    value={newMintUrl}
                    onChange={(event) => setNewMintUrl(event.target.value)}
                    placeholder="https://mint.example.com"
                    disabled={addingMint}
                  />
                  <Button
                    onClick={handleAddMint}
                    disabled={!newMintUrl.trim() || addingMint}
                    size="sm"
                  >
                    <Plus className="h-4 w-4" />
                    {addingMint ? "Adding..." : "Add"}
                  </Button>
                </div>
              </div>

              {/* Existing Mints */}
              <div className="space-y-2">
                <Label>Configured Mints</Label>
                {walletSummary?.balances &&
                walletSummary.balances.length > 0 ? (
                  <div className="space-y-2 max-h-64 overflow-hidden">
                    {walletSummary.balances.map((balance) => (
                      <div key={balance.mint_url}>
                        <div className="flex items-start justify-between">
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2">
                              <p className="text-sm font-medium truncate">
                                {balance.mint_url}
                              </p>
                            </div>
                            <p className="text-xs text-muted-foreground">
                              Balance: {balance.balance} {balance.unit}
                              {balance.pending > 0 && (
                                <span className="ml-2">
                                  (Pending: {balance.pending} {balance.unit})
                                </span>
                              )}
                            </p>
                          </div>
                          <div className="flex flex-col gap-2 shrink-0">
                            {walletSummary.default_mint !==
                              balance.mint_url && (
                              <Button
                                variant="outline"
                                size="sm"
                                onClick={onSaveMint}
                                onMouseDown={() =>
                                  setMintInput(balance.mint_url)
                                }
                                disabled={savingMint}
                                className="text-xs"
                              >
                                {savingMint ? "Setting..." : "Set Default"}
                              </Button>
                            )}
                            {walletSummary.default_mint ===
                              balance.mint_url && (
                              <span className="text-xs bg-primary/10 text-primary p-1 rounded">
                                Default
                              </span>
                            )}
                            <Button
                              variant="outline"
                              size="sm"
                              onClick={() => handleRemoveMint(balance.mint_url)}
                              disabled={
                                removingMints.has(balance.mint_url) ||
                                balance.balance > 0
                              }
                              title={
                                balance.balance > 0
                                  ? "Cannot remove mint with remaining balance"
                                  : "Remove mint"
                              }
                              className="flex items-center gap-1"
                            >
                              <Trash2 className="h-4 w-4" />
                              <span className="hidden sm:inline">
                                {removingMints.has(balance.mint_url)
                                  ? "Removing..."
                                  : "Remove"}
                              </span>
                            </Button>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                ) : (
                  <p className="text-sm text-muted-foreground py-4 text-center">
                    No mints configured. Add a mint above to get started.
                  </p>
                )}
              </div>
            </CollapsibleContent>
          </Collapsible>
        </Card>

        {/* Legacy Settings */}
        <Card className="mt-2 space-y-4 border border-dashed border-primary/20 bg-background/90 p-4">
          <div className="grid gap-3">
            <div className="grid gap-2">
              <Label htmlFor="wallet-npub">Wallet npub</Label>
              <Input
                id="wallet-npub"
                value={npubInput}
                onChange={(event) => setNpubInput(event.target.value)}
                placeholder="npub..."
              />
            </div>
            <div className="flex gap-2">
              <Button variant="outline" onClick={onReset}>
                Reset Settings
              </Button>
            </div>
          </div>
        </Card>
      </div>

      <div className="grid gap-3 pb-2">
        <h2 className="text-lg font-semibold uppercase tracking-[0.2em] text-muted-foreground">
          Features
        </h2>

        {["tollgate", "nwc", "402", "routstr"]
          .map((id) => features.find((feature) => feature.id === id))
          .filter((feature): feature is FeatureState => Boolean(feature))
          .map((feature) => {
            const isComingSoon = feature.id === "402";
            const isToggleDisabled = isComingSoon;
            const navigateToDebug = () => {
              if (feature.id === "tollgate") setLocation("/debug");
              if (feature.id === "nwc") setLocation("/connections");
              if (feature.id === "routstr") setLocation("/routstr");
            };

            return (
              <Card
                key={feature.id}
                className="space-y-3 border border-dashed border-primary/20 bg-background/90 p-3"
              >
                <div className="flex items-start gap-3">
                  <Checkbox
                    id={`${feature.id}-checkbox`}
                    checked={feature.enabled}
                    onCheckedChange={() => {
                      if (isToggleDisabled) return;
                      handleFeatureUpdate(feature.id, (current) => ({
                        ...current,
                        enabled: !current.enabled,
                      }));
                    }}
                    className="h-5 w-5 rounded-md border-border"
                    disabled={isToggleDisabled}
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
                      {isComingSoon ? " (Coming soon)" : null}
                    </p>
                  </div>
                  <Button
                    variant="outline"
                    size="sm"
                    className="ml-auto h-auto rounded-full px-3 py-1 text-xs"
                    onClick={() => {
                      if (isToggleDisabled) return;
                      handleFeatureUpdate(feature.id, (current) => ({
                        ...current,
                        enabled: !current.enabled,
                      }));
                    }}
                    disabled={isToggleDisabled}
                  >
                    {feature.enabled ? "Disable" : "Enable"}
                  </Button>
                </div>

                {!isComingSoon ? (
                  <div className="grid gap-4">
                    <BudgetUsage
                      used={feature.spent ?? 0}
                      total={Number(feature.budget) || 0}
                      periodLabel={periodMeta(
                        feature.period,
                      ).label.toUpperCase()}
                    />
                    <BudgetControls
                      idPrefix={feature.id}
                      budgetValue={feature.budget}
                      onBudgetChange={(value) =>
                        handleFeatureUpdate(feature.id, (current) => ({
                          ...current,
                          budget: value,
                        }))
                      }
                      periodValue={feature.period}
                      onPeriodChange={(value) =>
                        handleFeatureUpdate(feature.id, (current) => ({
                          ...current,
                          period: value as Period,
                        }))
                      }
                      periodOptions={periods.map((option) => ({
                        value: option.value,
                        label: option.label,
                      }))}
                    />
                  </div>
                ) : null}

                <div className="flex justify-end">
                  {feature.id === "tollgate" ? (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={navigateToDebug}
                    >
                      Tollgate Settings
                    </Button>
                  ) : feature.id === "nwc" ? (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={navigateToDebug}
                    >
                      NWC Settings
                    </Button>
                  ) : feature.id === "routstr" ? (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={navigateToDebug}
                    >
                      Routstr Settings
                    </Button>
                  ) : (
                    <Button variant="outline" size="sm" disabled>
                      Coming Soon
                    </Button>
                  )}
                </div>
              </Card>
            );
          })}
      </div>
    </Screen>
  );
}
