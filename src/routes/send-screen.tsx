import { Screen } from "@/components/layout/screen";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";

type SendScreenProps = {
  onBack: () => void;
  request: string;
  onChangeRequest: (value: string) => void;
  onSubmit: () => void;
};

export function SendScreen({ onBack, request, onChangeRequest, onSubmit }: SendScreenProps) {
  const canSend = request.trim().length > 0;

  return (
    <Screen className="gap-6 pt-6">
      <div className="text-left">
        <h1 className="text-3xl font-semibold">Send</h1>
      </div>

      <div className="flex-1">
        <div className="grid gap-2">
          <Label htmlFor="send-request">Payment request</Label>
          <Textarea
            id="send-request"
            placeholder="Paste payment request here"
            value={request}
            onChange={event => onChangeRequest(event.target.value)}
          />
        </div>
      </div>

      <div className="mt-auto flex gap-3 pb-2">
        <Button onClick={onSubmit} disabled={!canSend} className="flex-1">
          Send payment
        </Button>
        <Button variant="outline" onClick={onBack} className="flex-1">
          Cancel
        </Button>
      </div>
    </Screen>
  );
}
