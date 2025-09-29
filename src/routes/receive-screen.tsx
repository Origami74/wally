import { Screen } from "@/components/layout/screen";
import { CopyButton } from "@/components/copy-button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Button } from "@/components/ui/button";

type ReceiveScreenProps = {
  onBack: () => void;
  onCopy: () => Promise<void> | void;
};

export function ReceiveScreen({ onBack, onCopy }: ReceiveScreenProps) {
  return (
    <Screen className="h-screen gap-4">
        <h2 className="text-lg font-semibold uppercase tracking-[0.2em] text-muted-foreground">Receive</h2>

      <div className="flex flex-1 flex-col items-center justify-center gap-5">
        <div className="grid h-48 w-48 place-items-center rounded-3xl border-2 border-dashed border-primary/40 bg-muted text-xs font-medium text-muted-foreground">
          QR preview
        </div>
        <div className="grid w-full max-w-xs gap-2">
          <Label htmlFor="receive-amount">Optional amount (sats)</Label>
          <Input id="receive-amount" type="number" min={0} placeholder="Add an amount" />
        </div>
      </div>

      <div className="mt-auto flex gap-3 pb-2">
        <CopyButton onCopy={onCopy} label="Copy invoice" copiedLabel="Copied" className="flex-1" />
        <Button variant="outline" onClick={onBack} className="flex-1">
          Cancel
        </Button>
      </div>
    </Screen>
  );
}
