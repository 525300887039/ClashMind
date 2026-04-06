import { useEffect } from "react";
import { useMutation } from "@tanstack/react-query";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  api,
  type AiChatMessage,
  type AiChatParams,
  type AiProviderSettings,
  type AiStreamEvent,
  type ConfigApplyPayload,
  type SaveConversationMessageParams,
  isPendingConfigChangeResult,
} from "@/lib/tauri-api";
import { useAppStore } from "@/stores/app-store";
import { useAiStore, type AiMessage } from "@/stores/ai-store";

const AI_STREAM_LISTENER_KEY = "__clashmind_ai_stream_listener__" as const;

type GlobalAiListenerRegistry = typeof globalThis & {
  [AI_STREAM_LISTENER_KEY]?: Promise<UnlistenFn>;
};

function normalizeError(error: unknown) {
  return error instanceof Error ? error : new Error(String(error));
}

function buildConversation(messages: AiMessage[], nextUserInput: string): AiChatMessage[] {
  return [
    ...messages
      .filter((message) => !message.isStreaming && message.content.trim().length > 0)
      .map((message) => ({
        role: message.role,
        content: message.content,
      })),
    { role: "user", content: nextUserInput },
  ];
}

function providerRequiresApiKey(provider: AiProviderSettings["provider"]) {
  return provider !== "ollama";
}

function getPersistenceModel() {
  const model = useAppStore.getState().aiModel.trim();
  return model.length > 0 ? model : undefined;
}

function getChatSettings() {
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
      ok: false as const,
      message: "请先在设置页配置 AI 模型。",
    };
  }

  if (providerRequiresApiKey(aiProvider) && !apiKey) {
    return {
      ok: false as const,
      message: "请先在设置页配置 AI Provider 和 API Key。",
    };
  }

  const settings: AiProviderSettings = {
    provider: aiProvider,
    model,
    temperature: Number.isFinite(aiTemperature) ? aiTemperature : 0.3,
    ...(apiKey ? { apiKey } : {}),
    ...(baseUrl ? { baseUrl } : {}),
  };

  return {
    ok: true as const,
    settings,
  };
}

async function persistConversationMessage(params: SaveConversationMessageParams) {
  await api.ai.saveConversationMessage(params);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isConfigApplyPayload(value: unknown): value is ConfigApplyPayload {
  return (
    isRecord(value) &&
    typeof value.originalConfig === "string" &&
    typeof value.modifiedConfig === "string"
  );
}

function extractPendingConfigToolResult(value: unknown): {
  result: unknown;
  applyPayload: ConfigApplyPayload | null;
  confirmationBatchId: string | null;
} {
  if (!isPendingConfigChangeResult(value)) {
    return {
      result: value,
      applyPayload: null,
      confirmationBatchId: null,
    };
  }

  const confirmationBatchId =
    typeof value.confirmationBatchId === "string" ? value.confirmationBatchId : null;
  const applyPayload =
    isRecord(value) && isConfigApplyPayload(value.applyPayload) ? value.applyPayload : null;

  return {
    result: {
      action: value.action,
      params: value.params,
      status: value.status,
      diff: value.diff,
      confirmationBatchId: value.confirmationBatchId,
      confirmationBatchSize: value.confirmationBatchSize,
      isLatestInBatch: value.isLatestInBatch,
    },
    applyPayload,
    confirmationBatchId,
  };
}

async function handleStreamEvent(event: AiStreamEvent) {
  const store = useAiStore.getState();
  const activeMessageId = store.activeStreamMessageId;

  switch (event.type) {
    case "text_delta": {
      if (activeMessageId !== null) {
        store.appendToMessage(activeMessageId, event.content);
      }
      return;
    }
    case "tool_call": {
      if (activeMessageId !== null) {
        store.upsertToolCall(activeMessageId, {
          id: event.id,
          name: event.name,
          input: event.input,
          status: "executing",
        });
      }
      return;
    }
    case "tool_result": {
      if (activeMessageId !== null) {
        const extractedResult = extractPendingConfigToolResult(event.content);

        if (
          extractedResult.confirmationBatchId !== null &&
          extractedResult.applyPayload !== null
        ) {
          store.setConfigApplyPayload(
            extractedResult.confirmationBatchId,
            extractedResult.applyPayload,
          );
        }

        store.setToolCallResult(activeMessageId, event.id, extractedResult.result);
      }
      return;
    }
    case "done": {
      const assistantMessage =
        activeMessageId !== null
          ? store.messages.find((message) => message.id === activeMessageId) ?? null
          : null;

      if (activeMessageId !== null) {
        store.finalizeStream(activeMessageId);
        store.setActiveStreamMessageId(null);
      }
      store.setLoading(false);
      store.setError(null);

      if (assistantMessage !== null) {
        try {
          await persistConversationMessage({
            role: assistantMessage.role,
            content: assistantMessage.content,
            toolCalls:
              assistantMessage.toolCalls.length > 0 ? assistantMessage.toolCalls : undefined,
            tokensUsed: event.tokensUsed,
            model: getPersistenceModel(),
          });
        } catch (error) {
          useAiStore
            .getState()
            .setError(`保存对话历史失败: ${normalizeError(error).message}`);
        }
      }

      return;
    }
    case "error": {
      if (activeMessageId !== null) {
        store.finalizeStream(activeMessageId, { markPendingToolCallsAsError: true });
        store.setActiveStreamMessageId(null);
      }
      store.setLoading(false);
      store.setError(event.message);
      return;
    }
    default: {
      const exhaustiveEvent: never = event;
      return exhaustiveEvent;
    }
  }
}

function ensureAiStreamListener() {
  const registry = globalThis as GlobalAiListenerRegistry;

  if (registry[AI_STREAM_LISTENER_KEY] === undefined) {
    registry[AI_STREAM_LISTENER_KEY] = listen<AiStreamEvent>("ai-stream", (event) => {
      void handleStreamEvent(event.payload);
    }).catch((error: unknown) => {
      delete registry[AI_STREAM_LISTENER_KEY];
      throw normalizeError(error);
    });
  }

  return registry[AI_STREAM_LISTENER_KEY];
}

async function ensureAiServiceRunning() {
  const isRunning = await api.ai.status();
  if (!isRunning) {
    await api.ai.start();
  }
}

export function useAiChat() {
  const messages = useAiStore((state) => state.messages);
  const isLoading = useAiStore((state) => state.isLoading);
  const error = useAiStore((state) => state.error);
  const clearMessages = useAiStore((state) => state.clearMessages);

  useEffect(() => {
    void ensureAiStreamListener().catch((error) => {
      useAiStore.getState().setError(normalizeError(error).message);
    });
  }, []);

  const sendMessageMutation = useMutation<void, Error, string>({
    mutationFn: async (rawInput) => {
      const content = rawInput.trim();
      if (!content) {
        return;
      }

      const store = useAiStore.getState();
      const conversation = buildConversation(store.messages, content);
      const settingsResult = getChatSettings();

      if (!settingsResult.ok) {
        const settingsError = new Error(settingsResult.message);
        store.setError(settingsError.message);
        throw settingsError;
      }

      store.setError(null);
      const userMessageId = store.addMessage({ role: "user", content });

      const assistantMessageId = store.addMessage({
        role: "assistant",
        content: "",
        isStreaming: true,
      });

      store.setActiveStreamMessageId(assistantMessageId);
      store.setLoading(true);

      try {
        const params: AiChatParams = {
          messages: conversation,
          settings: settingsResult.settings,
        };

        await ensureAiStreamListener();
        await ensureAiServiceRunning();
        await api.ai.chat(params);
        void persistConversationMessage({
          role: "user",
          content,
          model: settingsResult.settings.model,
        }).catch((error: unknown) => {
          console.warn("[ClashMind] 保存用户对话历史失败:", normalizeError(error));
        });
      } catch (error) {
        store.removeMessage(userMessageId);
        store.removeMessage(assistantMessageId);
        store.setActiveStreamMessageId(null);
        store.setLoading(false);

        const normalizedError = normalizeError(error);
        store.setError(normalizedError.message);
        throw normalizedError;
      }
    },
  });

  return {
    messages,
    isLoading,
    error,
    sendMessage: sendMessageMutation.mutate,
    clearMessages,
  };
}
