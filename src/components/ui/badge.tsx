import * as React from "react";

import { cn } from "@/lib/utils";

export interface BadgeProps extends React.HTMLAttributes<HTMLDivElement> {
  tone?: "default" | "success" | "warning" | "danger" | "info";
}

const toneStyles: Record<Required<BadgeProps>["tone"], string> = {
  default: "bg-muted text-muted-foreground",
  success: "bg-emerald-100 text-emerald-700",
  warning: "bg-amber-100 text-amber-700",
  danger: "bg-red-100 text-red-700",
  info: "bg-primary/10 text-primary",
};

const Badge = React.forwardRef<HTMLDivElement, BadgeProps>(
  ({ className, tone = "default", ...props }, ref) => (
    <div
      ref={ref}
      className={cn(
        "inline-flex items-center rounded-full px-3 py-1 text-xs font-medium",
        toneStyles[tone],
        className
      )}
      {...props}
    />
  )
);
Badge.displayName = "Badge";

export { Badge };
