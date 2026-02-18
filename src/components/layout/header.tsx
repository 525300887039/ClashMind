import { Sun, Moon, Monitor, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { useTheme } from "@/hooks/use-theme";
import { useAppStore } from "@/stores/app-store";
import { useTraffic } from "@/features/traffic/hooks/use-traffic";
import { cn } from "@/lib/utils";

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
    <header className="flex h-12 items-center justify-between border-b border-border bg-muted/50 px-3">
      <div className="flex items-center gap-2">
        <button
          onClick={toggleSidebar}
          className="rounded-md p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground"
        >
          {collapsed ? <PanelLeftOpen size={18} /> : <PanelLeftClose size={18} />}
        </button>
        <span className="text-sm font-semibold text-foreground">ClashMind</span>
      </div>

      <div className="flex items-center gap-4 text-xs text-muted-foreground">
        <span>↑ {formatSpeed(up)}</span>
        <span>↓ {formatSpeed(down)}</span>
      </div>

      <button
        onClick={cycleTheme}
        className={cn(
          "rounded-md p-1.5 text-muted-foreground hover:bg-accent hover:text-foreground",
        )}
      >
        <ThemeIcon size={18} />
      </button>
    </header>
  );
}
