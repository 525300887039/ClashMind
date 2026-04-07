import { useState } from "react";
import { motion } from "framer-motion";
import { RotateCw, Save, Settings2, Waypoints } from "lucide-react";
import { toast, Toaster } from "sonner";
import { AiSettingsPanel } from "@/features/ai/ai-settings";
import { api } from "@/lib/tauri-api";
import { cn } from "@/lib/utils";
import { useAppStore, type Theme } from "@/stores/app-store";
import { ActionButton } from "@/components/ui/action-button";
import { FieldShell } from "@/components/ui/field-shell";
import { PageHeader } from "@/components/ui/page-header";

export function SettingsPage() {
  const store = useAppStore();
  const [mihomoConfigDir, setMihomoConfigDir] = useState(store.mihomoConfigDir);
  const [apiAddress, setApiAddress] = useState(store.apiAddress);
  const [apiSecret, setApiSecret] = useState(store.apiSecret);
  const [httpPort, setHttpPort] = useState(store.httpPort);
  const [theme, setTheme] = useState<Theme>(store.theme);
  const [saving, setSaving] = useState(false);

  const handleSave = async () => {
    setSaving(true);
    try {
      const prevAddress = store.apiAddress;
      const prevSecret = store.apiSecret;

      store.updateSettings({
        mihomoConfigDir,
        apiAddress,
        apiSecret,
        httpPort,
      });
      store.setTheme(theme);

      if (apiAddress !== prevAddress || apiSecret !== prevSecret) {
        await api.system.updateMihomoClient(`http://${apiAddress}`, apiSecret);
      }

      toast.success("系统设置已保存");
    } catch (error) {
      toast.error(`保存系统设置失败: ${String(error)}`);
    } finally {
      setSaving(false);
    }
  };

  return (
    <motion.section
      initial={{ opacity: 0, y: 14 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.24, ease: "easeOut" }}
      className="flex h-[calc(100vh-5rem)] flex-col gap-4"
    >
      <Toaster position="top-center" richColors />

      <PageHeader
        eyebrow="Control Surface"
        eyebrowIcon={Settings2}
        title="设置"
        description="上半区维护 Mihomo 本地控制面的连接参数，下半区集中管理 AI Provider、模型与 sidecar 服务状态。AI 设置会独立持久化到应用数据目录。"
        actions={
          <ActionButton tone="primary" onClick={handleSave} disabled={saving}>
            {saving ? <RotateCw className="size-4 animate-spin" /> : <Save className="size-4" />}
            {saving ? "保存中" : "保存系统设置"}
          </ActionButton>
        }
      />

      <div className="min-h-0 flex-1 overflow-y-auto pr-1">
        <div className="space-y-4">
          <section className="relative overflow-hidden rounded-[2rem] border border-border/70 bg-background/95 p-6 shadow-[0_28px_100px_-52px_rgba(15,23,42,0.55)]">
            <div className="pointer-events-none absolute -right-8 top-0 size-28 rounded-full bg-primary/10 blur-3xl" />

            <div className="relative">
              <div className="flex flex-col gap-4 border-b border-border/70 pb-5 xl:flex-row xl:items-end xl:justify-between">
                <div className="max-w-2xl">
                  <div className="inline-flex items-center gap-2 rounded-full border border-primary/15 bg-primary/10 px-3 py-1 text-xs font-medium tracking-[0.18em] text-primary uppercase">
                    <Waypoints className="size-3.5" />
                    Mihomo Control Plane
                  </div>
                  <h2 className="mt-4 text-2xl font-semibold tracking-tight text-foreground">
                    系统设置
                  </h2>
                  <p className="mt-2 text-sm leading-7 text-muted-foreground">
                    这些参数决定 ClashMind 如何定位配置目录、连接 Mihomo 控制面并应用代理端口。
                  </p>
                </div>
                <div className="rounded-full border border-border/70 bg-background/75 px-3 py-2 text-xs font-medium tracking-[0.18em] text-muted-foreground uppercase">
                  Theme / API / Paths
                </div>
              </div>

              <div className="mt-5 grid gap-4 xl:grid-cols-2">
                <FieldShell label="Mihomo 配置目录">
                  <input
                    type="text"
                    value={mihomoConfigDir}
                    onChange={(event) => setMihomoConfigDir(event.target.value)}
                    className={inputClassName()}
                  />
                </FieldShell>

                <FieldShell label="控制面地址">
                  <input
                    type="text"
                    value={apiAddress}
                    onChange={(event) => setApiAddress(event.target.value)}
                    className={inputClassName()}
                    placeholder="127.0.0.1:9090"
                  />
                </FieldShell>

                <FieldShell label="控制面密钥">
                  <input
                    type="password"
                    value={apiSecret}
                    onChange={(event) => setApiSecret(event.target.value)}
                    className={inputClassName()}
                    placeholder="可留空"
                  />
                </FieldShell>

                <FieldShell label="HTTP 代理端口">
                  <input
                    type="number"
                    value={httpPort}
                    onChange={(event) => setHttpPort(Number(event.target.value))}
                    className={inputClassName()}
                    min={1}
                    step={1}
                  />
                </FieldShell>

                <FieldShell label="主题">
                  <select
                    value={theme}
                    onChange={(event) => setTheme(event.target.value as Theme)}
                    className={cn(inputClassName(), "appearance-auto bg-background")}
                  >
                    <option value="system" className="bg-background text-foreground">跟随系统</option>
                    <option value="light" className="bg-background text-foreground">浅色</option>
                    <option value="dark" className="bg-background text-foreground">深色</option>
                  </select>
                </FieldShell>
              </div>
            </div>
          </section>

          <AiSettingsPanel />
        </div>
      </div>
    </motion.section>
  );
}

function inputClassName() {
  return cn(
    "h-12 w-full rounded-[1.1rem] border border-border/70 bg-background/80 px-4 text-sm text-foreground outline-none transition-colors",
    "placeholder:text-muted-foreground/70 hover:border-primary/20 focus-visible:border-primary/30",
  );
}
