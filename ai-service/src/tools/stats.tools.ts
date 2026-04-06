import { tool } from "ai";
import { z } from "zod";

import { requestFromRust } from "./rust-rpc.js";

const trafficSummaryParameters = z
  .object({
    days: z.number().int().min(1).max(365).default(7).describe("统计天数"),
  })
  .strict();

const topDomainsParameters = z
  .object({
    days: z.number().int().min(1).max(365).default(7).describe("统计天数"),
    limit: z.number().int().min(1).max(100).default(20).describe("返回数量"),
  })
  .strict();

const trafficTrendParameters = z
  .object({
    granularity: z.enum(["hourly", "daily"]).describe("时间粒度"),
    days: z.number().int().min(1).max(365).default(7).describe("统计天数"),
  })
  .strict();

export const statsTools = {
  get_traffic_summary: tool({
    description: "获取指定时间窗口的流量摘要",
    inputSchema: trafficSummaryParameters,
    execute: async (params) => requestFromRust("get_stats_overview", params),
  }),

  get_top_domains: tool({
    description: "获取流量排名靠前的域名列表",
    inputSchema: topDomainsParameters,
    execute: async (params) => requestFromRust("get_domain_stats", params),
  }),

  get_traffic_trend: tool({
    description: "获取流量趋势数据",
    inputSchema: trafficTrendParameters,
    execute: async (params) => requestFromRust("get_traffic_trend", params),
  }),
};
