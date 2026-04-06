import { useState } from "react";
import { Save, RotateCw } from "lucide-react";
import { toast, Toaster } from "sonner";
import { useAppStore, type Theme } from "@/stores/app-store";
import type { AiProviderKind } from "@/lib/tauri-api";
import { api } from "@/lib/tauri-api";
import { cn } from "@/lib/utils";

export function SettingsPage() {
  const store = useAppStore();

  const [mihomoConfigDir, setMihomoConfigDir] = useState(store.mihomoConfigDir);
  const [apiAddress, setApiAddress] = useState(store.apiAddress);
  const [apiSecret, setApiSecret] = useState(store.apiSecret);
  const [httpPort, setHttpPort] = useState(store.httpPort);
  const [aiProvider, setAiProvider] = useState<AiProviderKind>(store.aiProvider);
  const [aiModel, setAiModel] = useState(store.aiModel);
  const [aiApiKey, setAiApiKey] = useState(store.aiApiKey);
  const [aiBaseUrl, setAiBaseUrl] = useState(store.aiBaseUrl);
  const [aiTemperature, setAiTemperature] = useState(store.aiTemperature);
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
        aiProvider,
        aiModel,
        aiApiKey,
        aiBaseUrl,
        aiTemperature,
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

          <SectionLabel label="AI 助手" />

          <Row label="AI Provider">
            <select
              className={selectClass}
              value={aiProvider}
              onChange={(e) => setAiProvider(e.target.value as AiProviderKind)}
            >
              <option value="openai">OpenAI</option>
              <option value="claude">Claude</option>
              <option value="deepseek">DeepSeek</option>
              <option value="ollama">Ollama</option>
            </select>
          </Row>

          <Row label="AI 模型">
            <input
              type="text"
              className={inputClass}
              value={aiModel}
              onChange={(e) => setAiModel(e.target.value)}
              placeholder={aiProvider === "ollama" ? "qwen3:latest" : "gpt-4o-mini"}
            />
          </Row>

          {aiProvider !== "ollama" ? (
            <Row label="AI API Key">
              <input
                type="password"
                className={inputClass}
                value={aiApiKey}
                onChange={(e) => setAiApiKey(e.target.value)}
                placeholder="输入 API Key"
              />
            </Row>
          ) : null}

          <Row label="AI Base URL">
            <input
              type="text"
              className={inputClass}
              value={aiBaseUrl}
              onChange={(e) => setAiBaseUrl(e.target.value)}
              placeholder={aiProvider === "ollama" ? "http://localhost:11434/api" : "留空使用默认"}
            />
          </Row>

          <Row label="AI 温度">
            <input
              type="number"
              min={0}
              max={1}
              step={0.1}
              className={inputClass}
              value={aiTemperature}
              onChange={(e) => setAiTemperature(Number(e.target.value))}
            />
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

function SectionLabel({ label }: { label: string }) {
  return (
    <div className="pt-6">
      <span className="text-xs font-semibold tracking-[0.18em] text-muted-foreground uppercase">
        {label}
      </span>
    </div>
  );
}
