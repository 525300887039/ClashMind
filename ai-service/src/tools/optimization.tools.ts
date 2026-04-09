import { tool } from "ai";
import { z } from "zod";

import { pendingConfirmation, requestFromRust } from "./rust-rpc.js";

const MAX_OPTIMIZATION_SUGGESTIONS = 3;

const healthGradeSchema = z.enum(["excellent", "good", "fair", "poor", "critical"]);

const nodeHealthScoreSchema = z
  .object({
    nodeName: z.string().min(1),
    score: z.number().finite(),
    grade: healthGradeSchema,
    successRate: z.number().finite(),
    avgDelayMs: z.number().finite().nullable(),
    totalTests: z.number().int().nonnegative(),
    evaluatedAt: z.string().min(1),
  })
  .strict();

const optimizationContextSchema = z
  .object({
    group: z.string().min(1),
    currentNode: z.string().nullable(),
    nodes: z.array(z.string().min(1)),
    healthScores: z.array(nodeHealthScoreSchema),
  })
  .strict();

const suggestOptimizationParameters = z
  .object({
    group: z.string().min(1).describe("要分析的代理组名称"),
    reason: z.string().min(1).describe("优化原因说明"),
  })
  .strict();

const executeSwitchParameters = z
  .object({
    group: z.string().min(1).describe("代理组名称"),
    name: z.string().min(1).describe("目标节点名称"),
    reason: z.string().min(1).describe("切换原因"),
  })
  .strict();

type HealthGrade = z.infer<typeof healthGradeSchema>;
type NodeHealthScore = z.infer<typeof nodeHealthScoreSchema>;
type OptimizationContext = z.infer<typeof optimizationContextSchema>;
type SuggestOptimizationParameters = z.infer<typeof suggestOptimizationParameters>;
type ExecuteSwitchParameters = z.infer<typeof executeSwitchParameters>;

interface OptimizationSuggestion {
  group: string;
  currentNode: string | null;
  targetNode: string;
  reason: string;
  currentScore: number | null;
  currentGrade: HealthGrade | null;
  targetScore: number | null;
  targetGrade: HealthGrade | null;
  scoreDelta: number | null;
}

interface OptimizationToolResult {
  action: "suggest_optimization" | "switch_proxy";
  params: {
    group: string;
    reason?: string;
    name?: string;
  };
  status: "pending_confirmation" | "completed";
  context: OptimizationContext;
  suggestions: OptimizationSuggestion[];
  message: string;
}

function normalizeScore(score: number | null | undefined): number | null {
  if (score === null || score === undefined) {
    return null;
  }

  return Number(score.toFixed(1));
}

function isRecommendedGrade(grade: HealthGrade): boolean {
  return grade === "excellent" || grade === "good";
}

function compareHealthScores(left: NodeHealthScore, right: NodeHealthScore): number {
  if (left.score !== right.score) {
    return right.score - left.score;
  }

  if (left.successRate !== right.successRate) {
    return right.successRate - left.successRate;
  }

  if (left.avgDelayMs === null && right.avgDelayMs !== null) {
    return 1;
  }

  if (left.avgDelayMs !== null && right.avgDelayMs === null) {
    return -1;
  }

  if (left.avgDelayMs !== null && right.avgDelayMs !== null && left.avgDelayMs !== right.avgDelayMs) {
    return left.avgDelayMs - right.avgDelayMs;
  }

  return left.nodeName.localeCompare(right.nodeName, "zh-CN");
}

function findHealthScore(
  context: OptimizationContext,
  nodeName: string | null,
): NodeHealthScore | null {
  if (nodeName === null) {
    return null;
  }

  return context.healthScores.find((score) => score.nodeName === nodeName) ?? null;
}

function buildSuggestionReason(
  baseReason: string,
  targetNode: string,
  targetScore: number | null,
  currentNode: string | null,
  currentScore: number | null,
): string {
  if (targetScore === null) {
    return `${baseReason}；建议切换到「${targetNode}」，但该节点当前暂无健康评分。`;
  }

  if (currentNode === null || currentScore === null) {
    return `${baseReason}；建议选择「${targetNode}」，当前可用健康评分为 ${targetScore.toFixed(1)} 分。`;
  }

  if (targetScore < currentScore) {
    return `${baseReason}；目标节点「${targetNode}」当前评分为 ${targetScore.toFixed(1)}，低于「${currentNode}」的 ${currentScore.toFixed(1)}，请谨慎确认。`;
  }

  if (targetScore === currentScore) {
    return `${baseReason}；切换到「${targetNode}」后健康评分预计维持在 ${targetScore.toFixed(1)} 分。`;
  }

  return `${baseReason}；建议从「${currentNode}」切换到「${targetNode}」，健康评分预计从 ${currentScore.toFixed(1)} 提升到 ${targetScore.toFixed(1)}。`;
}

function buildSuggestion(
  context: OptimizationContext,
  targetNode: string,
  reason: string,
): OptimizationSuggestion {
  const currentHealth = findHealthScore(context, context.currentNode);
  const targetHealth = findHealthScore(context, targetNode);
  const currentScore = normalizeScore(currentHealth?.score);
  const targetScore = normalizeScore(targetHealth?.score);

  return {
    group: context.group,
    currentNode: context.currentNode,
    targetNode,
    reason: buildSuggestionReason(
      reason,
      targetNode,
      targetScore,
      context.currentNode,
      currentScore,
    ),
    currentScore,
    currentGrade: currentHealth?.grade ?? null,
    targetScore,
    targetGrade: targetHealth?.grade ?? null,
    scoreDelta:
      currentScore !== null && targetScore !== null
        ? normalizeScore(targetScore - currentScore)
        : null,
  };
}

function selectSuggestions(
  context: OptimizationContext,
  reason: string,
): OptimizationSuggestion[] {
  const currentHealth = findHealthScore(context, context.currentNode);

  return context.healthScores
    .filter((score) => score.nodeName !== context.currentNode)
    .filter((score) => isRecommendedGrade(score.grade))
    .filter((score) => currentHealth === null || score.score > currentHealth.score)
    .sort(compareHealthScores)
    .slice(0, MAX_OPTIMIZATION_SUGGESTIONS)
    .map((score) => buildSuggestion(context, score.nodeName, reason));
}

function buildSuggestionMessage(
  context: OptimizationContext,
  suggestions: OptimizationSuggestion[],
): string {
  if (context.nodes.length === 0) {
    return `未找到代理组「${context.group}」，或该组当前没有可切换节点。`;
  }

  if (suggestions.length === 0) {
    return `代理组「${context.group}」当前未发现评分达到 Good 且优于当前节点的候选节点。`;
  }

  if (context.currentNode === null) {
    return `代理组「${context.group}」当前没有已选节点，已按健康评分生成 ${suggestions.length} 条切换建议。`;
  }

  return `已基于节点健康评分为代理组「${context.group}」生成 ${suggestions.length} 条切换建议。`;
}

async function getOptimizationContext(group: string): Promise<OptimizationContext> {
  const response = await requestFromRust("get_optimization_context", { group });
  return optimizationContextSchema.parse(response);
}

function buildExecuteSwitchMessage(
  group: string,
  name: string,
  suggestion: OptimizationSuggestion,
): string {
  if (suggestion.targetScore === null) {
    return `已生成代理组「${group}」切换到「${name}」的待确认操作，但目标节点暂无健康评分。`;
  }

  return `已生成代理组「${group}」切换到「${name}」的待确认操作，请确认后执行。`;
}

function ensureNodeBelongsToGroup(
  context: OptimizationContext,
  nodeName: string,
): void {
  if (!context.nodes.includes(nodeName)) {
    throw new Error(`节点「${nodeName}」不属于代理组「${context.group}」，拒绝生成切换操作。`);
  }
}

function resolveValidatedSwitchSuggestion(
  context: OptimizationContext,
  params: ExecuteSwitchParameters,
): OptimizationSuggestion {
  ensureNodeBelongsToGroup(context, params.name);

  const validatedSuggestion = selectSuggestions(context, params.reason).find(
    (suggestion) => suggestion.targetNode === params.name,
  );

  if (validatedSuggestion !== undefined) {
    return validatedSuggestion;
  }

  if (context.currentNode === params.name) {
    throw new Error(`节点「${params.name}」已经是代理组「${params.group}」当前节点，无需切换。`);
  }

  const targetHealth = findHealthScore(context, params.name);
  if (targetHealth === null) {
    throw new Error(`节点「${params.name}」缺少健康评分，拒绝生成切换操作。`);
  }

  if (!isRecommendedGrade(targetHealth.grade)) {
    throw new Error(
      `节点「${params.name}」当前健康等级为 ${targetHealth.grade}，未达到 Good 及以上要求，拒绝生成切换操作。`,
    );
  }

  const currentHealth = findHealthScore(context, context.currentNode);
  if (currentHealth !== null && targetHealth.score <= currentHealth.score) {
    throw new Error(
      `节点「${params.name}」评分未高于当前节点「${context.currentNode ?? "未选中"}」，拒绝生成切换操作。`,
    );
  }

  throw new Error(`节点「${params.name}」未通过优化安全约束，拒绝生成切换操作。`);
}

export const optimizationTools = {
  suggest_optimization: tool({
    description: `分析代理组健康数据并生成优化建议。
安全约束：
- 仅限建议 switch_proxy 操作（切换代理组中的节点）
- 每次最多返回 3 项建议
- 所有建议都必须等待用户确认
- 不修改配置文件，不删除节点，不调整规则`,
    inputSchema: suggestOptimizationParameters,
    execute: async (params: SuggestOptimizationParameters) => {
      const context = await getOptimizationContext(params.group);
      const suggestions = selectSuggestions(context, params.reason);

      return {
        action: "suggest_optimization",
        params,
        status: suggestions.length > 0 ? "pending_confirmation" : "completed",
        context,
        suggestions,
        message: buildSuggestionMessage(context, suggestions),
      } satisfies OptimizationToolResult;
    },
  }),

  execute_switch: tool({
    description: "生成代理组切换的待确认操作，仅允许对当前组内、已验证且 Good 以上的节点执行 switch_proxy",
    inputSchema: executeSwitchParameters,
    execute: async (params: ExecuteSwitchParameters) => {
      const context = await getOptimizationContext(params.group);
      const suggestion = resolveValidatedSwitchSuggestion(context, params);
      const pendingResult = pendingConfirmation("switch_proxy", {
        group: params.group,
        name: params.name,
      });

      return {
        ...pendingResult,
        params: {
          ...pendingResult.params,
          reason: params.reason,
        },
        context,
        suggestions: [suggestion],
        message: buildExecuteSwitchMessage(params.group, params.name, suggestion),
      } satisfies OptimizationToolResult;
    },
  }),
};
