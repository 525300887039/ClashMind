import type { ToolSet } from "ai";

import { configTools } from "./config.tools.js";
import { diagnosisTools } from "./diagnosis.tools.js";
import { proxyTools } from "./proxy.tools.js";
import { statsTools } from "./stats.tools.js";

export const allTools = {
  ...configTools,
  ...proxyTools,
  ...statsTools,
  ...diagnosisTools,
} satisfies ToolSet;
