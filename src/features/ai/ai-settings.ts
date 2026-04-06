import { api, type AiProviderSettings } from "@/lib/tauri-api";
import { useAppStore } from "@/stores/app-store";

type AiSettingsResolution =
  | { ok: true; settings: AiProviderSettings }
  | { ok: false; message: string };

export function providerRequiresApiKey(provider: AiProviderSettings["provider"]) {
  return provider !== "ollama";
}

export function resolveAiSettings(): AiSettingsResolution {
  const {
    aiProvider,
    aiModel,
    aiApiKey,
    aiBaseUrl,
    aiTemperature,
  } = useAppStore.getState();

  const model = aiModel.trim();
  const apiKey = aiApiKey.trim();
  const baseUrl = aiBaseUrl.trim();

  if (!model) {
    return {
      ok: false,
      message: "请先在设置页配置 AI 模型。",
    };
  }

  if (providerRequiresApiKey(aiProvider) && !apiKey) {
    return {
      ok: false,
      message: "请先在设置页配置 AI Provider 和 API Key。",
    };
  }

  return {
    ok: true,
    settings: {
      provider: aiProvider,
      model,
      temperature: Number.isFinite(aiTemperature) ? aiTemperature : 0.3,
      ...(apiKey ? { apiKey } : {}),
      ...(baseUrl ? { baseUrl } : {}),
    },
  };
}

export async function ensureAiServiceRunning() {
  const isRunning = await api.ai.status();
  if (!isRunning) {
    await api.ai.start();
  }
}
