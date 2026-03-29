import { useEffect, useState } from "react";
import { useAppStore } from "@/stores/app-store";
import { api } from "@/lib/tauri-api";

type InitStatus = "checking" | "needs-setup" | "starting" | "ready" | "error";

export function useAppInit() {
  const [status, setStatus] = useState<InitStatus>("checking");
  const [error, setError] = useState<string | null>(null);
  const configDir = useAppStore((s) => s.mihomoConfigDir);

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
    } catch (err) {
      // If sidecar is already running, that's fine
      const msg = String(err);
      if (msg.includes("已在运行")) {
        setStatus("ready");
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
    } catch (err) {
      const msg = String(err);
      if (msg.includes("已在运行")) {
        setStatus("ready");
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
