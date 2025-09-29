import { useState } from "react";
import type { ComponentProps } from "react";
import { Copy } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

const COPY_TIMEOUT_MS = 1500;

type CopyButtonProps = {
  onCopy: () => Promise<void> | void;
  label: string;
  copiedLabel: string;
  disabled?: boolean;
  className?: string;
  variant?: ComponentProps<typeof Button>["variant"];
};

export function CopyButton({ onCopy, label, copiedLabel, disabled, className, variant }: CopyButtonProps) {
  const [copied, setCopied] = useState(false);

  const handleClick = async () => {
    await onCopy();
    setCopied(true);
    setTimeout(() => setCopied(false), COPY_TIMEOUT_MS);
  };

  return (
    <Button
      variant={variant}
      onClick={handleClick}
      disabled={disabled}
      className={cn("flex items-center justify-center gap-2", className)}
    >
      <Copy className="h-4 w-4" /> {copied ? copiedLabel : label}
    </Button>
  );
}
