import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

const formatNumber = (value: number) =>
  Number.isFinite(value) ? Math.max(value, 0).toLocaleString() : "0";

type BudgetUsageProps = {
  used: number;
  total: number;
  unitLabel?: string;
  periodLabel?: string | null;
  className?: string;
};

export function BudgetUsage({
  used,
  total,
  unitLabel = "sats",
  periodLabel,
  className,
}: BudgetUsageProps) {
  const safeTotal = Math.max(total, 0);
  const safeUsed = Math.min(Math.max(used, 0), safeTotal || used);
  const remaining = Math.max(safeTotal - safeUsed, 0);
  const percentage = safeTotal === 0 ? 0 : Math.min(100, Math.round((safeUsed / safeTotal) * 100));

  return (
    <div className={cn("space-y-3", className)}>
      <div className="flex items-center justify-between text-xs">
        <span className="uppercase tracking-wide text-muted-foreground">Budget Usage</span>
        <span className="font-semibold text-primary">{percentage}%</span>
      </div>

      <div className="h-2 rounded-full bg-muted">
        <div
          className="h-full rounded-full bg-primary transition-all"
          style={{ width: `${percentage}%` }}
        />
      </div>

      <div className="grid grid-cols-2 gap-3 text-xs">
        <div>
          <span className="block text-[10px] uppercase tracking-wide text-muted-foreground">Used</span>
          <span className="text-sm font-medium text-foreground">
            {formatNumber(safeUsed)} {unitLabel}
          </span>
        </div>
        <div className="text-right">
          <span className="block text-[10px] uppercase tracking-wide text-muted-foreground">
            Remaining
          </span>
          <span className="text-sm font-medium text-foreground">
            {formatNumber(remaining)} {unitLabel}
          </span>
        </div>
      </div>

      <div className="flex items-center justify-between text-xs">
        <div>
          <span className="block text-[10px] uppercase tracking-wide text-muted-foreground">
            Total Budget
          </span>
          <span className="text-sm font-medium text-foreground">
            {formatNumber(safeTotal)} {unitLabel}
          </span>
        </div>
        {periodLabel ? (
          <Badge tone="default" className="uppercase tracking-wide">
            {periodLabel}
          </Badge>
        ) : null}
      </div>
    </div>
  );
}
