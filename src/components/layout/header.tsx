import { Sun, Moon, Monitor, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { useTheme } from "@/hooks/use-theme";
import { useAppStore } from "@/stores/app-store";
import { cn } from "@/lib/utils";

export function Header() {
  const { theme, setTheme, resolvedTheme } = useTheme();
  const collapsed = useAppStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useAppStore((s) => s.toggleSidebar);

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
        <span>↑ 0 B/s</span>
        <span>↓ 0 B/s</span>
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
