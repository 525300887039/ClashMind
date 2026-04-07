import { Sun, Moon, Monitor, PanelLeftClose, PanelLeftOpen, ArrowUp, ArrowDown, Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useTheme } from "@/hooks/use-theme";
import { useAppStore } from "@/stores/app-store";
import { useTraffic } from "@/features/traffic/hooks/use-traffic";
import { cn } from "@/lib/utils";

const appWindow = getCurrentWindow();

function formatSpeed(bytes: number): string {
  if (bytes < 1024) return `${bytes} B/s`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB/s`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB/s`;
}

export function Header() {
  const { theme, setTheme, resolvedTheme } = useTheme();
  const collapsed = useAppStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useAppStore((s) => s.toggleSidebar);
  const { up, down } = useTraffic();

  const cycleTheme = () => {
    const next = { light: "dark", dark: "system", system: "light" } as const;
    setTheme(next[theme]);
  };

  const ThemeIcon = theme === "system" ? Monitor : resolvedTheme === "dark" ? Moon : Sun;

  return (
    <header data-tauri-drag-region className="flex h-12 select-none items-center justify-between border-b border-border/60 bg-background/80 backdrop-blur-xl px-3">
      <div className="flex items-center gap-2">
        <button
          onClick={toggleSidebar}
          className="rounded-xl p-1.5 text-muted-foreground hover:bg-accent/50 hover:text-foreground transition-colors"
        >
          {collapsed ? <PanelLeftOpen size={18} /> : <PanelLeftClose size={18} />}
        </button>
        <span className="text-sm font-semibold text-foreground">ClashMind</span>
      </div>

      <div className="flex items-center gap-4 text-xs text-muted-foreground">
        <span className="flex items-center gap-1 text-warning">
          <ArrowUp size={14} />
          {formatSpeed(up)}
        </span>
        <span className="flex items-center gap-1 text-success">
          <ArrowDown size={14} />
          {formatSpeed(down)}
        </span>
      </div>

      <div className="flex items-center gap-1">
        <button
          onClick={cycleTheme}
          className={cn(
            "rounded-xl p-1.5 text-muted-foreground hover:bg-accent/50 hover:text-foreground transition-colors",
          )}
        >
          <ThemeIcon size={18} />
        </button>

        <div className="mx-1 h-4 w-px bg-border/60" />

        <button
          onClick={() => appWindow.minimize()}
          className="rounded-lg p-1.5 text-muted-foreground hover:bg-accent/50 hover:text-foreground transition-colors"
        >
          <Minus size={15} />
        </button>
        <button
          onClick={() => appWindow.toggleMaximize()}
          className="rounded-lg p-1.5 text-muted-foreground hover:bg-accent/50 hover:text-foreground transition-colors"
        >
          <Square size={13} />
        </button>
        <button
          onClick={() => appWindow.close()}
          className="rounded-lg p-1.5 text-muted-foreground hover:bg-red-500/80 hover:text-white transition-colors"
        >
          <X size={15} />
        </button>
      </div>
    </header>
  );
}
