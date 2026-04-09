import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import * as Switch from "@radix-ui/react-switch";
import { useState } from "react";
import { motion } from "framer-motion";
import { BellRing, RotateCw, Save, Settings2, Waypoints } from "lucide-react";
import { toast, Toaster } from "sonner";
import { AiSettingsPanel } from "@/features/ai/ai-settings";
import { normalizeErrorMessage } from "@/lib/error";
import { api, type NotificationSettings } from "@/lib/tauri-api";
import { cn } from "@/lib/utils";
import { useAppStore, type Theme } from "@/stores/app-store";
import { ActionButton } from "@/components/ui/action-button";
import { FieldShell } from "@/components/ui/field-shell";
import { PageHeader } from "@/components/ui/page-header";

const NOTIFICATION_SETTINGS_QUERY_KEY = ["notification-settings"] as const;
const NOTIFICATION_INTERVAL_OPTIONS = [
  { value: 60, label: "1 分钟" },
  { value: 300, label: "5 分钟" },
  { value: 600, label: "10 分钟" },
  { value: 1800, label: "30 分钟" },
] as const;

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

          <NotificationSettingsSection />

          <AiSettingsPanel />
        </div>
      </div>
    </motion.section>
  );
}

function NotificationSettingsSection() {
  const queryClient = useQueryClient();
  const notificationQuery = useQuery({
    queryKey: NOTIFICATION_SETTINGS_QUERY_KEY,
    queryFn: api.diagnosis.getNotificationSettings,
  });

  const updateMutation = useMutation<
    NotificationSettings,
    Error,
    NotificationSettings,
    { previousSettings?: NotificationSettings }
  >({
    mutationFn: async (settings) => {
      await api.diagnosis.updateNotificationSettings(settings);
      return settings;
    },
    onMutate: async (settings) => {
      await queryClient.cancelQueries({ queryKey: NOTIFICATION_SETTINGS_QUERY_KEY });
      const previousSettings = queryClient.getQueryData<NotificationSettings>(
        NOTIFICATION_SETTINGS_QUERY_KEY,
      );
      queryClient.setQueryData(NOTIFICATION_SETTINGS_QUERY_KEY, settings);
      return { previousSettings };
    },
    onError: (error, _settings, context) => {
      if (context?.previousSettings) {
        queryClient.setQueryData(NOTIFICATION_SETTINGS_QUERY_KEY, context.previousSettings);
      }
      toast.error(`更新通知设置失败: ${normalizeErrorMessage(error)}`);
    },
    onSuccess: (settings) => {
      queryClient.setQueryData(NOTIFICATION_SETTINGS_QUERY_KEY, settings);
    },
    onSettled: async () => {
      await queryClient.invalidateQueries({ queryKey: NOTIFICATION_SETTINGS_QUERY_KEY });
    },
  });

  const settings = notificationQuery.data;
  const controlsDisabled = updateMutation.isPending;

  const updateSettings = (nextSettings: NotificationSettings) => {
    updateMutation.mutate(nextSettings);
  };

  return (
    <section className="relative overflow-hidden rounded-[2rem] border border-border/70 bg-background/95 p-6 shadow-[0_28px_100px_-52px_rgba(15,23,42,0.55)]">
      <div className="pointer-events-none absolute -left-12 top-4 size-32 rounded-full bg-primary/10 blur-3xl" />

      <div className="relative">
        <div className="flex flex-col gap-4 border-b border-border/70 pb-5 xl:flex-row xl:items-end xl:justify-between">
          <div className="max-w-2xl">
            <div className="inline-flex items-center gap-2 rounded-full border border-primary/15 bg-primary/10 px-3 py-1 text-xs font-medium tracking-[0.18em] text-primary uppercase">
              <BellRing className="size-3.5" />
              Desktop Notification
            </div>
            <h2 className="mt-4 text-2xl font-semibold tracking-tight text-foreground">
              通知设置
            </h2>
            <p className="mt-2 text-sm leading-7 text-muted-foreground">
              管理后台异常扫描的桌面通知行为。扫描间隔会同时作为同类告警的冷却时间，避免短时间重复提醒。
            </p>
          </div>
          <div className="rounded-full border border-border/70 bg-background/75 px-3 py-2 text-xs font-medium tracking-[0.18em] text-muted-foreground uppercase">
            Alerts / Cooldown / Scan Loop
          </div>
        </div>

        {notificationQuery.isLoading && !settings ? (
          <div className="mt-5 rounded-[1.4rem] border border-border/70 bg-background/75 px-4 py-5 text-sm text-muted-foreground">
            正在读取通知设置...
          </div>
        ) : null}

        {notificationQuery.error && !settings ? (
          <div className="mt-5 rounded-[1.4rem] border border-destructive/25 bg-destructive/5 px-4 py-5 text-sm text-destructive">
            读取通知设置失败: {normalizeErrorMessage(notificationQuery.error)}
          </div>
        ) : null}

        {settings ? (
          <div className="mt-5 grid gap-4 xl:grid-cols-2">
            <FieldShell
              label="启用桌面通知"
              hint="后台异常扫描命中 Warning 或 Critical 告警时，向系统发送桌面通知。"
            >
              <NotificationToggle
                checked={settings.enabled}
                disabled={controlsDisabled}
                onCheckedChange={(enabled) => updateSettings({ ...settings, enabled })}
                stateLabel={settings.enabled ? "当前为启用" : "当前为关闭"}
              />
            </FieldShell>

            <FieldShell
              label="仅严重告警"
              hint="启用后仅推送 Critical 级别告警；关闭时 Warning 和 Critical 都会通知。"
            >
              <NotificationToggle
                checked={settings.criticalOnly}
                disabled={controlsDisabled || !settings.enabled}
                onCheckedChange={(criticalOnly) =>
                  updateSettings({ ...settings, criticalOnly })
                }
                stateLabel={settings.criticalOnly ? "仅推送 Critical" : "推送 Warning 与 Critical"}
              />
            </FieldShell>

            <div className="xl:col-span-2">
              <FieldShell
                label="扫描间隔"
                hint="后台扫描异常的频率，同时也是同类通知的最短冷却时间。最短 60 秒。"
              >
                <select
                  value={settings.scanIntervalSecs}
                  onChange={(event) =>
                    updateSettings({
                      ...settings,
                      scanIntervalSecs: Number(event.target.value),
                    })
                  }
                  className={cn(inputClassName(), "appearance-auto bg-background")}
                  disabled={controlsDisabled}
                >
                  {NOTIFICATION_INTERVAL_OPTIONS.map((option) => (
                    <option
                      key={option.value}
                      value={option.value}
                      className="bg-background text-foreground"
                    >
                      {option.label}
                    </option>
                  ))}
                </select>
              </FieldShell>
            </div>
          </div>
        ) : null}
      </div>
    </section>
  );
}

interface NotificationToggleProps {
  checked: boolean;
  disabled?: boolean;
  onCheckedChange: (checked: boolean) => void;
  stateLabel: string;
}

function NotificationToggle({
  checked,
  disabled = false,
  onCheckedChange,
  stateLabel,
}: NotificationToggleProps) {
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="text-sm font-medium text-foreground">{stateLabel}</div>
      <Switch.Root
        checked={checked}
        disabled={disabled}
        onCheckedChange={onCheckedChange}
        className={cn(
          "relative inline-flex h-7 w-12 items-center rounded-full border transition-colors",
          checked
            ? "border-primary/30 bg-primary/85"
            : "border-border/80 bg-muted/35",
          disabled && "cursor-not-allowed opacity-60",
        )}
      >
        <Switch.Thumb
          className={cn(
            "block size-5 rounded-full bg-white shadow transition-transform",
            checked ? "translate-x-[1.45rem]" : "translate-x-1",
          )}
        />
      </Switch.Root>
    </div>
  );
}

function inputClassName() {
  return cn(
    "h-12 w-full rounded-[1.1rem] border border-border/70 bg-background/80 px-4 text-sm text-foreground outline-none transition-colors",
    "placeholder:text-muted-foreground/70 hover:border-primary/20 focus-visible:border-primary/30",
  );
}
