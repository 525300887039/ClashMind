import { cn } from "@/lib/utils";
import { RANGE_OPTIONS, type StatsRange } from "../constants";

interface RangeSelectorProps {
  selectedDays: StatsRange;
  onSelect: (days: StatsRange) => void;
  isPending?: boolean;
}

export function RangeSelector({
  selectedDays,
  onSelect,
  isPending = false,
}: RangeSelectorProps) {
  return (
    <div
      aria-busy={isPending}
      className="inline-flex rounded-full border border-border bg-muted/50 p-1"
    >
      {RANGE_OPTIONS.map((option) => {
        const isActive = option.days === selectedDays;

        return (
          <button
            key={option.days}
            type="button"
            onClick={() => onSelect(option.days)}
            className={cn(
              "rounded-full px-3 py-2 text-sm transition-all",
              isActive
                ? "bg-primary text-primary-foreground shadow-[0_12px_32px_-18px_var(--color-primary)]"
                : "text-muted-foreground hover:bg-background hover:text-foreground",
            )}
          >
            <span className="font-medium">{option.label}</span>
            <span className="ml-1 hidden text-xs opacity-80 sm:inline">
              {option.caption}
            </span>
          </button>
        );
      })}
    </div>
  );
}
