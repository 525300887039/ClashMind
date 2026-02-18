import { QueryClientProvider } from "@tanstack/react-query";
import { queryClient } from "@/lib/query-client";
import { AppLayout } from "@/components/layout/app-layout";
import { useAppStore, type Page } from "@/stores/app-store";

function PlaceholderPage({ name }: { name: string }) {
  return <div className="text-muted-foreground">{name}</div>;
}

const PAGE_MAP: Record<Page, React.ReactNode> = {
  proxies: <PlaceholderPage name="代理" />,
  connections: <PlaceholderPage name="连接" />,
  rules: <PlaceholderPage name="规则" />,
  logs: <PlaceholderPage name="日志" />,
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
