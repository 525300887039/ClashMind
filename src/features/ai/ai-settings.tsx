import { useEffect, useMemo, useState } from "react";
import * as Select from "@radix-ui/react-select";
import * as Slider from "@radix-ui/react-slider";
import * as Switch from "@radix-ui/react-switch";
import { motion } from "framer-motion";
import {
  Check,
  ChevronDown,
  Eye,
  EyeOff,
  Gauge,
  KeyRound,
  LoaderCircle,
  Orbit,
  Play,
  PlugZap,
  RefreshCw,
  Save,
  ServerCog,
  Square,
  TestTubeDiagonal,
} from "lucide-react";
import { toast } from "sonner";
import type { AiModelCatalogSource, AiProviderKind, AiSettings as AiSettingsValue } from "@/lib/tauri-api";
import { cn } from "@/lib/utils";
import {
  DEFAULT_AI_SETTINGS,
  getModelCatalogBlockingReason,
  getDefaultModel,
  getProviderModels,
  isAiConfigured,
  normalizeAiSettings,
  providerRequiresApiKey,
  providerRequiresBaseUrl,
  useAiModelCatalogQuery,
  useAiConnectionTestMutation,
  useAiServiceControls,
  useAiSettingsQuery,
  useSaveAiSettingsMutation,
} from "./hooks/use-ai-settings";

const PROVIDER_OPTIONS: Array<{ value: AiProviderKind; label: string; eyebrow: string }> = [
  { value: "openai", label: "OpenAI", eyebrow: "GPT 系列" },
  { value: "openai_compatible", label: "OpenAI Compatible", eyebrow: "DeepSeek / Ollama / 第三方兼容" },
  { value: "claude", label: "Claude", eyebrow: "Anthropic" },
  { value: "gemini", label: "Gemini", eyebrow: "Google" },
];

const CUSTOM_MODEL_VALUE = "__custom__";

function describeProvider(provider: AiProviderKind) {
  switch (provider) {
    case "openai":
      return "云端高可用，默认适合通用对话与配置生成。";
    case "openai_compatible":
      return "通用 OpenAI 格式兼容渠道，适用于 DeepSeek、Ollama 与其他第三方网关。";
    case "claude":
      return "偏长文本与推理分析，适合诊断和解释。";
    case "gemini":
      return "Google Gemini 渠道，适合通用对话、多模态与长上下文场景。";
  }
}

function baseUrlPlaceholder(provider: AiProviderKind) {
  switch (provider) {
    case "openai_compatible":
      return "例如 https://api.deepseek.com/v1 或 http://127.0.0.1:11434/v1";
    case "gemini":
      return "留空使用 Gemini 官方默认地址";
    default:
      return "留空使用官方默认地址";
  }
}

function connectionTone(
  success: boolean | undefined,
  hasMessage: boolean,
) {
  if (!hasMessage) {
    return "border-border/70 bg-background/70 text-muted-foreground";
  }

  return success
    ? "border-emerald-500/20 bg-emerald-500/10 text-emerald-200"
    : "border-rose-500/20 bg-rose-500/10 text-rose-200";
}

function modelCatalogTone(hasBlockingReason: boolean, source: AiModelCatalogSource | undefined) {
  if (hasBlockingReason || source === "empty") {
    return "border-amber-500/20 bg-amber-500/10 text-amber-200";
  }

  if (source === "fallback") {
    return "border-sky-500/20 bg-sky-500/10 text-sky-200";
  }

  if (source === "remote") {
    return "border-emerald-500/20 bg-emerald-500/10 text-emerald-200";
  }

  return "border-border/70 bg-background/70 text-muted-foreground";
}

function modelIdHint(provider: AiProviderKind) {
  switch (provider) {
    case "openai_compatible":
      return "支持输入 DeepSeek、Ollama 或任意 OpenAI 兼容模型名。";
    case "gemini":
      return "如需切换新版 Gemini 模型，也可直接在这里手动覆盖。";
    default:
      return "如果官方模型名更新，可直接在这里手动覆盖。";
  }
}

function triggerClassName() {
  return cn(
    "flex h-12 w-full items-center justify-between rounded-[1.1rem] border border-border/70 bg-background/80 px-4 text-left text-sm text-foreground shadow-[inset_0_1px_0_rgba(255,255,255,0.04)] outline-none transition-colors",
    "hover:border-primary/20 focus-visible:border-primary/30",
  );
}

function fieldInputClassName() {
  return cn(
    "h-12 w-full rounded-[1.1rem] border border-border/70 bg-background/80 px-4 text-sm text-foreground outline-none transition-colors",
    "placeholder:text-muted-foreground/70 hover:border-primary/20 focus-visible:border-primary/30",
  );
}

function SelectField({
  value,
  onValueChange,
  children,
}: {
  value: string;
  onValueChange: (value: string) => void;
  children: React.ReactNode;
}) {
  return (
    <Select.Root value={value} onValueChange={onValueChange}>
      <Select.Trigger className={triggerClassName()}>
        <Select.Value />
        <Select.Icon className="text-muted-foreground">
          <ChevronDown className="size-4" />
        </Select.Icon>
      </Select.Trigger>
      <Select.Portal>
        <Select.Content
          position="popper"
          sideOffset={8}
          className="z-50 min-w-[var(--radix-select-trigger-width)] overflow-hidden rounded-[1.2rem] border border-border/70 bg-background/95 p-1.5 shadow-[0_22px_80px_-36px_rgba(15,23,42,0.72)] backdrop-blur"
        >
          <Select.Viewport>{children}</Select.Viewport>
        </Select.Content>
      </Select.Portal>
    </Select.Root>
  );
}

function SelectItem({
  value,
  label,
  description,
}: {
  value: string;
  label: string;
  description?: string;
}) {
  return (
    <Select.Item
      value={value}
      className="group relative flex cursor-pointer select-none items-center rounded-[0.95rem] px-3 py-2.5 text-sm text-foreground outline-none transition-colors data-[highlighted]:bg-primary/10 data-[highlighted]:text-foreground"
    >
      <Select.ItemText>
        <div className="flex items-center gap-3">
          <div className="inline-flex size-6 items-center justify-center rounded-full border border-primary/15 bg-primary/10 text-primary">
            <Check className="size-3.5 opacity-0 transition-opacity group-data-[state=checked]:opacity-100" />
          </div>
          <div>
            <div className="font-medium">{label}</div>
            {description ? (
              <div className="text-xs text-muted-foreground">{description}</div>
            ) : null}
          </div>
        </div>
      </Select.ItemText>
      <Select.ItemIndicator className="absolute left-4 inline-flex items-center justify-center text-primary">
        <Check className="size-3.5" />
      </Select.ItemIndicator>
    </Select.Item>
  );
}

function ActionButton({
  tone = "primary",
  disabled,
  onClick,
  children,
}: {
  tone?: "primary" | "secondary" | "ghost";
  disabled?: boolean;
  onClick?: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "inline-flex items-center justify-center gap-2 rounded-full border px-4 py-2.5 text-sm font-medium transition-all",
        tone === "primary" &&
          "border-primary/20 bg-primary text-primary-foreground shadow-[0_18px_42px_-24px_var(--color-primary)] hover:translate-y-[-1px] hover:bg-primary/92",
        tone === "secondary" &&
          "border-border/70 bg-background/80 text-foreground hover:border-primary/20 hover:bg-primary/5",
        tone === "ghost" &&
          "border-border/70 bg-transparent text-muted-foreground hover:border-primary/20 hover:text-foreground",
        "disabled:cursor-not-allowed disabled:opacity-55 disabled:hover:translate-y-0",
      )}
    >
      {children}
    </button>
  );
}

function FieldShell({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <label className="rounded-[1.45rem] border border-border/70 bg-background/68 p-4">
      <div className="text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase">
        {label}
      </div>
      {hint ? <p className="mt-2 text-sm leading-6 text-muted-foreground">{hint}</p> : null}
      <div className={cn(hint ? "mt-4" : "mt-3")}>{children}</div>
    </label>
  );
}

export function AiSettingsPanel() {
  const settingsQuery = useAiSettingsQuery();
  const saveMutation = useSaveAiSettingsMutation();
  const connectionTestMutation = useAiConnectionTestMutation();
  const service = useAiServiceControls();
  const [draft, setDraft] = useState<AiSettingsValue>(DEFAULT_AI_SETTINGS);
  const [modelLookupKey, setModelLookupKey] = useState({
    provider: DEFAULT_AI_SETTINGS.provider,
    apiKey: DEFAULT_AI_SETTINGS.apiKey,
    baseUrl: DEFAULT_AI_SETTINGS.baseUrl,
  });
  const [showApiKey, setShowApiKey] = useState(false);

  useEffect(() => {
    if (settingsQuery.data) {
      setDraft(settingsQuery.data);
      setModelLookupKey({
        provider: settingsQuery.data.provider,
        apiKey: settingsQuery.data.apiKey,
        baseUrl: settingsQuery.data.baseUrl,
      });
    }
  }, [settingsQuery.data]);

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      setModelLookupKey({
        provider: draft.provider,
        apiKey: draft.apiKey,
        baseUrl: draft.baseUrl,
      });
    }, 450);

    return () => window.clearTimeout(timeout);
  }, [draft.provider, draft.apiKey, draft.baseUrl]);

  const modelLookupDraft = useMemo<AiSettingsValue>(
    () => ({ ...draft, ...modelLookupKey }),
    [draft, modelLookupKey],
  );

  const modelCatalogQuery = useAiModelCatalogQuery(modelLookupDraft, true);

  const providerModels = useMemo(
    () => {
      const remoteModels = modelCatalogQuery.data?.models ?? [];
      return remoteModels.length > 0 ? remoteModels : getProviderModels(draft.provider);
    },
    [draft.provider, modelCatalogQuery.data?.models],
  );
  const hasCustomModel = !providerModels.includes(draft.model);
  const modelSelectValue = hasCustomModel ? CUSTOM_MODEL_VALUE : draft.model;
  const isConfigured = isAiConfigured(draft);
  const currentSettings = settingsQuery.data ?? DEFAULT_AI_SETTINGS;
  const modelCatalogBlockingReason = getModelCatalogBlockingReason(draft);
  const canRefreshModels = modelCatalogBlockingReason === null;
  const modelCatalogMessage =
    modelCatalogBlockingReason ??
    modelCatalogQuery.error?.message ??
    modelCatalogQuery.data?.message ??
    "修改 Provider、API Key 或 Base URL 后会自动获取可用模型。";
  const isDirty = useMemo(
    () =>
      JSON.stringify(normalizeAiSettings(draft)) !== JSON.stringify(normalizeAiSettings(currentSettings)),
    [draft, currentSettings],
  );
  const connectionResult = connectionTestMutation.data;
  const connectionError = connectionTestMutation.error;
  const connectionMessage = connectionError?.message ?? connectionResult?.message ?? "";
  const connectionSuccess = connectionError ? false : connectionResult?.success;

  const updateDraft = (updater: (current: AiSettingsValue) => AiSettingsValue) => {
    setDraft((current) => normalizeAiSettings(updater(current)));
  };

  const handleProviderChange = (provider: AiProviderKind) => {
    updateDraft((current) => {
      const nextModel = getProviderModels(provider).includes(current.model)
        ? current.model
        : getDefaultModel(provider);

      return {
        ...current,
        provider,
        model: nextModel,
      };
    });
  };

  const handleSave = async () => {
    try {
      await saveMutation.mutateAsync(draft);
      toast.success("AI 设置已保存到应用数据目录");
    } catch (error) {
      toast.error(`保存 AI 设置失败: ${String(error)}`);
    }
  };

  const handleServiceStart = async () => {
    try {
      await service.start();
      toast.success("AI 服务已启动");
    } catch (error) {
      toast.error(`启动 AI 服务失败: ${String(error)}`);
    }
  };

  const handleServiceStop = async () => {
    try {
      await service.stop();
      toast.success("AI 服务已停止");
    } catch (error) {
      toast.error(`停止 AI 服务失败: ${String(error)}`);
    }
  };

  const handleConnectionTest = async () => {
    try {
      const result = await connectionTestMutation.mutateAsync(draft);
      if (result.success) {
        toast.success("连通性测试成功");
      } else {
        toast.error("连通性测试失败");
      }
    } catch (error) {
      toast.error(`连通性测试失败: ${String(error)}`);
    }
  };

  const handleRefreshModels = async () => {
    try {
      setModelLookupKey({
        provider: draft.provider,
        apiKey: draft.apiKey,
        baseUrl: draft.baseUrl,
      });
      const result = await modelCatalogQuery.refetch();
      if (result.error) {
        toast.error(`获取模型列表失败: ${result.error.message}`);
        return;
      }

      const message = result.data?.message ?? "模型列表已刷新。";
      if (result.data?.source === "remote") {
        toast.success(message);
      } else {
        toast(message);
      }
    } catch (error) {
      toast.error(`获取模型列表失败: ${String(error)}`);
    }
  };

  if (settingsQuery.isLoading && !settingsQuery.data) {
    return (
      <section className="rounded-[2rem] border border-border/70 bg-background/92 p-6 shadow-[0_28px_90px_-48px_rgba(15,23,42,0.6)]">
        <div className="flex items-center gap-3 text-sm text-muted-foreground">
          <LoaderCircle className="size-4 animate-spin" />
          读取 AI 设置中...
        </div>
      </section>
    );
  }

  if (settingsQuery.error) {
    return (
      <section className="rounded-[2rem] border border-rose-500/20 bg-rose-500/10 p-6 text-sm text-rose-200">
        读取 AI 设置失败: {settingsQuery.error.message}
      </section>
    );
  }

  return (
    <motion.section
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.24, ease: "easeOut" }}
      className="relative overflow-hidden rounded-[2rem] border border-border/70 bg-[radial-gradient(circle_at_top_right,rgba(59,130,246,0.12),transparent_28%),linear-gradient(180deg,rgba(15,23,42,0.02),rgba(15,23,42,0.08))] p-6 shadow-[0_30px_100px_-52px_rgba(15,23,42,0.62)]"
    >
      <div className="pointer-events-none absolute -right-12 top-0 size-40 rounded-full bg-primary/12 blur-3xl" />
      <div className="pointer-events-none absolute bottom-0 left-0 h-28 w-72 bg-linear-to-r from-primary/10 to-transparent" />

      <div className="relative space-y-5">
        <header className="flex flex-col gap-4 border-b border-border/70 pb-5 xl:flex-row xl:items-end xl:justify-between">
          <div className="max-w-3xl">
            <div className="inline-flex items-center gap-2 rounded-full border border-primary/15 bg-primary/10 px-3 py-1 text-xs font-medium tracking-[0.18em] text-primary uppercase">
              <Orbit className="size-3.5" />
              Model Control Room
            </div>
            <h2 className="mt-4 text-2xl font-semibold tracking-tight text-foreground">
              AI 设置
            </h2>
            <p className="mt-2 text-sm leading-7 text-muted-foreground">
              这里负责 Provider、模型与服务状态的统一编排。设置会持久化到应用数据目录，
              AI 对话、统计报告和连通性测试都会读取同一份配置。
            </p>
          </div>

          <div className="flex flex-wrap gap-2">
            <span
              className={cn(
                "inline-flex items-center gap-2 rounded-full border px-3 py-2 text-xs font-medium tracking-[0.18em] uppercase",
                service.isRunning
                  ? "border-emerald-400/25 bg-emerald-500/10 text-emerald-200"
                  : "border-border/70 bg-background/75 text-muted-foreground",
              )}
            >
              <span
                className={cn(
                  "size-2 rounded-full",
                  service.isRunning ? "bg-emerald-400" : "bg-muted-foreground/60",
                )}
              />
              {service.isCheckingStatus ? "检测服务中" : service.isRunning ? "AI 服务运行中" : "AI 服务已停止"}
            </span>

            <span
              className={cn(
                "inline-flex items-center gap-2 rounded-full border px-3 py-2 text-xs font-medium tracking-[0.18em] uppercase",
                isConfigured
                  ? "border-primary/20 bg-primary/10 text-primary"
                  : "border-amber-500/20 bg-amber-500/10 text-amber-200",
              )}
            >
              <PlugZap className="size-3.5" />
              {isConfigured ? `${draft.provider} / ${draft.model}` : "配置尚未完整"}
            </span>
          </div>
        </header>

        <div className="grid gap-4 xl:grid-cols-[minmax(0,1.12fr)_minmax(18rem,0.88fr)]">
          <div className="space-y-4">
            <FieldShell
              label="Provider"
              hint={describeProvider(draft.provider)}
            >
              <SelectField
                value={draft.provider}
                onValueChange={(value) => handleProviderChange(value as AiProviderKind)}
              >
                {PROVIDER_OPTIONS.map((option) => (
                  <SelectItem
                    key={option.value}
                    value={option.value}
                    label={option.label}
                    description={option.eyebrow}
                  />
                ))}
              </SelectField>
            </FieldShell>

            <div className="grid gap-4 lg:grid-cols-2">
              <FieldShell
                label="可用模型"
                hint="优先展示自动获取到的模型；获取失败时会回退到内置列表或保留手动输入。"
              >
                <div className="space-y-3">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <div className="text-xs text-muted-foreground">
                      {modelCatalogQuery.data?.source === "remote"
                        ? "来源: 远端自动发现"
                        : modelCatalogQuery.data?.source === "fallback"
                          ? "来源: 内置回退列表"
                          : "来源: 手动输入 / 待获取"}
                    </div>
                    <ActionButton
                      tone="ghost"
                      disabled={!canRefreshModels || modelCatalogQuery.isFetching}
                      onClick={handleRefreshModels}
                    >
                      {modelCatalogQuery.isFetching ? (
                        <LoaderCircle className="size-4 animate-spin" />
                      ) : (
                        <RefreshCw className="size-4" />
                      )}
                      刷新模型
                    </ActionButton>
                  </div>

                  <SelectField
                    value={modelSelectValue}
                    onValueChange={(value) => {
                      if (value === CUSTOM_MODEL_VALUE) {
                        return;
                      }

                      updateDraft((current) => ({ ...current, model: value }));
                    }}
                  >
                    {providerModels.map((model) => (
                      <SelectItem key={model} value={model} label={model} />
                    ))}
                    <SelectItem
                      value={CUSTOM_MODEL_VALUE}
                      label="自定义模型"
                      description="在下方输入自定义模型 ID"
                    />
                  </SelectField>

                  <div
                    className={cn(
                      "rounded-[1rem] border px-3 py-3 text-xs leading-6",
                      modelCatalogTone(Boolean(modelCatalogBlockingReason), modelCatalogQuery.data?.source),
                    )}
                  >
                    {modelCatalogMessage}
                  </div>
                </div>
              </FieldShell>

              <FieldShell
                label="模型 ID"
                hint={modelIdHint(draft.provider)}
              >
                <input
                  type="text"
                  value={draft.model}
                  onChange={(event) =>
                    updateDraft((current) => ({ ...current, model: event.target.value }))
                  }
                  className={fieldInputClassName()}
                  placeholder={getDefaultModel(draft.provider) || "例如 deepseek-chat / gemini-2.5-flash"}
                />
              </FieldShell>
            </div>

            <FieldShell
              label="API Key"
              hint={providerRequiresApiKey(draft.provider) ? "密码模式保存。当前渠道需要 API Key。" : "可选。兼容本地 Ollama 这类无鉴权服务时可以留空。"}
            >
              <div className="relative">
                <KeyRound className="pointer-events-none absolute left-4 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
                <input
                  type={showApiKey ? "text" : "password"}
                  value={draft.apiKey}
                  onChange={(event) =>
                    updateDraft((current) => ({ ...current, apiKey: event.target.value }))
                  }
                  className={cn(fieldInputClassName(), "pl-11 pr-14")}
                  placeholder="输入 Provider API Key"
                />
                <button
                  type="button"
                  onClick={() => setShowApiKey((current) => !current)}
                  className="absolute right-3 top-1/2 inline-flex size-8 -translate-y-1/2 items-center justify-center rounded-full border border-border/70 bg-background/80 text-muted-foreground transition-colors hover:border-primary/20 hover:text-foreground"
                >
                  {showApiKey ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
                </button>
              </div>
            </FieldShell>

            <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_12rem]">
              <FieldShell
                label="Base URL"
                hint={providerRequiresBaseUrl(draft.provider) ? "当前兼容渠道必填。请填写兼容服务的 API 根地址。" : "可选。留空时使用 Provider 默认地址。"}
              >
                <input
                  type="text"
                  value={draft.baseUrl}
                  onChange={(event) =>
                    updateDraft((current) => ({ ...current, baseUrl: event.target.value }))
                  }
                  className={fieldInputClassName()}
                  placeholder={baseUrlPlaceholder(draft.provider)}
                />
              </FieldShell>

              <FieldShell
                label="Max Tokens"
                hint="用于持久化默认输出上限。"
              >
                <input
                  type="number"
                  min={1}
                  step={1}
                  value={draft.maxTokens}
                  onChange={(event) =>
                    updateDraft((current) => ({
                      ...current,
                      maxTokens: Number(event.target.value),
                    }))
                  }
                  className={fieldInputClassName()}
                />
              </FieldShell>
            </div>

            <FieldShell
              label="Temperature"
              hint="0 更稳定，1 更发散。当前值会传递到聊天与报告生成。"
            >
              <div className="rounded-[1.15rem] border border-border/70 bg-background/60 p-4">
                <div className="flex items-center justify-between gap-4">
                  <div>
                    <div className="text-sm font-medium text-foreground">
                      {draft.temperature.toFixed(1)}
                    </div>
                    <div className="text-xs text-muted-foreground">0 精确 · 1 创造性</div>
                  </div>
                  <Gauge className="size-4 text-primary" />
                </div>
                <Slider.Root
                  value={[draft.temperature]}
                  min={0}
                  max={1}
                  step={0.1}
                  onValueChange={([value]) =>
                    updateDraft((current) => ({ ...current, temperature: value ?? current.temperature }))
                  }
                  className="mt-4 flex w-full touch-none select-none items-center"
                >
                  <Slider.Track className="relative h-2 grow overflow-hidden rounded-full bg-muted/60">
                    <Slider.Range className="absolute h-full rounded-full bg-linear-to-r from-primary via-sky-400 to-cyan-300" />
                  </Slider.Track>
                  <Slider.Thumb className="block size-5 rounded-full border border-primary/30 bg-background shadow-[0_12px_30px_-12px_var(--color-primary)] outline-none transition-transform hover:scale-105" />
                </Slider.Root>
              </div>
            </FieldShell>
          </div>

          <aside className="space-y-4">
            <FieldShell label="服务管理" hint="这里只管理本地 AI sidecar 进程状态。">
              <div className="space-y-4">
                <div className="rounded-[1.2rem] border border-border/70 bg-background/65 p-4">
                  <div className="flex items-center justify-between gap-4">
                    <div>
                      <div className="text-sm font-medium text-foreground">AI 服务状态</div>
                      <div className="mt-1 text-sm text-muted-foreground">
                        {service.statusError
                          ? service.statusError.message
                          : service.isCheckingStatus
                            ? "正在检查 sidecar 状态..."
                            : service.isRunning
                              ? "sidecar 已启动，可直接处理聊天、报告和测试请求。"
                              : "sidecar 当前未运行。"}
                      </div>
                    </div>
                    <div
                      className={cn(
                        "inline-flex size-12 items-center justify-center rounded-[1rem] border",
                        service.isRunning
                          ? "border-emerald-400/20 bg-emerald-500/10 text-emerald-300"
                          : "border-border/70 bg-background/75 text-muted-foreground",
                      )}
                    >
                      <ServerCog className="size-5" />
                    </div>
                  </div>
                </div>

                <div className="flex flex-wrap gap-2">
                  <ActionButton
                    tone="primary"
                    disabled={service.isRunning || service.isStarting}
                    onClick={handleServiceStart}
                  >
                    {service.isStarting ? (
                      <LoaderCircle className="size-4 animate-spin" />
                    ) : (
                      <Play className="size-4" />
                    )}
                    启动服务
                  </ActionButton>
                  <ActionButton
                    tone="secondary"
                    disabled={!service.isRunning || service.isStopping}
                    onClick={handleServiceStop}
                  >
                    {service.isStopping ? (
                      <LoaderCircle className="size-4 animate-spin" />
                    ) : (
                      <Square className="size-4" />
                    )}
                    停止服务
                  </ActionButton>
                </div>
              </div>
            </FieldShell>

            <FieldShell
              label="自动启动"
              hint="应用启动完成后，会读取持久化设置并按需自动启动 AI sidecar。"
            >
              <div className="flex items-center justify-between gap-4 rounded-[1.2rem] border border-border/70 bg-background/65 px-4 py-4">
                <div>
                  <div className="text-sm font-medium text-foreground">随应用启动 AI 服务</div>
                  <div className="mt-1 text-sm text-muted-foreground">
                    当前为 {draft.autoStart ? "启用" : "关闭"}
                  </div>
                </div>
                <Switch.Root
                  checked={draft.autoStart}
                  onCheckedChange={(checked) =>
                    updateDraft((current) => ({ ...current, autoStart: checked }))
                  }
                  className={cn(
                    "relative inline-flex h-7 w-12 items-center rounded-full border transition-colors",
                    draft.autoStart
                      ? "border-primary/30 bg-primary/85"
                      : "border-border/70 bg-muted/55",
                  )}
                >
                  <Switch.Thumb
                    className={cn(
                      "block size-5 rounded-full bg-white shadow transition-transform",
                      draft.autoStart ? "translate-x-[1.45rem]" : "translate-x-1",
                    )}
                  />
                </Switch.Root>
              </div>
            </FieldShell>

            <FieldShell
              label="连通性测试"
              hint="使用当前表单草稿直接向 Provider 发最小请求，不必先保存。"
            >
              <div className="space-y-4">
                <ActionButton
                  tone="secondary"
                  disabled={connectionTestMutation.isPending}
                  onClick={handleConnectionTest}
                >
                  {connectionTestMutation.isPending ? (
                    <LoaderCircle className="size-4 animate-spin" />
                  ) : (
                    <TestTubeDiagonal className="size-4" />
                  )}
                  测试连接
                </ActionButton>

                <div
                  className={cn(
                    "rounded-[1.2rem] border px-4 py-4 text-sm leading-6",
                    connectionTone(connectionSuccess, connectionMessage.length > 0),
                  )}
                >
                  {connectionMessage.length > 0 ? (
                    <>
                      <div className="font-medium">
                        {connectionSuccess ? "连接成功" : "连接失败"}
                        {connectionResult ? ` · ${connectionResult.latencyMs}ms` : ""}
                      </div>
                      <div className="mt-1">{connectionMessage}</div>
                    </>
                  ) : (
                    "尚未执行测试。保存前可先检查 Provider、API Key、Base URL 和模型是否可用。"
                  )}
                </div>
              </div>
            </FieldShell>
          </aside>
        </div>

        <footer className="flex flex-col gap-3 border-t border-border/70 pt-5 sm:flex-row sm:items-center sm:justify-between">
          <div className="text-sm text-muted-foreground">
            {isDirty ? "你有未保存的 AI 配置变更。" : "当前草稿与已保存配置一致。"}
          </div>
          <div className="flex flex-wrap gap-2">
            <ActionButton
              tone="ghost"
              disabled={!isDirty}
              onClick={() => setDraft(currentSettings)}
            >
              重置草稿
            </ActionButton>
            <ActionButton
              tone="primary"
              disabled={saveMutation.isPending || !isDirty}
              onClick={handleSave}
            >
              {saveMutation.isPending ? (
                <LoaderCircle className="size-4 animate-spin" />
              ) : (
                <Save className="size-4" />
              )}
              保存设置
            </ActionButton>
          </div>
        </footer>
      </div>
    </motion.section>
  );
}
