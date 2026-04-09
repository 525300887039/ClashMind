import { create } from "zustand";
import {
  isOptimizationToolResult,
  isPendingConfigChangeResult,
  type AiChatRole,
  type ConfigApplyPayload,
} from "@/lib/tauri-api";

export type AiToolCallStatus =
  | "pending"
  | "executing"
  | "awaiting_confirmation"
  | "completed"
  | "applied"
  | "rejected"
  | "error";

export interface AiToolCall {
  id: string;
  name: string;
  input: Record<string, unknown>;
  result?: unknown;
  status: AiToolCallStatus;
}

export interface AiMessage {
  id: string;
  role: AiChatRole;
  content: string;
  toolCalls: AiToolCall[];
  timestamp: number;
  isStreaming: boolean;
}

interface AddMessageInput {
  role: AiChatRole;
  content: string;
  toolCalls?: AiToolCall[];
  isStreaming?: boolean;
}

interface FinalizeStreamOptions {
  markPendingToolCallsAsError?: boolean;
}

interface AiStore {
  messages: AiMessage[];
  isLoading: boolean;
  error: string | null;
  activeStreamMessageId: string | null;
  configApplyPayloads: Record<string, ConfigApplyPayload>;
  addMessage: (message: AddMessageInput) => string;
  appendToMessage: (messageId: string, content: string) => void;
  upsertToolCall: (messageId: string, toolCall: AiToolCall) => void;
  setToolCallResult: (messageId: string, toolCallId: string, result: unknown) => void;
  finalizeStream: (messageId: string, options?: FinalizeStreamOptions) => void;
  removeMessage: (messageId: string) => void;
  setToolCallStatus: (toolCallId: string, status: AiToolCallStatus) => void;
  setConfigConfirmationBatchStatus: (
    confirmationBatchId: string,
    status: AiToolCallStatus,
  ) => void;
  setConfigApplyPayload: (
    confirmationBatchId: string,
    payload: ConfigApplyPayload,
  ) => void;
  getConfigApplyPayload: (confirmationBatchId: string) => ConfigApplyPayload | null;
  clearConfigApplyPayload: (confirmationBatchId: string) => void;
  setLoading: (isLoading: boolean) => void;
  setError: (error: string | null) => void;
  setActiveStreamMessageId: (messageId: string | null) => void;
  clearMessages: () => void;
}

function createId() {
  return crypto.randomUUID();
}

function omitKey<T extends Record<string, unknown>, K extends string>(
  obj: T,
  key: K,
): Omit<T, K> {
  const { [key]: _, ...rest } = obj;
  return rest as Omit<T, K>;
}

export const useAiStore = create<AiStore>()((set, get) => ({
  messages: [],
  isLoading: false,
  error: null,
  activeStreamMessageId: null,
  configApplyPayloads: {},
  addMessage: (message) => {
    const id = createId();
    set((state) => ({
      messages: [
        ...state.messages,
        {
          id,
          role: message.role,
          content: message.content,
          toolCalls: message.toolCalls ?? [],
          timestamp: Date.now(),
          isStreaming: message.isStreaming ?? false,
        },
      ],
    }));
    return id;
  },
  appendToMessage: (messageId, content) => {
    if (!content) {
      return;
    }

    set((state) => ({
      messages: state.messages.map((message) =>
        message.id === messageId
          ? { ...message, content: `${message.content}${content}` }
          : message,
      ),
    }));
  },
  upsertToolCall: (messageId, toolCall) => {
    set((state) => ({
      messages: state.messages.map((message) => {
        if (message.id !== messageId) {
          return message;
        }

        const toolCallIndex = message.toolCalls.findIndex((item) => item.id === toolCall.id);
        if (toolCallIndex === -1) {
          return { ...message, toolCalls: [...message.toolCalls, toolCall] };
        }

        const existingToolCall = message.toolCalls[toolCallIndex];
        if (existingToolCall === undefined) {
          return message;
        }

        const nextToolCalls = [...message.toolCalls];
        nextToolCalls[toolCallIndex] = { ...existingToolCall, ...toolCall };
        return { ...message, toolCalls: nextToolCalls };
      }),
    }));
  },
  setToolCallResult: (messageId, toolCallId, result) => {
    const nextStatus =
      isPendingConfigChangeResult(result) ||
      (isOptimizationToolResult(result) && result.status === "pending_confirmation")
        ? "awaiting_confirmation"
        : "completed";

    set((state) => ({
      messages: state.messages.map((message) => {
        if (message.id !== messageId) {
          return message;
        }

        const toolCallIndex = message.toolCalls.findIndex((item) => item.id === toolCallId);
        if (toolCallIndex === -1) {
          return {
            ...message,
            toolCalls: [
              ...message.toolCalls,
              {
                id: toolCallId,
                name: "tool",
                input: {},
                result,
                status: nextStatus,
              },
            ],
          };
        }

        const existingToolCall = message.toolCalls[toolCallIndex];
        if (existingToolCall === undefined) {
          return message;
        }

        const nextToolCalls = [...message.toolCalls];
        nextToolCalls[toolCallIndex] = {
          ...existingToolCall,
          result,
          status: nextStatus,
        };

        return { ...message, toolCalls: nextToolCalls };
      }),
    }));
  },
  finalizeStream: (messageId, options) => {
    set((state) => ({
      messages: state.messages.map((message) => {
        if (message.id !== messageId) {
          return message;
        }

        return {
          ...message,
          isStreaming: false,
          toolCalls:
            options?.markPendingToolCallsAsError === true
              ? message.toolCalls.map((toolCall) =>
                  toolCall.status === "pending" || toolCall.status === "executing"
                    ? { ...toolCall, status: "error" }
                    : toolCall,
                )
              : message.toolCalls,
        };
      }),
    }));
  },
  removeMessage: (messageId) => {
    set((state) => ({
      messages: state.messages.filter((message) => message.id !== messageId),
    }));
  },
  setToolCallStatus: (toolCallId, status) => {
    set((state) => ({
      messages: state.messages.map((message) => ({
        ...message,
        toolCalls: message.toolCalls.map((toolCall) =>
          toolCall.id === toolCallId ? { ...toolCall, status } : toolCall,
        ),
      })),
    }));
  },
  setConfigConfirmationBatchStatus: (confirmationBatchId, status) => {
    set((state) => ({
      configApplyPayloads:
        status === "applied" || status === "rejected"
          ? omitKey(state.configApplyPayloads, confirmationBatchId)
          : state.configApplyPayloads,
      messages: state.messages.map((message) => ({
        ...message,
        toolCalls: message.toolCalls.map((toolCall) =>
          isPendingConfigChangeResult(toolCall.result) &&
          toolCall.result.confirmationBatchId === confirmationBatchId
            ? { ...toolCall, status }
            : toolCall,
        ),
      })),
    }));
  },
  setConfigApplyPayload: (confirmationBatchId, payload) =>
    set((state) => ({
      configApplyPayloads: {
        ...state.configApplyPayloads,
        [confirmationBatchId]: payload,
      },
    })),
  getConfigApplyPayload: (confirmationBatchId) =>
    get().configApplyPayloads[confirmationBatchId] ?? null,
  clearConfigApplyPayload: (confirmationBatchId) =>
    set((state) => ({
      configApplyPayloads: omitKey(state.configApplyPayloads, confirmationBatchId),
    })),
  setLoading: (isLoading) =>
    set((state) => (state.isLoading === isLoading ? state : { isLoading })),
  setError: (error) =>
    set((state) => (state.error === error ? state : { error })),
  setActiveStreamMessageId: (activeStreamMessageId) =>
    set((state) =>
      state.activeStreamMessageId === activeStreamMessageId ? state : { activeStreamMessageId },
    ),
  clearMessages: () =>
    set({
      messages: [],
      isLoading: false,
      error: null,
      activeStreamMessageId: null,
      configApplyPayloads: {},
    }),
}));
