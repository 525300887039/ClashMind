import { create } from "zustand";
import type { AiChatRole } from "@/lib/tauri-api";

export type AiToolCallStatus = "pending" | "executing" | "completed" | "error";

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
  addMessage: (message: AddMessageInput) => string;
  appendToMessage: (messageId: string, content: string) => void;
  upsertToolCall: (messageId: string, toolCall: AiToolCall) => void;
  setToolCallResult: (messageId: string, toolCallId: string, result: unknown) => void;
  finalizeStream: (messageId: string, options?: FinalizeStreamOptions) => void;
  removeMessage: (messageId: string) => void;
  setLoading: (isLoading: boolean) => void;
  setError: (error: string | null) => void;
  setActiveStreamMessageId: (messageId: string | null) => void;
  clearMessages: () => void;
}

function createId() {
  return crypto.randomUUID();
}

export const useAiStore = create<AiStore>()((set) => ({
  messages: [],
  isLoading: false,
  error: null,
  activeStreamMessageId: null,
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
                status: "completed",
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
          status: "completed",
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
  setLoading: (isLoading) => set({ isLoading }),
  setError: (error) => set({ error }),
  setActiveStreamMessageId: (activeStreamMessageId) => set({ activeStreamMessageId }),
  clearMessages: () =>
    set({
      messages: [],
      isLoading: false,
      error: null,
      activeStreamMessageId: null,
    }),
}));
