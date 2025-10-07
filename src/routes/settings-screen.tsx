import { BudgetControls, BudgetUsage } from "@/components/budget";
import { Screen } from "@/components/layout/screen";
import { SectionHeader } from "@/components/layout/section-header";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { ServiceStatus } from "@/lib/tollgate/types";
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
};

export function SettingsScreen({
  status: _status,
  features,
  mintInput,
  npubInput,
  setMintInput,
  setNpubInput,
  savingMint,
  onSaveMint,
  onReset,
  handleFeatureUpdate,
  copyToClipboard: _copyToClipboard,
  periodMeta,
}: SettingsScreenProps) {
  const [, setLocation] = useLocation();
  void _status;
  void _copyToClipboard;

  return (
    <Screen className="min-h-screen gap-8 overflow-y-auto">
      <div className="grid gap-4">
        <SectionHeader title="Wallet Settings" />
        <Card className="mt-2 space-y-4 border border-dashed border-primary/20 bg-background/90 p-4">
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
                {savingMint ? "Setting…" : "Set Default Mint"}
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
                className="space-y-4 border border-dashed border-primary/20 bg-background/90 p-4"
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
                  periodLabel={periodMeta(feature.period).label.toUpperCase()}
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
