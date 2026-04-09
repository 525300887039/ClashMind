import {
  AlertCircle,
  AlertTriangle,
  Info,
  type LucideIcon,
} from "lucide-react";
import { cn, formatZhDateTime } from "@/lib/utils";
import type { AlertSeverity, AnomalyAlert } from "@/lib/tauri-api";

interface SeverityVisualConfig {
  icon: LucideIcon;
  label: string;
  borderClassName: string;
  backgroundClassName: string;
  iconWrapClassName: string;
  iconClassName: string;
  badgeClassName: string;
  accentClassName: string;
}

const SEVERITY_CONFIG: Record<AlertSeverity, SeverityVisualConfig> = {
  critical: {
    icon: AlertCircle,
    label: "严重",
    borderClassName: "border-red-500/30",
    backgroundClassName: "bg-red-500/8",
    iconWrapClassName: "bg-red-500/14",
    iconClassName: "text-red-400",
    badgeClassName: "bg-red-500/14 text-red-300 ring-1 ring-inset ring-red-500/25",
    accentClassName: "bg-red-400",
  },
  warning: {
    icon: AlertTriangle,
    label: "警告",
    borderClassName: "border-amber-500/30",
    backgroundClassName: "bg-amber-500/8",
    iconWrapClassName: "bg-amber-500/14",
    iconClassName: "text-amber-400",
    badgeClassName:
      "bg-amber-500/14 text-amber-300 ring-1 ring-inset ring-amber-500/25",
    accentClassName: "bg-amber-400",
  },
  info: {
    icon: Info,
    label: "提示",
    borderClassName: "border-blue-500/30",
    backgroundClassName: "bg-blue-500/8",
    iconWrapClassName: "bg-blue-500/14",
    iconClassName: "text-blue-400",
    badgeClassName: "bg-blue-500/14 text-blue-300 ring-1 ring-inset ring-blue-500/25",
    accentClassName: "bg-blue-400",
  },
};

interface DiagnosisAlertCardProps {
  alert: AnomalyAlert;
}

export function DiagnosisAlertCard({ alert }: DiagnosisAlertCardProps) {
  const config = SEVERITY_CONFIG[alert.severity];
  const Icon = config.icon;

  return (
    <article
      className={cn(
        "relative overflow-hidden rounded-[1.35rem] border p-4 shadow-[0_18px_42px_-30px_rgba(15,23,42,0.55)]",
        config.borderClassName,
        config.backgroundClassName,
      )}
    >
      <div
        className={cn(
          "pointer-events-none absolute bottom-4 left-3 top-4 w-0.5 rounded-full opacity-80",
          config.accentClassName,
        )}
      />

      <div className="relative flex items-start gap-3 pl-3">
        <div
          className={cn(
            "inline-flex size-10 shrink-0 items-center justify-center rounded-2xl",
            config.iconWrapClassName,
          )}
        >
          <Icon className={cn("size-5", config.iconClassName)} />
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <span
              className={cn(
                "inline-flex items-center rounded-full px-2.5 py-1 text-[11px] font-medium tracking-[0.16em] uppercase",
                config.badgeClassName,
              )}
            >
              {config.label}
            </span>
            <span className="text-xs text-muted-foreground">
              {formatZhDateTime(alert.detectedAt, alert.detectedAt)}
            </span>
          </div>

          <h3 className="mt-2 text-sm font-semibold tracking-tight text-foreground">
            {alert.title}
          </h3>
          <p className="mt-1 text-sm leading-6 text-muted-foreground">
            {alert.description}
          </p>
        </div>
      </div>
    </article>
  );
}
