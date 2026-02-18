import { QueryClientProvider } from "@tanstack/react-query";
import { queryClient } from "@/lib/query-client";
import { AppLayout } from "@/components/layout/app-layout";
import { useAppStore, type Page } from "@/stores/app-store";
import { ProxyPage } from "@/features/proxy/proxy-page";
import { ConnectionsPage } from "@/features/connections/connections-page";
import { RulesPage } from "@/features/rules/rules-page";
import { LogsPage } from "@/features/logs/logs-page";

function PlaceholderPage({ name }: { name: string }) {
  return <div className="text-muted-foreground">{name}</div>;
}

const PAGE_MAP: Record<Page, React.ReactNode> = {
  proxies: <ProxyPage />,
  connections: <ConnectionsPage />,
  rules: <RulesPage />,
  logs: <LogsPage />,
  config: <PlaceholderPage name="配置" />,
  settings: <PlaceholderPage name="设置" />,
};

function AppContent() {
  const currentPage = useAppStore((s) => s.currentPage);
  return <AppLayout>{PAGE_MAP[currentPage]}</AppLayout>;
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AppContent />
    </QueryClientProvider>
  );
}
