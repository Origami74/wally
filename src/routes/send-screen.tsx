import { useMemo, useState } from "react";

import { Screen } from "@/components/layout/screen";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { payBolt11Invoice, payNut18PaymentRequest } from "@/lib/wallet/api";

type SendScreenProps = {
  onBack: () => void;
  request: string;
  onChangeRequest: (value: string) => void;
  onPaymentComplete: () => Promise<void>;
};

export function SendScreen({
  onBack,
  request,
  onChangeRequest,
  onPaymentComplete,
}: SendScreenProps) {
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const requestType = useMemo(() => {
    const trimmed = request.trim().toLowerCase();
    if (!trimmed) return null;
    if (trimmed.startsWith("creqa")) return "cashu" as const;
    if (trimmed.startsWith("ln")) return "lightning" as const;
    return "unknown" as const;
  }, [request]);

  const handleSubmit = async () => {
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

  return (
    <Screen className="h-screen gap-4">
      <h2 className="text-lg font-semibold uppercase tracking-[0.2em] text-muted-foreground">
        Send
      </h2>

      <div className="flex-1">
        <div className="grid gap-2">
          <Label htmlFor="send-request">Payment request</Label>
          <Textarea
            id="send-request"
            placeholder="Paste a Cashu request or Lightning invoice"
            value={request}
            onChange={(event) => onChangeRequest(event.target.value)}
            disabled={isSubmitting}
          />
        </div>
        {requestType ? (
          <p className="mt-3 text-sm text-muted-foreground">
            {requestType === "cashu"
              ? "Detected Cashu NUT-18 payment request."
              : requestType === "lightning"
              ? "Detected Lightning BOLT11 invoice."
              : "Unknown request format."}
          </p>
        ) : null}
        {error ? <p className="mt-3 text-sm text-destructive">{error}</p> : null}
      </div>

      <div className="mt-auto flex gap-3 pb-2">
        <Button
          onClick={handleSubmit}
          disabled={isSubmitting || !request.trim()}
          className="flex-1"
        >
          {isSubmitting ? "Paying…" : "Send payment"}
        </Button>
        <Button variant="outline" onClick={onBack} className="flex-1" disabled={isSubmitting}>
          Cancel
        </Button>
      </div>
    </Screen>
  );
}
