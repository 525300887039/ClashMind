import { tool } from "ai";
import { z } from "zod";

import { requestFromRust } from "./rust-rpc.js";

const emptyParameters = z.object({}).strict();

const recentErrorsParameters = z
  .object({
    minutes: z.number().int().min(1).max(1_440).default(30).describe("最近 N 分钟"),
  })
  .strict();

const ruleMatchStatsParameters = z
  .object({
    days: z.number().int().min(1).max(30).default(1).describe("统计天数"),
  })
  .strict();

export const diagnosisTools = {
  check_connectivity: tool({
    description: "检查当前 Mihomo API、代理和采集器的连通性摘要",
    inputSchema: emptyParameters,
    execute: async () => requestFromRust("check_connectivity"),
  }),

  get_recent_errors: tool({
    description: "获取最近的运行时错误或健康问题摘要",
    inputSchema: recentErrorsParameters,
    execute: async (params) => requestFromRust("get_recent_errors", params),
  }),

  get_rule_match_stats: tool({
    description: "获取规则匹配统计，便于判断 MATCH 兜底比例",
    inputSchema: ruleMatchStatsParameters,
    execute: async (params) => requestFromRust("get_rule_stats", params),
  }),
};
