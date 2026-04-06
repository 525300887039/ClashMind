import type { ModelMessage } from "ai";

import { FEW_SHOT_EXAMPLES } from "./few-shots.js";
import { buildSystemPrompt } from "./system.js";
import type { ChatContext, ChatMessage } from "../types.js";

function toModelMessage(message: ChatMessage): ModelMessage {
  return {
    role: message.role,
    content: message.content,
  };
}

export function assemblePrompt(
  userMessages: ChatMessage[],
  context?: ChatContext,
): { system: string; messages: ModelMessage[] } {
  return {
    system: buildSystemPrompt(context),
    messages: [...FEW_SHOT_EXAMPLES, ...userMessages.map(toModelMessage)],
  };
}
