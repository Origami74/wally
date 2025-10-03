import { useEffect, useMemo, useState } from "react";
import QRCode from "react-qr-code";
import { Zap, FileText, Nut } from "lucide-react";

import { Screen } from "@/components/layout/screen";
import { CopyButton } from "@/components/copy-button";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  createBolt11Invoice,
  createNut18PaymentRequest,
  receiveCashuToken,
  type Bolt11InvoiceInfo,
  type Nut18PaymentRequestInfo,
} from "@/lib/wallet/api";

const MODES = [
  { id: "cashu", label: "Cashu", icon: Nut },
  { id: "lightning", label: "Lightning", icon: Zap },
  { id: "redeem", label: "Redeem", icon: FileText },
] as const;

type ReceiveMode = (typeof MODES)[number]["id"];

type ReceiveScreenProps = {
  onBack: () => void;
  copyToClipboard: (value: string) => Promise<void> | void;
  defaultMint?: string;
};

export function ReceiveScreen({ onBack, copyToClipboard, defaultMint }: ReceiveScreenProps) {
  const [mode, setMode] = useState<ReceiveMode>("cashu");
  const [amount, setAmount] = useState("");
  const [cashuTokenInput, setCashuTokenInput] = useState("");
  const [cashuRequest, setCashuRequest] = useState<Nut18PaymentRequestInfo | null>(null);
  const [lightningInvoice, setLightningInvoice] = useState<Bolt11InvoiceInfo | null>(null);
  const [isGenerating, setIsGenerating] = useState(false);
  const [isReceiving, setIsReceiving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const activeRequest = mode === "cashu" ? cashuRequest : mode === "lightning" ? lightningInvoice : null;
  const qrValue = activeRequest?.request ?? "";

  useEffect(() => {
    if (mode === "redeem") {
      setCashuRequest(null);
      setLightningInvoice(null);
      setIsGenerating(false);
      setError(null);
      return;
    }

    let cancelled = false;

    const run = async () => {
      const trimmedAmount = amount.trim();

      if (mode === "lightning") {
        const numeric = Number(trimmedAmount);
        if (!trimmedAmount) {
          setLightningInvoice(null);
          setError("Enter the amount in sats for a Lightning invoice.");
          return;
        }
        if (Number.isNaN(numeric) || numeric <= 0 || !Number.isInteger(numeric)) {
          setLightningInvoice(null);
          setError("Lightning invoices require a whole number of sats.");
          return;
        }
      } else if (
        trimmedAmount &&
        (Number.isNaN(Number(trimmedAmount)) || Number(trimmedAmount) < 0 || !Number.isInteger(Number(trimmedAmount)))
      ) {
        setCashuRequest(null);
        setError("Enter a whole number of sats.");
        return;
      }

      setIsGenerating(true);
      setError(null);

      try {
        if (mode === "cashu") {
          const numericAmount = trimmedAmount ? Number(trimmedAmount) : null;
          const request = await createNut18PaymentRequest(numericAmount, null);
          if (!cancelled) {
            setCashuRequest(request);
          }
        } else {
          const numeric = Number(trimmedAmount);
          const invoice = await createBolt11Invoice(numeric, null);
          if (!cancelled) {
            setLightningInvoice(invoice);
          }
        }
      } catch (err) {
        if (!cancelled) {
          console.error("Failed to create receive request", err);
          setError(
            mode === "cashu"
              ? "Unable to create a Cashu payment request."
              : "Unable to create a Lightning invoice."
          );
        }
      } finally {
        if (!cancelled) {
          setIsGenerating(false);
        }
      }
    };

    run();

    return () => {
      cancelled = true;
    };
  }, [mode, amount]);

  const handleModeChange = (nextMode: ReceiveMode) => {
    setMode(nextMode);
    setAmount("");
    setCashuRequest(null);
    setLightningInvoice(null);
    setCashuTokenInput("");
    setIsGenerating(false);
    setIsReceiving(false);
    setError(null);
    setSuccess(null);
  };

  const mintLabel = useMemo(() => {
    if (mode === "cashu") {
      if (cashuRequest?.mints?.length) {
        return cashuRequest.mints.join(", ");
      }
      return defaultMint ?? "";
    }

    if (mode === "redeem") {
      return "";
    }

    return lightningInvoice?.mint_url ?? defaultMint ?? "";
  }, [mode, cashuRequest, lightningInvoice, defaultMint]);

  const formattedExpiry = useMemo(() => {
    if (!lightningInvoice || mode !== "lightning") return null;

    const expiresAt = new Date(lightningInvoice.expiry * 1000);
    const now = new Date();
    const diff = expiresAt.getTime() - now.getTime();
    if (Number.isNaN(diff) || diff <= 0) return "Expired";

    const minutes = Math.floor(diff / (1000 * 60));
    const seconds = Math.floor((diff % (1000 * 60)) / 1000);
    return `${minutes}m ${seconds}s`;
  }, [lightningInvoice, mode]);

  const formattedAmount = useMemo(() => {
    if (!activeRequest || mode === "redeem") return null;
    if (mode === "cashu") {
      return activeRequest.amount ?? null;
    }
    return lightningInvoice?.amount ?? null;
  }, [activeRequest, mode, lightningInvoice]);

  const handleCopy = async () => {
    if (!qrValue) return;
    await copyToClipboard(qrValue);
  };

  const handleReceiveToken = async () => {
    if (!cashuTokenInput.trim()) return;

    setIsReceiving(true);
    setError(null);
    setSuccess(null);

    try {
      const result = await receiveCashuToken(cashuTokenInput.trim());
      setSuccess(`Successfully received ${result.amount} sats!`);
      setCashuTokenInput("");
    } catch (err) {
      console.error("Failed to receive token", err);
      setError("Failed to receive token. Check the token and try again.");
    } finally {
      setIsReceiving(false);
    }
  };

  return (
    <Screen className="min-h-screen flex flex-col gap-4">
      <div className="flex items-start justify-between">
        <h2 className="text-lg font-semibold uppercase tracking-[0.2em] text-muted-foreground">
          Receive
        </h2>
        <div className="flex items-center gap-2">
          {MODES.map(({ id, label, icon: Icon }) => (
            <Button
              key={id}
              variant={mode === id ? "default" : "outline"}
              size="icon"
              className="h-10 w-10 rounded-full"
              onClick={() => handleModeChange(id)}
              disabled={isGenerating && mode !== "redeem"}
              aria-label={`Switch to ${label} receive mode`}
            >
              <Icon className="h-5 w-5" />
            </Button>
          ))}
        </div>
      </div>

      {mode !== "redeem" ? (
        <div className="flex flex-col gap-3 flex-shrink-0">
          <div className="grid gap-2">
            <Label htmlFor="receive-amount">
              {mode === "lightning" ? "Amount (sats)" : "Optional amount (sats)"}
            </Label>
            <Input
              id="receive-amount"
              type="number"
              min={0}
              inputMode="numeric"
              placeholder={mode === "lightning" ? "Enter amount" : "Add an amount"}
              value={amount}
              onChange={(event) => setAmount(event.target.value)}
              disabled={isGenerating}
            />
          </div>

          {mintLabel ? (
            <p className="text-sm text-muted-foreground">Mint: {mintLabel}</p>
          ) : null}

          {formattedAmount !== null ? (
            <p className="text-sm text-muted-foreground">
              Amount: <span className="font-medium text-foreground">{formattedAmount} sats</span>
            </p>
          ) : null}

          {mode === "lightning" && formattedExpiry ? (
            <p className="text-sm text-muted-foreground">Expires: {formattedExpiry}</p>
          ) : null}

          {error ? (
            <p className="text-sm text-destructive">{error}</p>
          ) : null}
        </div>
      ) : (
        <div className="flex flex-col gap-3">
          <div className="grid gap-2">
            <Label htmlFor="cashu-token">Paste Cashu token to receive</Label>
            <Textarea
              id="cashu-token"
              rows={5}
              placeholder="cashuAeyJ0b2tlbiI6W3sibWludCI6Imh0dHBzOi8v..."
              value={cashuTokenInput}
              onChange={(event) => setCashuTokenInput(event.target.value)}
              disabled={isReceiving}
            />
          </div>

          <div className="flex gap-3">
            <Button
              onClick={handleReceiveToken}
              disabled={isReceiving || !cashuTokenInput.trim()}
              className="flex-1"
            >
              {isReceiving ? "Receiving…" : "Receive token"}
            </Button>
            <Button
              variant="outline"
              onClick={() => setCashuTokenInput("")}
              disabled={isReceiving || !cashuTokenInput}
            >
              Clear
            </Button>
          </div>

          {error ? <p className="text-sm text-destructive">{error}</p> : null}
          {success ? <p className="text-sm text-green-600">{success}</p> : null}
        </div>
      )}

      {mode !== "redeem" ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-4 min-h-0">
          <div className="grid h-40 w-40 place-items-center rounded-3xl border-2 border-dashed border-primary/40 bg-muted p-5 flex-shrink-0">
            {qrValue ? (
              <QRCode value={qrValue} className="h-full w-full" />
            ) : (
              <div className="flex h-full w-full items-center justify-center text-center text-sm text-muted-foreground">
                {mode === "lightning"
                  ? "Provide an amount to generate a Lightning invoice."
                  : "Add an amount (optional) to generate a Cashu request."}
              </div>
            )}
          </div>
        </div>
      ) : (
        <div className="flex-1" />
      )}

      {mode !== "redeem" ? (
        <div className="flex gap-3 pb-2 flex-shrink-0">
          <CopyButton
            onCopy={handleCopy}
            label={isGenerating ? "Preparing…" : "Copy request"}
            copiedLabel="Copied!"
            disabled={!qrValue || isGenerating}
            className="flex-1"
          />
          <Button variant="outline" onClick={onBack} className="flex-1" disabled={isGenerating}>
            Cancel
          </Button>
        </div>
      ) : (
        <div className="flex gap-3 pb-2 flex-shrink-0">
          <Button variant="outline" onClick={onBack} className="flex-1" disabled={isReceiving}>
            Done
          </Button>
        </div>
      )}
    </Screen>
  );
}
