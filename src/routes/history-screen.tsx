import { Screen } from "@/components/layout/screen";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import type { WalletTransactionEntry } from "@/lib/wallet/api";

function formatTimestamp(timestamp: number): string {
  if (!timestamp) return "";
  const date = new Date(timestamp * 1000);
  return date.toLocaleString();
}

type HistoryScreenProps = {
  transactions: WalletTransactionEntry[];
};

export function HistoryScreen({ transactions }: HistoryScreenProps) {
  return (
    <Screen className="min-h-screen gap-6 overflow-y-auto pb-4 pr-16 pt-6">
      <h2 className="text-lg font-semibold uppercase tracking-[0.2em] text-muted-foreground">
        History
      </h2>

      {transactions.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          No transactions yet. Generate a receive request or pay an invoice to populate your history.
        </p>
      ) : (
        <div className="space-y-3">
          {transactions.map((tx) => {
            const isIncoming = tx.direction === "incoming";
            const amountDisplay = `${isIncoming ? "+" : "-"}${tx.amount.toLocaleString()} ${tx.unit}`;

            return (
              <Card
                key={tx.id}
                className="space-y-3 border border-dashed border-primary/20 bg-background/90 p-4"
              >
                <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
                  <div className="flex items-center gap-2">
                    <Badge tone={isIncoming ? "success" : "destructive"}>
                      {isIncoming ? "Incoming" : "Outgoing"}
                    </Badge>
                    <span className="text-xs text-muted-foreground">
                      {formatTimestamp(tx.timestamp)}
                    </span>
                  </div>
                  <span className="text-sm font-semibold text-foreground sm:text-right">
                    {amountDisplay}
                  </span>
                </div>

                <div className="space-y-1 text-xs text-muted-foreground">
                  <p className="truncate">Mint: {tx.mint_url}</p>
                  {tx.fee > 0 ? (
                    <p>Fee: {tx.fee.toLocaleString()} {tx.unit}</p>
                  ) : null}
                  {tx.memo ? <p>Memo: {tx.memo}</p> : null}
                  {tx.quote_id ? <p>Quote ID: {tx.quote_id}</p> : null}
                  <p className="text-[11px] uppercase tracking-wide text-muted-foreground/80">
                    Tx ID: {tx.id}
                  </p>
                </div>
              </Card>
            );
          })}
        </div>
      )}
    </Screen>
  );
}
