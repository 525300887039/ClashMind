import type { ReportStatsPayload, ReportType } from "../types.js";

function buildSharedConstraints(type: ReportType): string {
  const lengthHint = type === "daily" ? "300-500 字" : "500-800 字";

  return [
    "使用中文输出。",
    "必须使用 Markdown。",
    "一级标题不要使用 `#`，统一从 `##` 开始。",
    "正文使用简洁段落和 `-` 列表，不要输出 JSON。",
    "重要数字使用 **加粗**。",
    "禁止编造不存在的数据；如果样本不足，请明确说明。",
    `整体长度控制在 ${lengthHint}。`,
  ].join("\n");
}

function buildDailyPrompt(payload: ReportStatsPayload): string {
  return `# 任务
你是 ClashMind 的网络统计分析助手。请基于提供的日报统计数据，生成一份可读性强、可执行的每日 AI 统计报告。

# 报告周期
${payload.period.start} 至 ${payload.period.end}

# 统计数据
${JSON.stringify(payload.stats, null, 2)}

# 报告要求
1. 先写 **## 每日使用报告**，标题中带上报告日期。
2. 必须包含以下小节：
   - **## 流量概览**：总上传、总下载、总连接数、与前一日对比。
   - **## 热门域名**：概括 Top 域名流量分布。
   - **## 规则分析**：概括主要规则命中，并关注 MATCH 兜底率是否偏高。
   - **## 峰值时段**：指出最高流量时段。
   - **## 异常与建议**：如有流量突增、域名集中、MATCH 比例偏高等现象，请给出提醒和优化建议。
3. 如果没有明显异常，也要明确写出“未发现明显异常”。

# 输出约束
${buildSharedConstraints("daily")}`;
}

function buildWeeklyPrompt(payload: ReportStatsPayload): string {
  return `# 任务
你是 ClashMind 的网络统计分析助手。请基于提供的近 7 天统计数据，生成一份面向配置优化的 AI 周报。

# 报告周期
${payload.period.start} 至 ${payload.period.end}

# 统计数据
${JSON.stringify(payload.stats, null, 2)}

# 报告要求
1. 先写 **## 每周使用报告**，标题中带上报告周期。
2. 必须包含以下小节：
   - **## 本周期概览**：总上传、总下载、总连接数。
   - **## 趋势变化**：结合与上一周期对比数据，概括变化方向。
   - **## 热门域名**：总结本周期访问最集中的域名或服务类型。
   - **## 规则效率**：说明主要规则命中和 MATCH 兜底比例是否合理。
   - **## 周期节奏**：结合日级趋势解释哪几天更活跃。
   - **## 优化建议**：给出 2-3 条配置或策略层面的建议。
3. 如果统计数据很少，说明“本周期样本有限”，并避免过度解读。

# 输出约束
${buildSharedConstraints("weekly")}`;
}

export function buildReportPrompt(type: ReportType, payload: ReportStatsPayload): string {
  return type === "daily" ? buildDailyPrompt(payload) : buildWeeklyPrompt(payload);
}
