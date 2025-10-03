import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Screen } from "@/components/layout/screen";
import { formatBytes, formatDuration } from "@/lib/tollgate/utils";
import type { NetworkInfo, SessionInfo } from "@/lib/tollgate/types";
import type { StatusBadge } from "./types";
import { cn } from "@/lib/utils";

type HomeScreenProps = {
  statusBadges: StatusBadge[];
  walletBalance: number;
  currentSession: SessionInfo | null;
  currentNetwork: NetworkInfo | null;
  onReceive: () => void;
  onSend: () => void;
};

export function HomeScreen({
  statusBadges,
  walletBalance,
  currentSession,
  currentNetwork,
  onReceive,
  onSend,
}: HomeScreenProps) {
  return (
    <Screen className="relative h-full gap-6">
      <div className="absolute left-4 top-4 flex flex-col gap-2">
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
                  : "cursor-default"
              )}
            >
              <span>{badge.label}</span>
              <span className="ml-2 text-[11px] capitalize">{badge.value.toLowerCase()}</span>
            </button>
          );
        })}
      </div>

      {/* Spacer about the height of the buttons */}
      <div className="h-12"></div>

      <div className="flex-1 flex flex-col justify-center items-center gap-3">
        <div className="text-[72px] font-semibold leading-none text-primary">
          {walletBalance.toLocaleString()}
        </div>
        <span className="text-sm font-medium uppercase tracking-[0.35em] text-muted-foreground">
          sats
        </span>
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

      <div className="mt-auto flex gap-3 pb-2">
        <Button onClick={onReceive} variant="outline" className="flex-1 py-5 text-base font-semibold">
          Receive
        </Button>
        <Button onClick={onSend} variant="outline" className="flex-1 py-5 text-base font-semibold">
          Send
        </Button>
      </div>
    </Screen>
  );
}
