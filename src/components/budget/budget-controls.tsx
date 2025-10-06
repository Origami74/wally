import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";

type PeriodOption = {
  value: string;
  label: string;
};

type BudgetControlsProps = {
  idPrefix: string;
  budgetValue: string;
  onBudgetChange: (value: string) => void;
  onBudgetBlur?: () => void;
  periodValue: string;
  onPeriodChange: (value: string) => void;
  periodOptions: PeriodOption[];
  budgetLabel?: string;
  periodLabel?: string;
  minBudget?: number;
  disabled?: boolean;
  className?: string;
};

export function BudgetControls({
  idPrefix,
  budgetValue,
  onBudgetChange,
  onBudgetBlur,
  periodValue,
  onPeriodChange,
  periodOptions,
  budgetLabel = "Budget",
  periodLabel = "Per",
  minBudget = 0,
  disabled = false,
  className,
}: BudgetControlsProps) {
  return (
    <div className={cn("grid gap-3 text-sm", className)}>
      <div className="grid gap-2 sm:grid-cols-[1fr_auto] sm:items-end">
        <div className="grid gap-2">
          <Label htmlFor={`${idPrefix}-budget`}>{budgetLabel}</Label>
          <Input
            id={`${idPrefix}-budget`}
            type="number"
            min={minBudget}
            value={budgetValue}
            onChange={(event) => onBudgetChange(event.target.value)}
            onBlur={onBudgetBlur}
            disabled={disabled}
          />
        </div>
        <div className="grid gap-2">
          <Label htmlFor={`${idPrefix}-period`}>{periodLabel}</Label>
          <Select
            value={periodValue}
            onValueChange={onPeriodChange}
            disabled={disabled}
          >
            <SelectTrigger id={`${idPrefix}-period`}>
              <SelectValue placeholder={periodOptions[0]?.label ?? ""} />
            </SelectTrigger>
            <SelectContent>
              {periodOptions.map((option) => (
                <SelectItem key={option.value} value={option.value}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>
    </div>
  );
}
