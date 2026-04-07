import { createAnthropic } from "@ai-sdk/anthropic";
import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { createOpenAI } from "@ai-sdk/openai";
import type { LanguageModel } from "ai";

import type {
  ModelCatalogResult,
  ModelCatalogSettings,
  ProviderId,
  ProviderSettings,
} from "../types.js";
import { getErrorMessage, isRecord } from "../utils.js";

const DEFAULT_OPENAI_BASE_URL = "https://api.openai.com/v1";
const DEFAULT_ANTHROPIC_BASE_URL = "https://api.anthropic.com";
const DEFAULT_GOOGLE_BASE_URL = "https://generativelanguage.googleapis.com/v1beta";
const ANTHROPIC_VERSION = "2023-06-01";

export const DEFAULT_MODELS = {
  openai: ["gpt-4o", "gpt-4o-mini", "gpt-4.1-mini", "o3-mini"],
  openai_compatible: [],
  claude: ["claude-sonnet-4-5", "claude-haiku-4-5", "claude-opus-4-1"],
  gemini: ["gemini-2.5-flash", "gemini-2.5-flash-lite", "gemini-2.5-pro"],
} as const satisfies Record<ProviderId, readonly string[]>;

function normalizeOptionalString(value: string | undefined): string | undefined {
  if (typeof value !== "string") {
    return undefined;
  }

  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

function withTrailingSlash(value: string): string {
  return value.endsWith("/") ? value : `${value}/`;
}

function buildUrl(
  baseUrl: string,
  path: string,
  searchParams?: Record<string, string | undefined>,
): URL {
  const url = new URL(path.replace(/^\/+/, ""), withTrailingSlash(baseUrl));

  for (const [key, value] of Object.entries(searchParams ?? {})) {
    if (value !== undefined) {
      url.searchParams.set(key, value);
    }
  }

  return url;
}


function uniqueModelIds(models: Iterable<string>): string[] {
  return [...new Set(
    [...models]
      .map((model) => model.trim())
      .filter((model) => model.length > 0),
  )];
}

async function fetchJson(
  url: URL,
  init?: RequestInit,
): Promise<unknown> {
  const response = await fetch(url, init);

  if (!response.ok) {
    const body = await response.text().catch(() => "");
    throw new Error(`${response.status} ${response.statusText}${body ? `: ${body}` : ""}`);
  }

  return response.json();
}

function parseOpenAIModelIds(payload: unknown): string[] {
  if (!isRecord(payload) || !Array.isArray(payload.data)) {
    return [];
  }

  return uniqueModelIds(
    payload.data.flatMap((entry) => {
      if (!isRecord(entry) || typeof entry.id !== "string") {
        return [];
      }

      return [entry.id];
    }),
  );
}

function parseGeminiModelIds(payload: unknown): string[] {
  if (!isRecord(payload) || !Array.isArray(payload.models)) {
    return [];
  }

  return payload.models.flatMap((entry) => {
    if (!isRecord(entry)) {
      return [];
    }

    const supportedGenerationMethods = Array.isArray(entry.supportedGenerationMethods)
      ? entry.supportedGenerationMethods.filter((method): method is string => typeof method === "string")
      : [];

    if (
      supportedGenerationMethods.length > 0 &&
      !supportedGenerationMethods.includes("generateContent")
    ) {
      return [];
    }

    const rawName =
      typeof entry.name === "string"
        ? entry.name
        : typeof entry.baseModelId === "string"
          ? entry.baseModelId
          : undefined;

    if (!rawName) {
      return [];
    }

    return [rawName.replace(/^models\//, "")];
  });
}

function createFallbackCatalog(provider: ProviderId, message: string): ModelCatalogResult {
  const fallbackModels = [...DEFAULT_MODELS[provider]];

  if (fallbackModels.length === 0) {
    return {
      models: [],
      source: "empty",
      message,
    };
  }

  return {
    models: fallbackModels,
    source: "fallback",
    message: `${message}，已回退到内置模型列表。`,
  };
}

async function listOpenAIModels(
  apiKey: string | undefined,
  baseUrl: string,
): Promise<string[]> {
  const payload = await fetchJson(buildUrl(baseUrl, "models"), {
    headers: {
      ...(apiKey ? { Authorization: `Bearer ${apiKey}` } : {}),
    },
  });

  return parseOpenAIModelIds(payload);
}

async function listAnthropicModels(
  apiKey: string,
  baseUrl: string,
): Promise<string[]> {
  const payload = await fetchJson(buildUrl(baseUrl, "v1/models"), {
    headers: {
      "anthropic-version": ANTHROPIC_VERSION,
      "x-api-key": apiKey,
    },
  });

  return parseOpenAIModelIds(payload);
}

async function listGeminiModels(
  apiKey: string,
  baseUrl: string,
): Promise<string[]> {
  const models: string[] = [];
  let pageToken: string | undefined;
  let pages = 0;
  const maxPages = 20;

  do {
    const payload = await fetchJson(buildUrl(baseUrl, "models", {
      pageSize: "1000",
      pageToken,
    }), {
      headers: {
        "x-goog-api-key": apiKey,
      },
    });

    models.push(...parseGeminiModelIds(payload));
    pageToken =
      isRecord(payload) && typeof payload.nextPageToken === "string"
        ? payload.nextPageToken
        : undefined;
    pages++;
  } while (pageToken && pages < maxPages);

  return uniqueModelIds(models);
}

function providerRequiresApiKey(provider: ProviderId): boolean {
  return provider === "openai" || provider === "claude" || provider === "gemini";
}

export async function listModels(settings: ModelCatalogSettings): Promise<ModelCatalogResult> {
  const provider = settings.provider;
  const apiKey = normalizeOptionalString(settings.apiKey);
  const baseUrl = normalizeOptionalString(settings.baseUrl);

  if (provider === "openai_compatible" && !baseUrl) {
    return {
      models: [],
      source: "empty",
      message: "请先填写兼容服务 Base URL，再获取可用模型。",
    };
  }

  if (providerRequiresApiKey(provider) && !apiKey) {
    return createFallbackCatalog(provider, "缺少 API Key，无法自动获取模型");
  }

  try {
    let models: string[] = [];

    switch (provider) {
      case "openai":
        models = await listOpenAIModels(apiKey, baseUrl ?? DEFAULT_OPENAI_BASE_URL);
        break;
      case "openai_compatible":
        models = await listOpenAIModels(apiKey, baseUrl!);
        break;
      case "claude":
        models = await listAnthropicModels(apiKey!, baseUrl ?? DEFAULT_ANTHROPIC_BASE_URL);
        break;
      case "gemini":
        models = await listGeminiModels(apiKey!, baseUrl ?? DEFAULT_GOOGLE_BASE_URL);
        break;
    }

    if (models.length === 0) {
      return createFallbackCatalog(provider, "远端接口未返回可用模型");
    }

    return {
      models,
      source: "remote",
      message: `已自动获取 ${models.length} 个可用模型。`,
    };
  } catch (error) {
    return createFallbackCatalog(provider, `自动获取模型失败: ${getErrorMessage(error)}`);
  }
}

export function createModel(settings: ProviderSettings): LanguageModel {
  const apiKey = normalizeOptionalString(settings.apiKey);
  const baseUrl = normalizeOptionalString(settings.baseUrl);

  switch (settings.provider) {
    case "openai": {
      const openai = createOpenAI({
        ...(apiKey ? { apiKey } : {}),
        ...(baseUrl ? { baseURL: baseUrl } : {}),
      });
      return openai(settings.model);
    }
    case "openai_compatible": {
      if (!baseUrl) {
        throw new Error("OpenAI Compatible 渠道缺少 Base URL");
      }

      const compatible = createOpenAI({
        ...(apiKey ? { apiKey } : {}),
        baseURL: baseUrl,
      });

      return compatible(settings.model);
    }
    case "claude": {
      const anthropic = createAnthropic({
        ...(apiKey ? { apiKey } : {}),
        ...(baseUrl ? { baseURL: baseUrl } : {}),
      });
      return anthropic(settings.model);
    }
    case "gemini": {
      const google = createGoogleGenerativeAI({
        ...(apiKey ? { apiKey } : {}),
        ...(baseUrl ? { baseURL: baseUrl } : {}),
      });
      return google(settings.model);
    }
  }
}
