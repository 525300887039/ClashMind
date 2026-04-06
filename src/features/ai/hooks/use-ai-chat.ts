import { useEffect } from "react";
import { useMutation } from "@tanstack/react-query";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  api,
  type AiChatMessage,
  type AiChatParams,
  type AiProviderSettings,
  type AiStreamEvent,
} from "@/lib/tauri-api";
import { useAiStore, type AiMessage } from "@/stores/ai-store";

const DEFAULT_CHAT_SETTINGS: AiProviderSettings = {
  provider: "openai",
  model: "gpt-4o-mini",
  temperature: 0.3,
};

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

function handleStreamEvent(event: AiStreamEvent) {
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
        store.setToolCallResult(activeMessageId, event.id, event.content);
      }
      return;
    }
    case "done": {
      if (activeMessageId !== null) {
        store.finalizeStream(activeMessageId);
        store.setActiveStreamMessageId(null);
      }
      store.setLoading(false);
      store.setError(null);
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
      handleStreamEvent(event.payload);
    });
  }
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
    ensureAiStreamListener();
  }, []);

  const sendMessageMutation = useMutation<void, Error, string>({
    mutationFn: async (rawInput) => {
      const content = rawInput.trim();
      if (!content) {
        return;
      }

      const store = useAiStore.getState();
      const conversation = buildConversation(store.messages, content);

      store.setError(null);
      store.addMessage({ role: "user", content });

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
          settings: DEFAULT_CHAT_SETTINGS,
        };

        await ensureAiServiceRunning();
        await api.ai.chat(params);
      } catch (error) {
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
