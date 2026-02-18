import {
  Globe,
  Cable,
  Shield,
  ScrollText,
  FileCog,
  Settings,
  type LucideIcon,
} from "lucide-react";
import { useAppStore, type Page } from "@/stores/app-store";
import { cn } from "@/lib/utils";

const NAV_ITEMS: { page: Page; label: string; icon: LucideIcon }[] = [
  { page: "proxies", label: "代理", icon: Globe },
  { page: "connections", label: "连接", icon: Cable },
  { page: "rules", label: "规则", icon: Shield },
  { page: "logs", label: "日志", icon: ScrollText },
  { page: "config", label: "配置", icon: FileCog },
  { page: "settings", label: "设置", icon: Settings },
];

export function Sidebar() {
  const currentPage = useAppStore((s) => s.currentPage);
  const collapsed = useAppStore((s) => s.sidebarCollapsed);
  const setCurrentPage = useAppStore((s) => s.setCurrentPage);

  return (
    <nav
      className={cn(
        "flex flex-col gap-1 border-r border-border bg-muted/50 p-2 transition-all",
        collapsed ? "w-14" : "w-48",
      )}
    >
      {NAV_ITEMS.map(({ page, label, icon: Icon }) => (
        <button
          key={page}
          onClick={() => setCurrentPage(page)}
          className={cn(
            "flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
            currentPage === page
              ? "bg-accent text-foreground font-medium"
              : "text-muted-foreground hover:bg-accent/50 hover:text-foreground",
          )}
        >
          <Icon size={18} className="shrink-0" />
          {!collapsed && <span>{label}</span>}
        </button>
      ))}
    </nav>
  );
}
