import { useState } from "react";
import { Save, RotateCw } from "lucide-react";
import { toast, Toaster } from "sonner";
import { useAppStore, type Theme } from "@/stores/app-store";
import { api } from "@/lib/tauri-api";
import { cn } from "@/lib/utils";

export function SettingsPage() {
  const store = useAppStore();

  const [mihomoConfigDir, setMihomoConfigDir] = useState(store.mihomoConfigDir);
  const [apiAddress, setApiAddress] = useState(store.apiAddress);
  const [apiSecret, setApiSecret] = useState(store.apiSecret);
  const [httpPort, setHttpPort] = useState(store.httpPort);
  const [socksPort, setSocksPort] = useState(store.socksPort);
  const [autoStart, setAutoStart] = useState(store.autoStart);
  const [language, setLanguage] = useState(store.language);
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
        socksPort,
        autoStart,
        language,
      });
      store.setTheme(theme);

      if (apiAddress !== prevAddress || apiSecret !== prevSecret) {
        await api.system.updateMihomoClient(
          `http://${apiAddress}`,
          apiSecret,
        );
      }

      toast.success("设置已保存");
    } catch (err) {
      toast.error(`保存失败: ${err}`);
    } finally {
      setSaving(false);
    }
  };

  const inputClass =
    "rounded-md border border-border bg-background px-3 py-1.5 text-sm outline-none focus:ring-1 focus:ring-primary w-64";
  const selectClass = cn(inputClass, "appearance-auto");

  return (
    <div className="flex h-full flex-col">
      <Toaster position="top-center" richColors />
      <div className="flex-1 overflow-y-auto p-6">
        <div className="mx-auto max-w-2xl space-y-0 divide-y divide-border">
          <Row label="Mihomo 配置目录">
            <input
              type="text"
              className={inputClass}
              value={mihomoConfigDir}
              onChange={(e) => setMihomoConfigDir(e.target.value)}
            />
          </Row>

          <Row label="API 地址">
            <input
              type="text"
              className={inputClass}
              value={apiAddress}
              onChange={(e) => setApiAddress(e.target.value)}
              placeholder="127.0.0.1:9090"
            />
          </Row>

          <Row label="API 密钥">
            <input
              type="password"
              className={inputClass}
              value={apiSecret}
              onChange={(e) => setApiSecret(e.target.value)}
            />
          </Row>

          <Row label="HTTP 代理端口">
            <input
              type="number"
              className={inputClass}
              value={httpPort}
              onChange={(e) => setHttpPort(Number(e.target.value))}
            />
          </Row>

          <Row label="SOCKS 端口">
            <input
              type="number"
              className={inputClass}
              value={socksPort}
              onChange={(e) => setSocksPort(Number(e.target.value))}
            />
          </Row>

          <Row label="开机自启">
            <button
              type="button"
              onClick={() => setAutoStart(!autoStart)}
              className={cn(
                "relative h-6 w-11 rounded-full transition-colors",
                autoStart ? "bg-primary" : "bg-muted",
              )}
            >
              <span
                className={cn(
                  "absolute top-0.5 left-0.5 block h-5 w-5 rounded-full bg-white transition-transform",
                  autoStart && "translate-x-5",
                )}
              />
            </button>
          </Row>

          <Row label="语言">
            <select
              className={selectClass}
              value={language}
              onChange={(e) =>
                setLanguage(e.target.value as "zh-CN" | "en-US")
              }
            >
              <option value="zh-CN">中文</option>
              <option value="en-US">English</option>
            </select>
          </Row>

          <Row label="主题">
            <select
              className={selectClass}
              value={theme}
              onChange={(e) => setTheme(e.target.value as Theme)}
            >
              <option value="system">跟随系统</option>
              <option value="light">浅色</option>
              <option value="dark">深色</option>
            </select>
          </Row>
        </div>

        <div className="mx-auto mt-6 max-w-2xl">
          <button
            onClick={handleSave}
            disabled={saving}
            className="inline-flex items-center gap-1.5 rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-50"
          >
            {saving ? (
              <RotateCw size={14} className="animate-spin" />
            ) : (
              <Save size={14} />
            )}
            {saving ? "保存中..." : "保存"}
          </button>
        </div>
      </div>
    </div>
  );
}

function Row({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between py-4">
      <span className="text-sm font-medium">{label}</span>
      {children}
    </div>
  );
}
