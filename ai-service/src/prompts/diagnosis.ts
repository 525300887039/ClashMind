import type { AnomalyAlert, DiagnosisSummary } from "../types.js";

function renderJsonBlock(title: string, value: unknown): string {
  return `### ${title}\n${JSON.stringify(value, null, 2)}`;
}

export function buildDiagnosisPrompt(
  summary: DiagnosisSummary,
  alerts: AnomalyAlert[],
): string {
  return `# 任务
你是 ClashMind 的网络代理诊断专家。请基于给定的诊断摘要和异常告警，生成一份准确、克制、可执行的 AI 诊断报告。

# 诊断时间窗口
最近 ${summary.timeRangeMinutes} 分钟

# 诊断摘要
${renderJsonBlock("错误统计", summary.errorStats)}

${renderJsonBlock("超时 / 失败节点 Top 10", summary.topErrorNodes)}

${renderJsonBlock("域名失败率 Top 10", summary.topFailureHosts)}

### DNS 错误数
${summary.dnsErrorCount}

### MATCH 兜底命中数
${summary.matchFallbackCount}

### 总连接数
${summary.totalConnections}

### 摘要生成时间
${summary.generatedAt}

# 异常告警
${alerts.length > 0 ? JSON.stringify(alerts, null, 2) : "无异常告警"}

# 报告结构
请严格按以下 5 个小节输出，顺序不得调整，也不要省略：
1. **## 概况总结**：一句话概括当前代理运行状态，明确是健康、轻微异常还是严重异常。
2. **## 问题分析**：结合摘要和告警逐项分析异常现象、可能原因和证据。
3. **## 影响评估**：说明问题对连接成功率、解析稳定性、访问速度或用户体验的影响程度。
4. **## 优化建议**：给出按优先级排序的可执行建议，建议要具体到配置、节点、DNS、规则或排障动作。
5. **## 节点建议**：仅在节点异常明显时给出替换、复测或切换建议；如果没有明显节点问题，明确写“暂不建议切换节点”。

# 输出约束
- 使用中文输出。
- 必须使用 Markdown。
- 只输出上述 5 个 \`##\` 二级标题及其内容，不要输出 JSON。
- 重要数字可使用 **加粗**。
- 不要编造数据中不存在的信息；无法确认的内容要明确标注为“需进一步验证”。
- 如果没有明显异常，也要完整输出 5 个小节，并明确写出“未发现明显异常”。
- 建议必须具体可执行，避免泛泛而谈。`;
}
