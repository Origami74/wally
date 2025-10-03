import type { PropsWithChildren, ReactNode } from "react";

import { cn } from "@/lib/utils";

interface SectionHeaderProps {
  title: string;
  description?: string;
  actions?: ReactNode;
  className?: string;
}

export function SectionHeader({
  title,
  description,
  actions,
  className,
}: PropsWithChildren<SectionHeaderProps>) {
  return (
    <div className={cn("space-y-2", className)}>
      <h2 className="text-lg font-semibold uppercase tracking-[0.2em] text-muted-foreground">
        {title}
      </h2>
      {description ? <p className="text-sm text-muted-foreground">{description}</p> : null}
      {actions ? <div className="flex flex-wrap gap-2">{actions}</div> : null}
    </div>
  );
}

export default SectionHeader;
