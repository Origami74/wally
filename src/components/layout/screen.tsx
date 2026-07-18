import type { PropsWithChildren } from "react";

import { cn } from "@/lib/utils";

export function Screen({ children, className }: PropsWithChildren<{ className?: string }>) {
  const isTransparent = className?.includes("bg-transparent");
  return (
    <section
      className={cn(
        "flex h-full flex-col overflow-hidden p-4",
        // Force background if not explicitly transparent
        !isTransparent && "bg-background",
        className
      )}
      style={{
        paddingTop: "calc(env(safe-area-inset-top) + 1rem)",
        paddingBottom: "calc(env(safe-area-inset-bottom) + 1rem)",
        paddingLeft: "calc(env(safe-area-inset-left) + 1rem)",
        paddingRight: "calc(env(safe-area-inset-right) + 1rem)",
      }}
    >
      {children}
    </section>
  );
}
