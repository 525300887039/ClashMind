import { QueryClientProvider } from "@tanstack/react-query";
import { queryClient } from "@/lib/query-client";
import { AppLayout } from "@/components/layout/app-layout";
import { useAppStore, type Page } from "@/stores/app-store";
import { ProxyPage } from "@/features/proxy/proxy-page";
import { ConnectionsPage } from "@/features/connections/connections-page";
import { RulesPage } from "@/features/rules/rules-page";
import { LogsPage } from "@/features/logs/logs-page";
import { ConfigPage } from "@/features/config/config-page";
import { SettingsPage } from "@/features/settings/settings-page";
import { StatsPage } from "@/features/stats/stats-page";
import { useAppInit } from "@/hooks/use-app-init";

const PAGE_MAP: Record<Page, React.ReactNode> = {
  proxies: <ProxyPage />,
  connections: <ConnectionsPage />,
  stats: <StatsPage />,
  rules: <RulesPage />,
  logs: <LogsPage />,
  config: <ConfigPage />,
  settings: <SettingsPage />,
};

function AppContent() {
  const currentPage = useAppStore((s) => s.currentPage);
  const { status, error, setupAndStart, retry } = useAppInit();

  if (status === "checking" || status === "starting") {
    return (
      <div className="flex h-screen items-center justify-center">
        <p className="text-sm text-muted-foreground">
          {status === "checking" ? "检查配置中..." : "启动 mihomo..."}
        </p>
      </div>
    );
  }

  if (status === "needs-setup") {
    return (
      <div className="flex h-screen flex-col items-center justify-center gap-4">
        <p className="text-sm text-muted-foreground">
          未检测到 mihomo 配置文件，是否创建默认配置并启动？
        </p>
        <button
          onClick={setupAndStart}
          className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground"
        >
          创建默认配置并启动
        </button>
      </div>
    );
  }

  if (status === "error") {
    return (
      <div className="flex h-screen flex-col items-center justify-center gap-4">
        <p className="text-sm text-destructive">启动失败: {error}</p>
        <button
          onClick={retry}
          className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground"
        >
          重试
        </button>
      </div>
    );
  }

  return <AppLayout>{PAGE_MAP[currentPage]}</AppLayout>;
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AppContent />
    </QueryClientProvider>
  );
}
