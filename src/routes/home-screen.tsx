import { ScanQrCode } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Screen } from "@/components/layout/screen";
import { calculateTotalMsat, formatBalanceDisplay } from "@/lib/wallet/utils";
import type { WalletSummary } from "@/lib/wallet/api";
import type { StatusBadge } from "./types";
import { cn } from "@/lib/utils";

type HomeScreenProps = {
  statusBadges: StatusBadge[];
  walletBalance: number;
  walletSummary?: WalletSummary | null;
  onReceive: () => void;
  onSend: () => void;
  onScan: () => void;
};

export function HomeScreen({
  statusBadges,
  walletBalance,
  walletSummary,
  onReceive,
  onSend,
  onScan,
}: HomeScreenProps) {
  return (
    <Screen className="relative h-full gap-6">
      <div
        className="absolute left-4 flex flex-col gap-2"
        style={{ top: "calc(env(safe-area-inset-top) + 1rem)" }}
      >
        {statusBadges.map((badge) => {
          const clickable = Boolean(badge.onClick);
          const isIdle = badge.value.toLowerCase() === "idle";
          return (
            <button
              key={badge.id}
              type="button"
              onClick={badge.onClick}
              disabled={!clickable}
              className={cn(
                "inline-flex items-center self-start rounded-full border px-3 py-[6px] text-[11px] font-medium uppercase tracking-wide transition",
                isIdle
                  ? "border-primary/60 bg-muted text-muted-foreground"
                  : "border-primary/70 bg-background text-primary",
                clickable
                  ? "cursor-pointer hover:bg-primary/10 focus:outline-none focus:ring-2 focus:ring-primary/30"
                  : "cursor-default",
              )}
            >
              <span>{badge.label}</span>
              <span className="ml-2 text-[11px] capitalize">
                {badge.value.toLowerCase()}
              </span>
            </button>
          );
        })}
      </div>

      <div className="h-12"></div>

      <div className="flex-1 flex flex-col justify-center items-center gap-3">
        {(() => {
          const totalMsat = walletSummary?.balances
            ? calculateTotalMsat(walletSummary.balances)
            : walletBalance * 1000;

          const balanceDisplay = formatBalanceDisplay(totalMsat);

          return (
            <>
              <div className="text-[72px] font-semibold leading-none text-primary">
                {balanceDisplay.primary}
              </div>
              <span className="text-sm font-medium uppercase tracking-[0.35em] text-muted-foreground">
                {balanceDisplay.unit}
              </span>

              {balanceDisplay.secondary && (
                <div className="text-center">
                  <div className="text-lg text-muted-foreground">
                    {balanceDisplay.secondary}
                  </div>
                </div>
              )}
            </>
          );
        })()}
      </div>

      <div className="mt-auto flex gap-3 pb-2">
        <Button
          onClick={onReceive}
          variant="outline"
          className="flex-1 py-5 text-base font-semibold"
        >
          Receive
        </Button>
        <Button
          onClick={onSend}
          variant="outline"
          className="flex-1 py-5 text-base font-semibold"
        >
          Send
        </Button>
      </div>
    </Screen>
  );
}
