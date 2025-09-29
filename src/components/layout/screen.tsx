import type { PropsWithChildren } from "react";

import { cn } from "@/lib/utils";

export function Screen({ children, className }: PropsWithChildren<{ className?: string }>) {
  return <section className={cn("flex h-full flex-col overflow-hidden", className)}>{children}</section>;
}
