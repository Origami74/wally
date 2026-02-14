import QRCode from "react-qr-code";
import { CopyButton } from "@/components/copy-button";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { WalletTransactionEntry } from "@/lib/wallet/api";

function formatTimestamp(timestamp: number): string {
  if (!timestamp) return "";
  const date = new Date(timestamp * 1000);
  return date.toLocaleString();
}

type TransactionDetailDialogProps = {
  transaction: WalletTransactionEntry | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
};

export function TransactionDetailDialog({
  transaction,
  open,
  onOpenChange,
}: TransactionDetailDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center justify-between">
            <span>Transaction Details</span>
            {transaction && (
              <Badge
                tone={transaction.direction === "incoming" ? "success" : "warning"}
                className="uppercase"
              >
                {transaction.direction}
              </Badge>
            )}
          </DialogTitle>
        </DialogHeader>

        {transaction && (
          <div className="space-y-6 pt-4">
            <div className="flex flex-col items-center gap-2">
              <span className="text-3xl font-bold">
                {transaction.direction === "incoming" ? "+" : "-"}{" "}
                {transaction.amount.toLocaleString()} {transaction.unit}
              </span>
              <span className="text-sm text-muted-foreground">
                {formatTimestamp(transaction.timestamp)}
              </span>
            </div>

            <div className="grid gap-4 rounded-xl border border-dashed border-primary/20 bg-accent/5 p-4 text-sm">
              <div className="grid gap-1">
                <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground/70">
                  Mint URL
                </span>
                <span className="break-all font-mono text-xs">
                  {transaction.mint_url}
                </span>
              </div>

              {transaction.fee > 0 && (
                <div className="grid gap-1">
                  <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground/70">
                    Fee
                  </span>
                  <span>
                    {transaction.fee.toLocaleString()} {transaction.unit}
                  </span>
                </div>
              )}

              {transaction.memo && (
                <div className="grid gap-1">
                  <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground/70">
                    Memo
                  </span>
                  <span>{transaction.memo}</span>
                </div>
              )}

              {transaction.quote_id && (
                <div className="grid gap-1">
                  <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground/70">
                    Quote ID
                  </span>
                  <span className="break-all font-mono text-[10px]">
                    {transaction.quote_id}
                  </span>
                </div>
              )}

              <div className="grid gap-1">
                <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground/70">
                  Transaction ID
                </span>
                <span className="break-all font-mono text-[10px]">
                  {transaction.id}
                </span>
              </div>
            </div>

            {transaction.token && (
              <div className="space-y-4">
                <div className="flex flex-col items-center gap-4">
                  <div className="grid h-48 w-48 place-items-center rounded-3xl border-2 border-dashed border-primary/40 bg-white p-5">
                    <QRCode value={transaction.token} className="h-full w-full" />
                  </div>
                  <div className="w-full grid gap-2">
                    <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground/70">
                      Cashu Token
                    </span>
                    <p className="text-[10px] break-all p-3 bg-muted rounded-md border font-mono line-clamp-3">
                      {transaction.token}
                    </p>
                  </div>
                  <CopyButton
                    onCopy={() => navigator.clipboard.writeText(transaction.token!)}
                    label="Copy Token"
                    copiedLabel="Copied!"
                    className="w-full"
                  />
                </div>
              </div>
            )}
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
