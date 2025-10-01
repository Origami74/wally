import { useEffect, useMemo, useState } from "react";
import QRCode from "react-qr-code";

import { Screen } from "@/components/layout/screen";
import { CopyButton } from "@/components/copy-button";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  createBolt11Invoice,
  createNut18PaymentRequest,
  type Bolt11InvoiceInfo,
  type Nut18PaymentRequestInfo,
  type SwapRequest,
} from "@/lib/wallet/api";

const MODES = [
  { id: "cashu", label: "Cashu" },
  { id: "lightning", label: "Lightning" },
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
  const [description, setDescription] = useState("");
  const [token, setCashuToken] = useState<SwapRequest | null>(null);
  const [cashuRequest, setCashuRequest] = useState<Nut18PaymentRequestInfo | null>(null);
  const [lightningInvoice, setLightningInvoice] = useState<Bolt11InvoiceInfo | null>(null);
  const [isGenerating, setIsGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const activeRequest = mode === "cashu" ? cashuRequest : lightningInvoice;
  const qrValue = activeRequest?.request ?? "";

  useEffect(() => {
    let cancelled = false;

    const run = async () => {
      const trimmedAmount = amount.trim();
      const trimmedDescription = description.trim() || null;

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
          const request = await createNut18PaymentRequest(numericAmount, trimmedDescription);
          if (!cancelled) {
            setCashuRequest(request);
          }
        } else {
          const numeric = Number(trimmedAmount);
          const invoice = await createBolt11Invoice(numeric, trimmedDescription);
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
  }, [mode, amount, description]);

  const mintLabel = useMemo(() => {
    if (mode === "cashu") {
      if (cashuRequest?.mints?.length) {
        return cashuRequest.mints.join(", ");
      }
      return defaultMint ?? "";
    }

    return lightningInvoice?.mint_url ?? defaultMint ?? "";
  }, [mode, cashuRequest, lightningInvoice, defaultMint]);

  const formattedExpiry = useMemo(() => {
    if (!lightningInvoice || mode !== "lightning") return null;
    const expiresAt = new Date(lightningInvoice.expiry * 1000);
    return expiresAt.toLocaleString();
  }, [lightningInvoice, mode]);

  const formattedAmount = useMemo(() => {
    if (!activeRequest) return null;
    if (mode === "cashu") {
      return activeRequest.amount ?? null;
    }
    return lightningInvoice?.amount ?? null;
  }, [activeRequest, mode, lightningInvoice]);

  const handleCopy = async () => {
    if (!qrValue) return;
    await copyToClipboard(qrValue);
  };

  return (
    <Screen className="h-screen gap-4">
      <h2 className="text-lg font-semibold uppercase tracking-[0.2em] text-muted-foreground">
        Receive
      </h2>

      <div className="flex flex-col gap-3">
        <div className="flex gap-2">
          {MODES.map(({ id, label }) => (
            <Button
              key={id}
              variant={mode === id ? "default" : "outline"}
              size="sm"
              className="flex-1 rounded-full"
              onClick={() => {
                setMode(id);
                setAmount("");
                setDescription("");
                setCashuRequest(null);
                setLightningInvoice(null);
                setError(null);
              }}
              disabled={isGenerating}
            >
              {label}
            </Button>
          ))}
        </div>

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

        <div className="grid gap-2">
          <Label htmlFor="receive-description">Description (optional)</Label>
          <Input
            id="receive-description"
            placeholder="Add a note for the payer"
            value={description}
            onChange={(event) => setDescription(event.target.value)}
            disabled={isGenerating}
          />
        </div>

        <div className="grid gap-2">
          <Label htmlFor="cashu-token">Cashu token</Label>
          <Input
            id="cashu-token"
            placeholder="Add a cashu token"
            value={description}
            onChange={(event) => setCashuToken(event.target.value)}
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

      <div className="flex flex-1 flex-col items-center justify-center gap-4">
        <div className="grid h-40 w-40 place-items-center rounded-3xl border-2 border-dashed border-primary/40 bg-muted p-5">
          {qrValue ? (
            <QRCode value={qrValue} className="h-full w-full" />
          ) : (
            <span className="text-xs font-medium text-muted-foreground">
              {mode === "lightning"
                ? "Provide an amount to generate a Lightning invoice"
                : "Generating Cashu request..."}
            </span>
          )}
        </div>
      </div>

      <div className="mt-auto flex gap-3 pb-2">
        <CopyButton
          onCopy={handleCopy}
          label={isGenerating ? "Preparing…" : "Copy request"}
          copiedLabel="Copied"
          className="flex-1"
          disabled={!qrValue || isGenerating}
        />
        <Button variant="outline" onClick={onBack} className="flex-1" disabled={isGenerating}>
          Cancel
        </Button>
      </div>
    </Screen>
  );
}
