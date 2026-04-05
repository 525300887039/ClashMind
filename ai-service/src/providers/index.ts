import { createAnthropic } from "@ai-sdk/anthropic";
import { createDeepSeek } from "@ai-sdk/deepseek";
import { createOpenAI } from "@ai-sdk/openai";
import { createOllama, type OllamaProvider } from "ollama-ai-provider";
import type {
  CallWarning,
  FinishReason,
  LanguageModel,
  LanguageModelUsage,
  ProviderMetadata,
  ToolResultPart,
} from "ai";

import type { ProviderId, ProviderSettings } from "../types.js";

type AdaptedLanguageModel = Exclude<LanguageModel, string>;
type AdaptedCallOptions = Parameters<AdaptedLanguageModel["doGenerate"]>[0];
type AdaptedGenerateResult = Awaited<ReturnType<AdaptedLanguageModel["doGenerate"]>>;
type AdaptedStreamResult = Awaited<ReturnType<AdaptedLanguageModel["doStream"]>>;
type AdaptedPrompt = AdaptedCallOptions["prompt"];
type AdaptedPromptMessage = AdaptedPrompt[number];
type AdaptedGenerateContent = AdaptedGenerateResult["content"][number];
type AdaptedStreamPart = StreamPartFromReadable<AdaptedStreamResult["stream"]>;
type AdaptedWarning = CallWarning;
type AdaptedToolDefinition = NonNullable<AdaptedCallOptions["tools"]>[number];
type AdaptedUserMessage = Extract<AdaptedPromptMessage, { role: "user" }>;
type AdaptedAssistantMessage = Extract<AdaptedPromptMessage, { role: "assistant" }>;
type AdaptedToolMessage = Extract<AdaptedPromptMessage, { role: "tool" }>;
type AdaptedUserPart = Exclude<AdaptedUserMessage["content"], string>[number];
type AdaptedAssistantPart = Exclude<AdaptedAssistantMessage["content"], string>[number];
type AdaptedToolPart = AdaptedToolMessage["content"][number];
type AdaptedToolResultOutput = ToolResultPart["output"];
type AdaptedToolResultContentPart = Extract<AdaptedToolResultOutput, { type: "content" }>["value"][number];
type AdaptedProviderOptions = AdaptedCallOptions["providerOptions"];
type LegacyRegularMode = Extract<LegacyOllamaCallOptions["mode"], { type: "regular" }>;

type LegacyOllamaModel = ReturnType<OllamaProvider>;
type LegacyOllamaCallOptions = Parameters<LegacyOllamaModel["doGenerate"]>[0];
type LegacyOllamaGenerateResult = Awaited<ReturnType<LegacyOllamaModel["doGenerate"]>>;
type LegacyOllamaStreamResult = Awaited<ReturnType<LegacyOllamaModel["doStream"]>>;
type LegacyPromptMessage = LegacyOllamaCallOptions["prompt"][number];
type LegacyUserContentPart = Extract<LegacyPromptMessage, { role: "user" }>["content"][number];
type LegacyAssistantContentPart = Extract<LegacyPromptMessage, { role: "assistant" }>["content"][number];
type LegacyToolContentPart = Extract<LegacyPromptMessage, { role: "tool" }>["content"][number];
type StreamPartFromReadable<T extends ReadableStream<unknown>> =
  T extends ReadableStream<infer PART> ? PART : never;
type LegacyOllamaStreamPart = StreamPartFromReadable<LegacyOllamaStreamResult["stream"]>;

export const DEFAULT_MODELS = {
  openai: ["gpt-4o", "gpt-4o-mini", "gpt-4.1-mini", "o3-mini"],
  claude: ["claude-sonnet-4-5", "claude-haiku-4-5", "claude-opus-4-1"],
  deepseek: ["deepseek-chat", "deepseek-reasoner"],
  ollama: ["llama3.1", "qwen2.5", "mistral"],
} as const satisfies Record<ProviderId, readonly string[]>;

const DEFAULT_OLLAMA_BASE_URL = "http://127.0.0.1:11434/api";

function convertProviderOptions(
  options: AdaptedProviderOptions | undefined,
): LegacyOllamaCallOptions["providerMetadata"] {
  return options as LegacyOllamaCallOptions["providerMetadata"];
}

function convertProviderMetadata(
  metadata: LegacyOllamaGenerateResult["providerMetadata"],
): ProviderMetadata | undefined {
  return metadata as ProviderMetadata | undefined;
}

function convertWarningSetting(
  setting: "maxTokens" | keyof Omit<LegacyOllamaCallOptions, "mode" | "prompt" | "inputFormat">,
): Extract<AdaptedWarning, { type: "unsupported-setting" }>["setting"] {
  return setting === "maxTokens" ? "maxOutputTokens" : setting;
}

function convertWarnings(
  warnings: LegacyOllamaGenerateResult["warnings"] | undefined,
): AdaptedWarning[] {
  return (warnings ?? []).map((warning) =>
    warning.type === "unsupported-setting"
      ? {
          type: "unsupported-setting",
          setting: convertWarningSetting(warning.setting),
          details: warning.details,
        }
      : warning.type === "unsupported-tool"
        ? {
            type: "unsupported-tool",
            tool:
              warning.tool.type === "function"
                ? {
                    type: "function",
                    name: warning.tool.name,
                    description: warning.tool.description,
                    inputSchema: warning.tool.parameters,
                  }
                : {
                    type: "provider-defined",
                    id: warning.tool.id,
                    name: warning.tool.name,
                    args: warning.tool.args,
                  },
            details: warning.details,
          }
      : {
          type: "other",
          message: warning.message,
        },
  );
}

function convertUsage(usage: {
  promptTokens: number;
  completionTokens: number;
}): LanguageModelUsage {
  return {
    inputTokens: usage.promptTokens,
    outputTokens: usage.completionTokens,
    totalTokens: usage.promptTokens + usage.completionTokens,
  };
}

function convertFinishReason(
  reason: LegacyOllamaGenerateResult["finishReason"],
): FinishReason {
  return reason;
}

function convertToolResultOutput(output: AdaptedToolResultOutput): {
  result: unknown;
  isError?: boolean;
  content?: Array<{ type: "text"; text: string } | { type: "image"; data: string; mimeType?: string }>;
} {
  switch (output.type) {
    case "text":
      return { result: output.value };
    case "json":
      return { result: output.value };
    case "error-text":
      return { result: output.value, isError: true };
    case "error-json":
      return { result: output.value, isError: true };
    case "content":
      const content: NonNullable<ReturnType<typeof convertToolResultOutput>["content"]> = [];
      for (const part of output.value) {
        if (part.type === "text") {
          content.push({ type: "text", text: part.text });
        } else {
          content.push({ type: "image", data: part.data, mimeType: part.mediaType });
        }
      }

      return {
        result: output.value,
        content,
      };
    default:
      return { result: output };
  }
}

function convertFileData(data: string | Uint8Array | URL): string | URL {
  if (typeof data === "string" || data instanceof URL) {
    return data;
  }

  return Buffer.from(data).toString("base64");
}

function convertPrompt(prompt: AdaptedPrompt): LegacyOllamaCallOptions["prompt"] {
  return prompt.map((message: AdaptedPromptMessage) => {
    switch (message.role) {
      case "system":
        return {
          role: "system",
          content: message.content,
          providerMetadata: convertProviderOptions(message.providerOptions),
        };
      case "user":
        const userContent: LegacyUserContentPart[] =
          typeof message.content === "string"
            ? [{ type: "text", text: message.content }]
            : message.content.map((part: AdaptedUserPart) => {
                if (part.type === "text") {
                  return {
                    type: "text",
                    text: part.text,
                    providerMetadata: convertProviderOptions(part.providerOptions),
                  };
                }

                if (part.mediaType.startsWith("image/")) {
                  return {
                    type: "image",
                    image:
                      part.data instanceof URL || part.data instanceof Uint8Array
                        ? part.data
                        : Buffer.from(part.data, "base64"),
                    mimeType: part.mediaType,
                    providerMetadata: convertProviderOptions(part.providerOptions),
                  };
                }

                return {
                  type: "file",
                  filename: part.filename,
                  data: convertFileData(part.data),
                  mimeType: part.mediaType,
                  providerMetadata: convertProviderOptions(part.providerOptions),
                };
              });

        return {
          role: "user",
          content: userContent,
          providerMetadata: convertProviderOptions(message.providerOptions),
        };
      case "assistant":
        const assistantContent: LegacyAssistantContentPart[] =
          typeof message.content === "string"
            ? [{ type: "text", text: message.content }]
            : message.content.reduce<LegacyAssistantContentPart[]>(
                (content, part: AdaptedAssistantPart) => {
                  switch (part.type) {
                    case "text":
                      content.push({
                        type: "text",
                        text: part.text,
                        providerMetadata: convertProviderOptions(part.providerOptions),
                      });
                      break;
                    case "reasoning":
                      content.push({
                        type: "reasoning",
                        text: part.text,
                        providerMetadata: convertProviderOptions(part.providerOptions),
                      });
                      break;
                    case "file":
                      content.push({
                        type: "file",
                        filename: part.filename,
                        data: convertFileData(part.data),
                        mimeType: part.mediaType,
                        providerMetadata: convertProviderOptions(part.providerOptions),
                      });
                      break;
                    case "tool-call":
                      content.push({
                        type: "tool-call",
                        toolCallId: part.toolCallId,
                        toolName: part.toolName,
                        args: part.input,
                        providerMetadata: convertProviderOptions(part.providerOptions),
                      });
                      break;
                    case "tool-result": {
                      break;
                    }
                  }

                  return content;
                },
                [],
              );

        return {
          role: "assistant",
          content: assistantContent,
          providerMetadata: convertProviderOptions(message.providerOptions),
        };
      case "tool":
        const toolContent: LegacyToolContentPart[] = message.content.map((part: AdaptedToolPart) => {
          const toolResult = convertToolResultOutput(part.output);
          return {
            type: "tool-result",
            toolCallId: part.toolCallId,
            toolName: part.toolName,
            result: toolResult.result,
            isError: toolResult.isError,
            content: toolResult.content,
            providerMetadata: convertProviderOptions(part.providerOptions),
          };
        });

        return {
          role: "tool",
          content: toolContent,
          providerMetadata: convertProviderOptions(message.providerOptions),
        };
    }
  });
}

function convertContent(result: LegacyOllamaGenerateResult): AdaptedGenerateContent[] {
  const content: AdaptedGenerateContent[] = [];

  if (typeof result.text === "string" && result.text.length > 0) {
    content.push({ type: "text", text: result.text });
  }

  if (typeof result.reasoning === "string" && result.reasoning.length > 0) {
    content.push({ type: "reasoning", text: result.reasoning });
  } else if (Array.isArray(result.reasoning)) {
    for (const part of result.reasoning) {
      if (part.type === "text" && part.text.length > 0) {
        content.push({ type: "reasoning", text: part.text });
      }
    }
  }

  for (const file of result.files ?? []) {
    const generatedFile: Extract<AdaptedGenerateContent, { type: "file" }> = {
      type: "file",
      mediaType: file.mimeType,
      data: file.data,
    };
    content.push(generatedFile);
  }

  for (const source of result.sources ?? []) {
    content.push({
      type: "source",
      sourceType: source.sourceType,
      id: source.id,
      title: source.title,
      url: source.url,
      providerMetadata: convertProviderMetadata(source.providerMetadata),
    });
  }

  for (const toolCall of result.toolCalls ?? []) {
    content.push({
      type: "tool-call",
      toolCallId: toolCall.toolCallId,
      toolName: toolCall.toolName,
      input: toolCall.args,
    });
  }

  return content;
}
function createLegacyStreamMapper() {
  const state = {
    textId: undefined as string | undefined,
    reasoningId: undefined as string | undefined,
    openToolInputs: new Set<string>(),
  };

  return {
    map(part: LegacyOllamaStreamPart): AdaptedStreamPart[] {
      switch (part.type) {
        case "text-delta": {
          const streamId = state.textId ?? crypto.randomUUID();
          const events: AdaptedStreamPart[] = [];

          if (state.textId === undefined) {
            state.textId = streamId;
            events.push({ type: "text-start", id: streamId });
          }

          events.push({ type: "text-delta", id: streamId, delta: part.textDelta });
          return events;
        }
        case "reasoning": {
          const streamId = state.reasoningId ?? crypto.randomUUID();
          const events: AdaptedStreamPart[] = [];

          if (state.reasoningId === undefined) {
            state.reasoningId = streamId;
            events.push({ type: "reasoning-start", id: streamId });
          }

          events.push({ type: "reasoning-delta", id: streamId, delta: part.textDelta });
          return events;
        }
        case "tool-call-delta": {
          const events: AdaptedStreamPart[] = [];

          if (!state.openToolInputs.has(part.toolCallId)) {
            state.openToolInputs.add(part.toolCallId);
            events.push({
              type: "tool-input-start",
              id: part.toolCallId,
              toolName: part.toolName,
            });
          }

          events.push({
            type: "tool-input-delta",
            id: part.toolCallId,
            delta: part.argsTextDelta,
          });
          return events;
        }
        case "tool-call": {
          const events: AdaptedStreamPart[] = [];

          if (state.openToolInputs.has(part.toolCallId)) {
            state.openToolInputs.delete(part.toolCallId);
            events.push({ type: "tool-input-end", id: part.toolCallId });
          }

          events.push({
            type: "tool-call",
            toolCallId: part.toolCallId,
            toolName: part.toolName,
            input: part.args,
          });

          return events;
        }
        case "response-metadata":
          return [
            {
              type: "response-metadata",
              id: part.id,
              timestamp: part.timestamp,
              modelId: part.modelId,
            },
          ];
        case "source":
          return [
            {
              type: "source",
              sourceType: part.source.sourceType,
              id: part.source.id,
              title: part.source.title,
              url: part.source.url,
              providerMetadata: convertProviderMetadata(part.source.providerMetadata),
            },
          ];
        case "file":
          return [
            {
              type: "file",
              mediaType: part.mimeType,
              data: part.data,
            },
          ];
        case "finish": {
          const events: AdaptedStreamPart[] = [];

          if (state.textId !== undefined) {
            events.push({ type: "text-end", id: state.textId });
            state.textId = undefined;
          }

          if (state.reasoningId !== undefined) {
            events.push({ type: "reasoning-end", id: state.reasoningId });
            state.reasoningId = undefined;
          }

          for (const toolInputId of state.openToolInputs) {
            events.push({ type: "tool-input-end", id: toolInputId });
          }
          state.openToolInputs.clear();

          events.push({
            type: "finish",
            finishReason: convertFinishReason(part.finishReason),
            usage: convertUsage(part.usage),
            providerMetadata: convertProviderMetadata(part.providerMetadata),
          });

          return events;
        }
        case "error":
          return [{ type: "error", error: part.error }];
        case "reasoning-signature":
        case "redacted-reasoning":
          return [];
      }
    },
  };
}

function convertTools(
  tools: AdaptedCallOptions["tools"],
): LegacyRegularMode["tools"] {
  return tools?.map((tool: AdaptedToolDefinition) =>
    tool.type === "function"
      ? {
          type: "function",
          name: tool.name,
          description: tool.description,
          parameters: tool.inputSchema,
        }
      : {
          type: "provider-defined",
          id: tool.id,
          name: tool.name,
          args: tool.args,
        },
  );
}

function adaptOllamaModel(model: LegacyOllamaModel): AdaptedLanguageModel {
  return {
    specificationVersion: "v2",
    provider: model.provider,
    modelId: model.modelId,
    supportedUrls: {},
    async doGenerate(options: AdaptedCallOptions) {
      const result = await model.doGenerate({
        inputFormat: "messages",
        mode: {
          type: "regular",
          tools: convertTools(options.tools),
          toolChoice: options.toolChoice,
        },
        prompt: convertPrompt(options.prompt),
        providerMetadata: convertProviderOptions(options.providerOptions),
        maxTokens: options.maxOutputTokens,
        temperature: options.temperature,
        stopSequences: options.stopSequences,
        topP: options.topP,
        topK: options.topK,
        presencePenalty: options.presencePenalty,
        frequencyPenalty: options.frequencyPenalty,
        responseFormat: options.responseFormat,
        seed: options.seed,
        abortSignal: options.abortSignal,
        headers: options.headers,
      });

      return {
        content: convertContent(result),
        finishReason: convertFinishReason(result.finishReason),
        usage: convertUsage(result.usage),
        providerMetadata: convertProviderMetadata(result.providerMetadata),
        warnings: convertWarnings(result.warnings),
        request: result.request,
        response: result.response
          ? {
              ...result.response,
              headers: result.rawResponse?.headers,
              body: result.rawResponse?.body,
            }
          : result.rawResponse
            ? {
                headers: result.rawResponse.headers,
                body: result.rawResponse.body,
              }
            : undefined,
      };
    },
    async doStream(options: AdaptedCallOptions) {
      const result = await model.doStream({
        inputFormat: "messages",
        mode: {
          type: "regular",
          tools: convertTools(options.tools),
          toolChoice: options.toolChoice,
        },
        prompt: convertPrompt(options.prompt),
        providerMetadata: convertProviderOptions(options.providerOptions),
        maxTokens: options.maxOutputTokens,
        temperature: options.temperature,
        stopSequences: options.stopSequences,
        topP: options.topP,
        topK: options.topK,
        presencePenalty: options.presencePenalty,
        frequencyPenalty: options.frequencyPenalty,
        responseFormat: options.responseFormat,
        seed: options.seed,
        abortSignal: options.abortSignal,
        headers: options.headers,
      });

      const mapper = createLegacyStreamMapper();

      return {
        stream: result.stream.pipeThrough(
          new TransformStream<LegacyOllamaStreamPart, AdaptedStreamPart>({
            start(controller) {
              controller.enqueue({
                type: "stream-start",
                warnings: convertWarnings(result.warnings),
              });
            },
            transform(part, controller) {
              for (const mappedPart of mapper.map(part)) {
                controller.enqueue(mappedPart);
              }
            },
          }),
        ),
        request: result.request,
        response: result.rawResponse ? { headers: result.rawResponse.headers } : undefined,
      };
    },
  };
}

export function createModel(settings: ProviderSettings): LanguageModel {
  switch (settings.provider) {
    case "openai": {
      const openai = createOpenAI({
        apiKey: settings.apiKey,
        baseURL: settings.baseUrl,
      });
      return openai(settings.model);
    }
    case "claude": {
      const anthropic = createAnthropic({
        apiKey: settings.apiKey,
        baseURL: settings.baseUrl,
      });
      return anthropic(settings.model);
    }
    case "deepseek": {
      const deepseek = createDeepSeek({
        apiKey: settings.apiKey,
        baseURL: settings.baseUrl,
      });
      return deepseek(settings.model);
    }
    case "ollama": {
      const ollama = createOllama({
        baseURL: settings.baseUrl ?? DEFAULT_OLLAMA_BASE_URL,
      });

      return adaptOllamaModel(ollama(settings.model));
    }
  }
}
