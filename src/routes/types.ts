import type { ComponentProps } from "react";

import type { Badge } from "@/components/ui/badge";
import type { ServiceStatus } from "@/lib/tollgate/types";

export const periods = [
  { value: "day", label: "day", human: "today" },
  { value: "week", label: "week", human: "this week" },
  { value: "month", label: "month", human: "this month" },
] as const;

export type Period = (typeof periods)[number]["value"];

export type StatusBadge = {
  id: string;
  label: string;
  value: string;
  tone: ComponentProps<typeof Badge>["tone"];
  onClick?: () => void;
};

export type FeatureState = {
  id: "tollgate" | "402" | "routstr" | "nwc";
  title: string;
  description: string;
  enabled: boolean;
  budget: string;
  period: Period;
  spent: number;
  infoOpen: boolean;
};

export type PeriodMetaFn = (period: Period) => (typeof periods)[number];

export type CopyHandler = () => Promise<void> | void;

export type SettingsController = {
  status: ServiceStatus | null;
  features: FeatureState[];
};
