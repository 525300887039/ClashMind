import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  api,
  type AiConnectionTestResult,
  type AiModelCatalog,
  type AiProviderKind,
  type AiProviderSettings,
  type AiSettings,
} from "@/lib/tauri-api";
import { normalizeError } from "@/lib/error";
import { queryClient } from "@/lib/query-client";

const AI_SETTINGS_QUERY_KEY = ["ai-settings"] as const;
const AI_SERVICE_STATUS_QUERY_KEY = ["ai-service-status"] as const;
const AI_MODEL_CATALOG_QUERY_KEY = ["ai-model-catalog"] as const;
const LEGACY_APP_STORE_KEY = "clashmind-store";
const LEGACY_AI_SETTINGS_MISSING_ERROR = "AI 设置文件不存在";
const LEGACY_DEEPSEEK_BASE_URL = "https://api.deepseek.com/v1";
const LEGACY_OLLAMA_OPENAI_BASE_URL = "http://127.0.0.1:11434/v1";
const LEGACY_AI_STATE_KEYS = [
  "aiProvider",
  "aiModel",
  "aiApiKey",
  "aiBaseUrl",
  "aiTemperature",
  "autoStart",
] as const;

export const AI_PROVIDER_MODELS: Record<AiProviderKind, readonly string[]> = {
  openai: ["gpt-4o", "gpt-4o-mini", "gpt-4.1-mini", "o3-mini"],
  openai_compatible: [],
  claude: ["claude-sonnet-4-5", "claude-haiku-4-5", "claude-opus-4-1"],
  gemini: ["gemini-2.5-flash", "gemini-2.5-flash-lite", "gemini-2.5-pro"],
};

const AI_PROVIDER_DEFAULT_MODELS: Record<AiProviderKind, string> = {
  openai: "gpt-4o-mini",
  openai_compatible: "",
  claude: "claude-sonnet-4-5",
  gemini: "gemini-2.5-flash",
};

export const DEFAULT_AI_SETTINGS: AiSettings = {
  provider: "openai",
  model: AI_PROVIDER_DEFAULT_MODELS.openai,
  apiKey: "",
  baseUrl: "",
  temperature: 0.3,
  maxTokens: 4096,
  autoStart: false,
};

let legacyAiSettingsMigrationPromise: Promise<AiSettings | null> | null = null;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function getLegacyProviderDefaultBaseUrl(value: unknown): string {
  if (value === "deepseek") {
    return LEGACY_DEEPSEEK_BASE_URL;
  }

  if (value === "ollama") {
    return LEGACY_OLLAMA_OPENAI_BASE_URL;
  }

  return "";
}

function isAiProviderKind(value: unknown): value is AiProviderKind {
  return value === "openai" || value === "openai_compatible" || value === "claude" || value === "gemini";
}

function normalizeProvider(value: unknown): AiProviderKind {
  if (isAiProviderKind(value)) {
    return value;
  }

  if (value === "deepseek" || value === "ollama") {
    return "openai_compatible";
  }

  return DEFAULT_AI_SETTINGS.provider;
}

function clampTemperature(value: number) {
  if (!Number.isFinite(value)) {
    return DEFAULT_AI_SETTINGS.temperature;
  }

  return Math.min(1, Math.max(0, value));
}

export function providerRequiresApiKey(provider: AiProviderKind) {
  return provider === "openai" || provider === "claude" || provider === "gemini";
}

export function providerRequiresBaseUrl(provider: AiProviderKind) {
  return provider === "openai_compatible";
}

export function getProviderModels(provider: AiProviderKind) {
  return AI_PROVIDER_MODELS[provider];
}

export function getDefaultModel(provider: AiProviderKind) {
  return AI_PROVIDER_DEFAULT_MODELS[provider] ?? DEFAULT_AI_SETTINGS.model;
}

export function normalizeAiSettings(settings: AiSettings): AiSettings {
  const provider = settings.provider;
  const model = settings.model.trim();
  const baseUrl = settings.baseUrl.trim();
  const maxTokens = Math.floor(Number.isFinite(settings.maxTokens) ? settings.maxTokens : 0);

  return {
    provider,
    model: model.length > 0 ? model : getDefaultModel(provider),
    apiKey: settings.apiKey.trim(),
    baseUrl,
    temperature: clampTemperature(settings.temperature),
    maxTokens: maxTokens > 0 ? maxTokens : DEFAULT_AI_SETTINGS.maxTokens,
    autoStart: Boolean(settings.autoStart),
  };
}

export function getModelCatalogBlockingReason(settings: AiSettings): string | null {
  const normalized = normalizeAiSettings(settings);

  if (providerRequiresApiKey(normalized.provider) && normalized.apiKey.length === 0) {
    return "请先填写 API Key，再自动获取模型。";
  }

  if (providerRequiresBaseUrl(normalized.provider) && normalized.baseUrl.length === 0) {
    return "请先填写兼容服务 Base URL，再自动获取模型。";
  }

  return null;
}

export function canFetchProviderModels(settings: AiSettings) {
  return getModelCatalogBlockingReason(settings) === null;
}

export function isAiConfigured(settings: AiSettings | null | undefined) {
  if (!settings) {
    return false;
  }

  const normalized = normalizeAiSettings(settings);
  return (
    normalized.model.length > 0 &&
    (!providerRequiresApiKey(normalized.provider) || normalized.apiKey.length > 0) &&
    (!providerRequiresBaseUrl(normalized.provider) || normalized.baseUrl.length > 0)
  );
}

function getLegacyStoreState() {
  if (typeof window === "undefined") {
    return null;
  }

  const raw = window.localStorage.getItem(LEGACY_APP_STORE_KEY);
  if (!raw) {
    return null;
  }

  try {
    const parsed = JSON.parse(raw);
    if (isRecord(parsed.state)) {
      return parsed.state;
    }

    return isRecord(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function readLegacyAiSettings() {
  const state = getLegacyStoreState();
  if (!state) {
    return null;
  }

  const hasLegacyAiState = LEGACY_AI_STATE_KEYS.some((key) => key in state);
  if (!hasLegacyAiState) {
    return null;
  }

  const provider = normalizeProvider(state.aiProvider);
  const fallbackBaseUrl = getLegacyProviderDefaultBaseUrl(state.aiProvider);
  const rawBaseUrl = typeof state.aiBaseUrl === "string" ? state.aiBaseUrl.trim() : "";
  const temperature =
    typeof state.aiTemperature === "number"
      ? state.aiTemperature
      : DEFAULT_AI_SETTINGS.temperature;
  const autoStart =
    typeof state.autoStart === "boolean"
      ? state.autoStart
      : DEFAULT_AI_SETTINGS.autoStart;

  return normalizeAiSettings({
    provider,
    model: typeof state.aiModel === "string" ? state.aiModel : "",
    apiKey: typeof state.aiApiKey === "string" ? state.aiApiKey : "",
    baseUrl: rawBaseUrl || fallbackBaseUrl,
    temperature,
    maxTokens: DEFAULT_AI_SETTINGS.maxTokens,
    autoStart,
  });
}

function clearLegacyAiSettings() {
  if (typeof window === "undefined") {
    return;
  }

  const raw = window.localStorage.getItem(LEGACY_APP_STORE_KEY);
  if (!raw) {
    return;
  }

  try {
    const parsed = JSON.parse(raw);

    if (isRecord(parsed.state)) {
      const nextState = { ...parsed.state };
      for (const key of LEGACY_AI_STATE_KEYS) {
        delete nextState[key];
      }

      window.localStorage.setItem(
        LEGACY_APP_STORE_KEY,
        JSON.stringify({
          ...parsed,
          state: nextState,
        }),
      );
      return;
    }

    if (isRecord(parsed)) {
      const nextParsed = { ...parsed };
      for (const key of LEGACY_AI_STATE_KEYS) {
        delete nextParsed[key];
      }

      window.localStorage.setItem(LEGACY_APP_STORE_KEY, JSON.stringify(nextParsed));
    }
  } catch {
    // Ignore malformed legacy state and avoid blocking settings load.
  }
}

function isMissingAiSettingsError(error: unknown) {
  return normalizeError(error).message.includes(LEGACY_AI_SETTINGS_MISSING_ERROR);
}

async function migrateLegacyAiSettingsIfNeeded() {
  if (legacyAiSettingsMigrationPromise) {
    return legacyAiSettingsMigrationPromise;
  }

  legacyAiSettingsMigrationPromise = (async () => {
    const legacySettings = readLegacyAiSettings();
    if (!legacySettings) {
      return null;
    }

    await api.ai.setSettings(legacySettings);
    clearLegacyAiSettings();
    return legacySettings;
  })();

  try {
    return await legacyAiSettingsMigrationPromise;
  } finally {
    legacyAiSettingsMigrationPromise = null;
  }
}

function ensureProviderSettings(settings: AiSettings): AiProviderSettings {
  const normalized = normalizeAiSettings(settings);

  if (normalized.model.length === 0) {
    throw new Error("请先在设置页配置 AI 模型。");
  }

  if (providerRequiresApiKey(normalized.provider) && normalized.apiKey.length === 0) {
    throw new Error("请先在设置页配置当前 Provider 的 API Key。");
  }

  if (providerRequiresBaseUrl(normalized.provider) && normalized.baseUrl.length === 0) {
    throw new Error("请先在设置页配置兼容服务 Base URL。");
  }

  return {
    provider: normalized.provider,
    model: normalized.model,
    temperature: normalized.temperature,
    maxTokens: normalized.maxTokens,
    ...(normalized.apiKey ? { apiKey: normalized.apiKey } : {}),
    ...(normalized.baseUrl ? { baseUrl: normalized.baseUrl } : {}),
  };
}

async function fetchAiSettings() {
  try {
    return normalizeAiSettings(await api.ai.getSettings());
  } catch (error) {
    if (!isMissingAiSettingsError(error)) {
      throw normalizeError(error);
    }

    const migratedSettings = await migrateLegacyAiSettingsIfNeeded();
    return migratedSettings ?? DEFAULT_AI_SETTINGS;
  }
}

export async function getAiSettingsSnapshot() {
  const cached = queryClient.getQueryData<AiSettings>(AI_SETTINGS_QUERY_KEY);
  if (cached) {
    return cached;
  }

  return queryClient.fetchQuery({
    queryKey: AI_SETTINGS_QUERY_KEY,
    queryFn: fetchAiSettings,
  });
}

export async function resolveAiProviderSettings() {
  const settings = await getAiSettingsSnapshot();
  return ensureProviderSettings(settings);
}

export async function ensureAiServiceRunning() {
  const isRunning = await api.ai.status();
  if (!isRunning) {
    await api.ai.start();
  }
}

export function useAiSettingsQuery() {
  return useQuery({
    queryKey: AI_SETTINGS_QUERY_KEY,
    queryFn: fetchAiSettings,
  });
}

export function useSaveAiSettingsMutation() {
  const qc = useQueryClient();

  return useMutation<AiSettings, Error, AiSettings>({
    mutationFn: async (settings) => {
      const normalized = normalizeAiSettings(settings);
      await api.ai.setSettings(normalized);
      return normalized;
    },
    onSuccess: async (normalized) => {
      qc.setQueryData(AI_SETTINGS_QUERY_KEY, normalized);
      await qc.invalidateQueries({ queryKey: AI_SETTINGS_QUERY_KEY });
    },
  });
}

export function useAiConnectionTestMutation() {
  return useMutation<AiConnectionTestResult, Error, AiSettings>({
    mutationFn: async (settings) => {
      const normalized = normalizeAiSettings(settings);
      ensureProviderSettings(normalized);
      return api.ai.testConnection(normalized);
    },
  });
}

export function useAiModelCatalogQuery(settings: AiSettings, enabled: boolean) {
  const normalized = normalizeAiSettings(settings);
  const canFetch = canFetchProviderModels(normalized);

  return useQuery<AiModelCatalog, Error>({
    queryKey: [
      ...AI_MODEL_CATALOG_QUERY_KEY,
      normalized.provider,
      normalized.baseUrl,
      normalized.apiKey,
    ],
    queryFn: () => api.ai.fetchModels(normalized),
    enabled: enabled && canFetch,
    retry: false,
    staleTime: 30_000,
    refetchOnWindowFocus: false,
  });
}

export function useAiServiceControls() {
  const qc = useQueryClient();

  const refreshStatus = async () => {
    await qc.invalidateQueries({ queryKey: AI_SERVICE_STATUS_QUERY_KEY });
  };

  const statusQuery = useQuery({
    queryKey: AI_SERVICE_STATUS_QUERY_KEY,
    queryFn: api.ai.status,
    refetchInterval: 3000,
  });

  const startMutation = useMutation({
    mutationFn: () => api.ai.start(),
    onSettled: refreshStatus,
  });

  const stopMutation = useMutation({
    mutationFn: () => api.ai.stop(),
    onSettled: refreshStatus,
  });

  return {
    isRunning: statusQuery.data ?? false,
    isCheckingStatus: statusQuery.isLoading,
    statusError: statusQuery.error ? normalizeError(statusQuery.error) : null,
    refreshStatus,
    start: startMutation.mutateAsync,
    stop: stopMutation.mutateAsync,
    isStarting: startMutation.isPending,
    isStopping: stopMutation.isPending,
  };
}
