import {
  Sparkles,
  Globe,
  Cable,
  ChartNoAxesCombined,
  Shield,
  ScrollText,
  FileCog,
  Settings,
  type LucideIcon,
} from "lucide-react";
import { motion } from "framer-motion";
import { useAppStore, type Page } from "@/stores/app-store";
import { cn } from "@/lib/utils";

const NAV_ITEMS: { page: Page; label: string; icon: LucideIcon }[] = [
  { page: "proxies", label: "代理", icon: Globe },
  { page: "connections", label: "连接", icon: Cable },
  { page: "ai", label: "AI 助手", icon: Sparkles },
  { page: "stats", label: "统计", icon: ChartNoAxesCombined },
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
        "flex flex-col gap-1 border-r border-border/60 bg-background/60 backdrop-blur-xl p-2 transition-all duration-200",
        collapsed ? "w-14" : "w-48",
      )}
    >
      {NAV_ITEMS.map(({ page, label, icon: Icon }) => (
        <button
          key={page}
          onClick={() => setCurrentPage(page)}
          className={cn(
            "relative flex items-center gap-3 rounded-xl px-3 py-2 text-sm transition-colors",
            currentPage === page
              ? "text-primary-foreground font-medium"
              : "text-muted-foreground hover:bg-accent/50 hover:text-foreground",
          )}
        >
          {currentPage === page && (
            <motion.div
              layoutId="sidebar-active"
              className="absolute inset-0 rounded-xl bg-primary"
              transition={{ type: "spring", stiffness: 350, damping: 30 }}
            />
          )}
          <span className="relative z-10 flex items-center gap-3">
            <Icon size={18} className="shrink-0" />
            {!collapsed && <span>{label}</span>}
          </span>
        </button>
      ))}
    </nav>
  );
}
