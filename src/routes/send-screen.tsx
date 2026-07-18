import { useMemo, useState, useEffect } from "react";
import { ScanQrCode, Send, Ticket } from "lucide-react";
import QRCode from "react-qr-code";

import { Screen } from "@/components/layout/screen";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { CopyButton } from "@/components/copy-button";
import {
  payBolt11Invoice,
  payNut18PaymentRequest,
  createExternalToken,
  type WalletSummary,
} from "@/lib/wallet/api";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { identifyRequest } from "@/lib/wallet/utils";
import { cn } from "@/lib/utils";

const MODES = [
  { id: "pay", label: "Pay Request", icon: Send },
  { id: "token", label: "Create Token", icon: Ticket },
] as const;

type SendMode = (typeof MODES)[number]["id"];

type SendScreenProps = {
  onBack: () => void;
  request: string;
  onChangeRequest: (value: string) => void;
  onPaymentComplete: () => Promise<void>;
  onScan: () => void;
  walletSummary: WalletSummary | null;
};

export function SendScreen({
  onBack,
  request,
  onChangeRequest,
  onPaymentComplete,
  onScan,
  walletSummary,
}: SendScreenProps) {
  const [mode, setMode] = useState<SendMode>("pay");
  const [amount, setAmount] = useState("");
  const [selectedMint, setSelectedMint] = useState<string>(
    walletSummary?.default_mint || walletSummary?.balances[0]?.mint_url || "",
  );
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [generatedToken, setGeneratedToken] = useState<string | null>(null);

  useEffect(() => {
    if (!selectedMint && walletSummary) {
      setSelectedMint(
        walletSummary.default_mint || walletSummary.balances[0]?.mint_url || "",
      );
    }
  }, [walletSummary, selectedMint]);

  const requestType = useMemo(() => {
    if (mode !== "pay") return null;
    const type = identifyRequest(request);
    if (type === "cashu-request") return "cashu" as const;
    if (type === "lightning-invoice") return "lightning" as const;
    return "unknown" as const;
  }, [request, mode]);

  const handlePay = async () => {
    const trimmed = request.trim();
    if (!trimmed) {
      setError("Paste a Cashu request or Lightning invoice to continue.");
      return;
    }

    if (requestType === "unknown") {
      setError("Unsupported payment request format.");
      return;
    }

    setIsSubmitting(true);
    setError(null);
    try {
      if (requestType === "cashu") {
        await payNut18PaymentRequest(trimmed, null);
      } else if (requestType === "lightning") {
        await payBolt11Invoice(trimmed);
      }

      await onPaymentComplete();
    } catch (err) {
      console.error("Payment failed", err);
      setError("Payment failed. Check your balance and try again.");
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCreateToken = async () => {
    const numericAmount = parseInt(amount, 10);
    if (isNaN(numericAmount) || numericAmount <= 0) {
      setError("Please enter a valid amount.");
      return;
    }

    setIsSubmitting(true);
    setError(null);
    try {
      const token = await createExternalToken(
        numericAmount,
        selectedMint || undefined,
      );
      setGeneratedToken(token);
    } catch (err) {
      console.error("Failed to create token", err);
      setError("Failed to create token. Check your balance.");
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleModeChange = (newMode: SendMode) => {
    setMode(newMode);
    setError(null);
    setGeneratedToken(null);
  };

  return (
    <Screen className="gap-4">
      <div className="flex flex-col gap-4 flex-shrink-0">
        <h2 className="text-lg font-semibold uppercase tracking-[0.2em] text-muted-foreground">
          Send
        </h2>
        
        <div className="flex p-1 bg-muted rounded-lg">
          {MODES.map((m) => {
            const Icon = m.icon;
            const isActive = mode === m.id;
            return (
              <button
                key={m.id}
                onClick={() => handleModeChange(m.id)}
                disabled={isSubmitting}
                className={cn(
                  "flex-1 flex items-center justify-center gap-2 py-2 text-sm font-medium rounded-md transition-all",
                  isActive 
                    ? "bg-background text-foreground shadow-sm" 
                    : "text-muted-foreground hover:text-foreground"
                )}
              >
                <Icon className="h-4 w-4" />
                {m.label}
              </button>
            );
          })}
        </div>
      </div>

      <div className="flex-1 flex flex-col gap-4 overflow-y-auto pr-1">
        {mode === "pay" ? (
          <div className="grid gap-2">
            <div className="flex items-center justify-between">
              <Label htmlFor="send-request">Payment request</Label>
              <Button
                variant="outline"
                size="sm"
                onClick={onScan}
                disabled={isSubmitting}
                className="h-8 gap-2"
              >
                <ScanQrCode className="h-4 w-4" />
                Scan QR
              </Button>
            </div>
            <Textarea
              id="send-request"
              placeholder="Paste a Cashu request or Lightning invoice"
              value={request}
              onChange={(event) => onChangeRequest(event.target.value)}
              disabled={isSubmitting}
              rows={4}
            />
            {requestType && (
              <p className="text-sm text-muted-foreground">
                {requestType === "cashu"
                  ? "Detected Cashu NUT-18 payment request."
                  : requestType === "lightning"
                  ? "Detected Lightning BOLT11 invoice."
                  : "Unknown request format."}
              </p>
            )}
          </div>
        ) : (
          <div className="grid gap-4 pb-4">
            {!generatedToken ? (
              <div className="grid gap-4">
                <div className="grid gap-2">
                  <Label htmlFor="token-amount">Amount (sats)</Label>
                  <Input
                    id="token-amount"
                    type="number"
                    placeholder="Enter amount to send"
                    value={amount}
                    onChange={(e) => setAmount(e.target.value)}
                    disabled={isSubmitting}
                  />
                </div>

                <div className="grid gap-2">
                  <Label>Select Mint</Label>
                  <Select
                    value={selectedMint}
                    onValueChange={setSelectedMint}
                    disabled={isSubmitting}
                  >
                    <SelectTrigger className="h-auto py-2">
                      <SelectValue placeholder="Select a mint" />
                    </SelectTrigger>
                    <SelectContent>
                      {walletSummary?.balances.map((balance) => {
                        let hostname = balance.mint_url;
                        try {
                          hostname = new URL(balance.mint_url).hostname;
                        } catch {
                          // ignore
                        }

                        return (
                          <SelectItem
                            key={balance.mint_url}
                            value={balance.mint_url}
                          >
                            <div className="flex flex-col items-start gap-0.5">
                              <span className="text-xs font-medium">
                                {balance.balance} {balance.unit}
                              </span>
                              <span className="text-[10px] text-muted-foreground opacity-80 break-all text-left line-clamp-1">
                                {hostname}
                              </span>
                            </div>
                          </SelectItem>
                        );
                      })}
                    </SelectContent>
                  </Select>
                </div>
              </div>
            ) : (
              <div className="flex flex-col items-center gap-4">
                <div className="grid h-48 w-48 place-items-center rounded-3xl border-2 border-dashed border-primary/40 bg-white p-5">
                  <QRCode value={generatedToken} className="h-full w-full" />
                </div>
                <div className="w-full grid gap-2">
                  <Label>Token Data</Label>
                  <p className="text-[10px] break-all p-3 bg-muted rounded-md border font-mono line-clamp-4">
                    {generatedToken}
                  </p>
                </div>
                <CopyButton
                  onCopy={() => navigator.clipboard.writeText(generatedToken)}
                  label="Copy Token"
                  copiedLabel="Copied!"
                  className="w-full"
                />
              </div>
            )}
          </div>
        )}

        {error ? <p className="text-sm text-destructive">{error}</p> : null}
      </div>

      <div className="mt-auto flex gap-3 pb-2">
        {mode === "pay" ? (
          <Button
            onClick={handlePay}
            disabled={isSubmitting || !request.trim()}
            className="flex-1"
          >
            {isSubmitting ? "Paying…" : "Send payment"}
          </Button>
        ) : !generatedToken ? (
          <Button
            onClick={handleCreateToken}
            disabled={isSubmitting || !amount}
            className="flex-1"
          >
            {isSubmitting ? "Creating…" : "Create Token"}
          </Button>
        ) : (
          <Button onClick={onPaymentComplete} className="flex-1">
            Done
          </Button>
        )}
        <Button variant="outline" onClick={onBack} className="flex-1" disabled={isSubmitting}>
          {generatedToken ? "Back" : "Cancel"}
        </Button>
      </div>
    </Screen>
  );
}
