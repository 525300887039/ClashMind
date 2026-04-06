import { useEffect, useState } from "react";
import { useAppStore } from "@/stores/app-store";
import { api } from "@/lib/tauri-api";
import { getAiSettingsSnapshot } from "@/features/ai/hooks/use-ai-settings";

type InitStatus = "checking" | "needs-setup" | "starting" | "ready" | "error";

export function useAppInit() {
  const [status, setStatus] = useState<InitStatus>("checking");
  const [error, setError] = useState<string | null>(null);
  const configDir = useAppStore((s) => s.mihomoConfigDir);

  const startCollectorQuietly = async () => {
    try {
      await api.collector.start();
      console.log("[ClashMind] Collector 启动成功");
    } catch (err) {
      console.warn("[ClashMind] Collector 启动失败:", err);
    }
  };

  const startAiQuietly = async () => {
    try {
      const settings = await getAiSettingsSnapshot();
      if (!settings.autoStart) {
        return;
      }

      await api.ai.start();
      console.log("[ClashMind] AI sidecar 启动成功");
    } catch (err) {
      const message = String(err);
      if (message.includes("已在运行")) {
        console.log("[ClashMind] AI sidecar 已在运行");
        return;
      }

      console.warn("[ClashMind] AI sidecar 自动启动失败:", err);
    }
  };

  const initialize = async () => {
    try {
      setStatus("checking");
      const hasConfig = await api.mihomo.checkConfig(configDir);

      if (!hasConfig) {
        setStatus("needs-setup");
        return;
      }

      setStatus("starting");
      await api.mihomo.start(configDir);
      setStatus("ready");
      startCollectorQuietly();
      void startAiQuietly();
    } catch (err) {
      // If sidecar is already running, that's fine
      const msg = String(err);
      if (msg.includes("已在运行")) {
        setStatus("ready");
        startCollectorQuietly();
        void startAiQuietly();
      } else {
        setError(msg);
        setStatus("error");
      }
    }
  };

  const setupAndStart = async () => {
    try {
      setStatus("starting");
      await api.mihomo.ensureDefaultConfig(configDir);
      await api.mihomo.start(configDir);
      setStatus("ready");
      startCollectorQuietly();
      void startAiQuietly();
    } catch (err) {
      const msg = String(err);
      if (msg.includes("已在运行")) {
        setStatus("ready");
        startCollectorQuietly();
        void startAiQuietly();
      } else {
        setError(msg);
        setStatus("error");
      }
    }
  };

  useEffect(() => {
    initialize();
  }, []);

  return { status, error, setupAndStart, retry: initialize };
}
