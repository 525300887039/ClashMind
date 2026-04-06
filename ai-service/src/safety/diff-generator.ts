import yaml from "js-yaml";
import { z } from "zod";

export interface DiffChange {
  type: "add" | "remove" | "modify";
  path: string;
  description: string;
}

export interface ConfigDiff {
  original: string;
  modified: string;
  unifiedDiff: string;
  summary: string;
  changes: DiffChange[];
}

const PROXY_GROUP_TYPE_LABELS = {
  select: "手动选择",
  "url-test": "自动测速",
  fallback: "故障转移",
  "load-balance": "负载均衡",
} as const;

const PROXY_GROUP_FIELD_LABELS: Record<string, string> = {
  type: "类型",
  proxies: "节点列表",
  filter: "筛选条件",
  url: "探测地址",
  interval: "探测间隔",
  tolerance: "延迟容差",
  lazy: "懒检查",
  hidden: "隐藏状态",
};

const DNS_FIELD_LABELS: Record<string, string> = {
  nameserver: "上游 DNS",
  fallback: "后备 DNS",
  "fake-ip-filter": "Fake-IP 过滤列表",
  "enhanced-mode": "增强模式",
};

const MODE_LABELS = {
  rule: "规则模式",
  global: "全局模式",
  direct: "直连模式",
} as const;

const addProxyParamsSchema = z
  .object({
    name: z.string().min(1),
    type: z.enum(["ss", "vmess", "vless", "trojan", "hysteria2", "tuic"]),
    server: z.string().min(1),
    port: z.number().int().min(1).max(65535),
    settings: z.record(z.string(), z.unknown()).optional(),
  })
  .strict();

const removeProxyParamsSchema = z
  .object({
    name: z.string().min(1),
  })
  .strict();

const addProxyGroupParamsSchema = z
  .object({
    name: z.string().min(1),
    type: z.enum(["select", "url-test", "fallback", "load-balance"]),
    proxies: z.array(z.string().min(1)).optional(),
    filter: z.string().min(1).optional(),
    url: z.string().min(1).optional(),
    interval: z.number().int().positive().optional(),
  })
  .strict();

const updateProxyGroupParamsSchema = z
  .object({
    name: z.string().min(1),
    updates: z.record(z.string(), z.unknown()),
  })
  .strict();

const addRuleParamsSchema = z
  .object({
    type: z.enum([
      "DOMAIN",
      "DOMAIN-SUFFIX",
      "DOMAIN-KEYWORD",
      "IP-CIDR",
      "GEOIP",
      "PROCESS-NAME",
      "MATCH",
    ]),
    value: z.string().min(1).optional(),
    policy: z.string().min(1),
    position: z.enum(["prepend", "append"]).default("prepend"),
  })
  .strict();

const removeRuleParamsSchema = z
  .object({
    pattern: z.string().min(1),
  })
  .strict();

const updateDnsParamsSchema = z
  .object({
    nameserver: z.array(z.string().min(1)).optional(),
    fallback: z.array(z.string().min(1)).optional(),
    fakeIpFilter: z.array(z.string().min(1)).optional(),
    enhancedMode: z.enum(["fake-ip", "redir-host"]).optional(),
  })
  .strict();

const setModeParamsSchema = z
  .object({
    mode: z.enum(["rule", "global", "direct"]),
  })
  .strict();

const pendingConfigChangeSchema = z.discriminatedUnion("action", [
  z
    .object({
      action: z.literal("add_proxy"),
      params: addProxyParamsSchema,
      status: z.literal("pending_confirmation"),
    })
    .strict(),
  z
    .object({
      action: z.literal("remove_proxy"),
      params: removeProxyParamsSchema,
      status: z.literal("pending_confirmation"),
    })
    .strict(),
  z
    .object({
      action: z.literal("add_proxy_group"),
      params: addProxyGroupParamsSchema,
      status: z.literal("pending_confirmation"),
    })
    .strict(),
  z
    .object({
      action: z.literal("update_proxy_group"),
      params: updateProxyGroupParamsSchema,
      status: z.literal("pending_confirmation"),
    })
    .strict(),
  z
    .object({
      action: z.literal("add_rule"),
      params: addRuleParamsSchema,
      status: z.literal("pending_confirmation"),
    })
    .strict(),
  z
    .object({
      action: z.literal("remove_rule"),
      params: removeRuleParamsSchema,
      status: z.literal("pending_confirmation"),
    })
    .strict(),
  z
    .object({
      action: z.literal("update_dns"),
      params: updateDnsParamsSchema,
      status: z.literal("pending_confirmation"),
    })
    .strict(),
  z
    .object({
      action: z.literal("set_mode"),
      params: setModeParamsSchema,
      status: z.literal("pending_confirmation"),
    })
    .strict(),
]);

type PendingConfigChange = z.infer<typeof pendingConfigChangeSchema>;
type ConfigDocument = Record<string, unknown>;
type DiffOpType = "context" | "add" | "remove";

interface DiffOp {
  type: DiffOpType;
  line: string;
}

interface AnnotatedDiffLine {
  type: DiffOpType;
  line: string;
  oldLine: number | null;
  newLine: number | null;
}

const DIFF_CONTEXT_LINES = 3;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function normalizeYamlText(value: string): string {
  return value.replace(/\r\n/g, "\n");
}

function splitIntoLines(value: string): string[] {
  const normalized = normalizeYamlText(value);
  if (normalized.length === 0) {
    return [];
  }

  const lines = normalized.split("\n");
  if (lines.at(-1) === "") {
    lines.pop();
  }

  return lines;
}

function loadYamlDocument(originalYaml: string): ConfigDocument {
  const parsed = yaml.load(originalYaml);
  return loadConfigDocument(parsed);
}

function loadConfigDocument(parsed: unknown): ConfigDocument {
  if (parsed === undefined || parsed === null) {
    return {};
  }

  if (!isRecord(parsed)) {
    throw new Error("当前配置根节点必须是 YAML 对象");
  }

  return parsed;
}

function dumpYamlDocument(document: ConfigDocument): string {
  return yaml.dump(document, {
    lineWidth: -1,
    noArrayIndent: false,
    noCompatMode: true,
    noRefs: true,
    sortKeys: false,
  });
}

function ensureObject(value: unknown, path: string): Record<string, unknown> {
  if (!isRecord(value)) {
    throw new Error(`${path} 必须是对象`);
  }

  return value;
}

function ensureStringArray(value: unknown, path: string): string[] {
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) {
    throw new Error(`${path} 必须是字符串数组`);
  }

  return value;
}

function getOrCreateObject(
  container: ConfigDocument,
  key: string,
): Record<string, unknown> {
  const existing = container[key];
  if (existing === undefined) {
    const nextValue: Record<string, unknown> = {};
    container[key] = nextValue;
    return nextValue;
  }

  return ensureObject(existing, key);
}

function getOrCreateArray(
  container: ConfigDocument,
  key: string,
): unknown[] {
  const existing = container[key];
  if (existing === undefined) {
    const nextValue: unknown[] = [];
    container[key] = nextValue;
    return nextValue;
  }

  if (!Array.isArray(existing)) {
    throw new Error(`${key} 必须是数组`);
  }

  return existing;
}

function getArray(container: ConfigDocument, key: string): unknown[] {
  const existing = container[key];
  if (existing === undefined) {
    return [];
  }

  if (!Array.isArray(existing)) {
    throw new Error(`${key} 必须是数组`);
  }

  return existing;
}

function buildRuleString(params: z.infer<typeof addRuleParamsSchema>): string {
  if (params.type === "MATCH") {
    return `MATCH,${params.policy}`;
  }

  if (params.value === undefined) {
    throw new Error(`规则类型 ${params.type} 需要提供 value`);
  }

  return `${params.type},${params.value},${params.policy}`;
}

function formatListWithQuotes(values: string[], labels?: Record<string, string>): string {
  return values
    .map((value) => `「${labels?.[value] ?? value}」`)
    .join("、");
}

function formatProxyGroupType(
  type: z.infer<typeof addProxyGroupParamsSchema>["type"],
): string {
  return PROXY_GROUP_TYPE_LABELS[type];
}

function assertMatchRuleAtTail(rules: string[]): void {
  if (!rules.some((rule) => rule.startsWith("MATCH,"))) {
    return;
  }

  const lastRule = rules.at(-1);
  if (lastRule === undefined || !lastRule.startsWith("MATCH,")) {
    throw new Error("rules 最后一条必须保留 MATCH 兜底规则");
  }
}

function findProxyIndex(document: ConfigDocument, proxyName: string): number {
  const proxies = getArray(document, "proxies");
  return proxies.findIndex((item) => isRecord(item) && item.name === proxyName);
}

function findProxyGroupIndex(document: ConfigDocument, groupName: string): number {
  const proxyGroups = getArray(document, "proxy-groups");
  return proxyGroups.findIndex((item) => isRecord(item) && item.name === groupName);
}

function applyPendingChange(
  document: ConfigDocument,
  change: PendingConfigChange,
  changes: DiffChange[],
): void {
  switch (change.action) {
    case "add_proxy": {
      const proxies = getOrCreateArray(document, "proxies");
      if (findProxyIndex(document, change.params.name) !== -1) {
        throw new Error(`代理节点「${change.params.name}」已存在`);
      }

      proxies.push({
        name: change.params.name,
        type: change.params.type,
        server: change.params.server,
        port: change.params.port,
        ...(change.params.settings ?? {}),
      });

      changes.push({
        type: "add",
        path: `proxies[${change.params.name}]`,
        description: `新增代理节点「${change.params.name}」，协议为 ${change.params.type}，地址为 ${change.params.server}:${change.params.port}`,
      });
      return;
    }
    case "remove_proxy": {
      const proxies = getOrCreateArray(document, "proxies");
      const proxyIndex = findProxyIndex(document, change.params.name);
      if (proxyIndex === -1) {
        throw new Error(`未找到代理节点「${change.params.name}」`);
      }

      const proxyGroups = getArray(document, "proxy-groups");
      const referencedBy = proxyGroups.find((item) => {
        if (!isRecord(item) || typeof item.name !== "string") {
          return false;
        }

        const groupProxies = item.proxies;
        return Array.isArray(groupProxies) && groupProxies.includes(change.params.name);
      });

      if (isRecord(referencedBy) && typeof referencedBy.name === "string") {
        throw new Error(
          `代理节点「${change.params.name}」仍被代理组「${referencedBy.name}」引用，不能直接删除`,
        );
      }

      proxies.splice(proxyIndex, 1);
      changes.push({
        type: "remove",
        path: `proxies[${change.params.name}]`,
        description: `删除代理节点「${change.params.name}」`,
      });
      return;
    }
    case "add_proxy_group": {
      const proxyGroups = getOrCreateArray(document, "proxy-groups");
      if (findProxyGroupIndex(document, change.params.name) !== -1) {
        throw new Error(`代理组「${change.params.name}」已存在`);
      }

      const nextGroup: Record<string, unknown> = {
        name: change.params.name,
        type: change.params.type,
      };

      if (change.params.proxies !== undefined) {
        nextGroup.proxies = [...change.params.proxies];
      }

      if (change.params.filter !== undefined) {
        nextGroup.filter = change.params.filter;
      }

      if (change.params.type !== "select") {
        nextGroup.url = change.params.url ?? "http://www.gstatic.com/generate_204";
        nextGroup.interval = change.params.interval ?? 300;
      }

      proxyGroups.push(nextGroup);

      changes.push({
        type: "add",
        path: `proxy-groups[${change.params.name}]`,
        description: `新增代理组「${change.params.name}」，类型为「${formatProxyGroupType(change.params.type)}」`,
      });
      return;
    }
    case "update_proxy_group": {
      const proxyGroups = getOrCreateArray(document, "proxy-groups");
      const proxyGroupIndex = findProxyGroupIndex(document, change.params.name);
      if (proxyGroupIndex === -1) {
        throw new Error(`未找到代理组「${change.params.name}」`);
      }

      const existing = proxyGroups[proxyGroupIndex];
      const nextValue = {
        ...ensureObject(existing, `proxy-groups[${change.params.name}]`),
        ...change.params.updates,
      };
      proxyGroups[proxyGroupIndex] = nextValue;

      const updateFields = Object.keys(change.params.updates);
      changes.push({
        type: "modify",
        path: `proxy-groups[${change.params.name}]`,
        description:
          updateFields.length === 0
            ? `更新代理组「${change.params.name}」`
            : `更新代理组「${change.params.name}」的${formatListWithQuotes(
                updateFields,
                PROXY_GROUP_FIELD_LABELS,
              )}`,
      });
      return;
    }
    case "add_rule": {
      const rules = getOrCreateArray(document, "rules");
      const nextRule = buildRuleString(change.params);

      const normalizedRules = ensureStringArray(rules, "rules");
      if (normalizedRules.some((item) => item === nextRule)) {
        throw new Error(`规则「${nextRule}」已存在`);
      }

      assertMatchRuleAtTail(normalizedRules);
      const matchIndex = normalizedRules.findIndex((rule) => rule.startsWith("MATCH,"));

      if (change.params.type === "MATCH") {
        if (matchIndex === -1) {
          rules.push(nextRule);
          changes.push({
            type: "add",
            path: "rules[MATCH]",
            description: `新增兜底规则，默认策略切换为「${change.params.policy}」`,
          });
          return;
        }

        const lastRuleIndex = normalizedRules.length - 1;
        rules[lastRuleIndex] = nextRule;
        changes.push({
          type: "modify",
          path: "rules[MATCH]",
          description: `更新兜底规则，默认策略切换为「${change.params.policy}」`,
        });
        return;
      }

      if (change.params.position === "prepend") {
        rules.splice(0, 0, nextRule);
      } else {
        rules.splice(matchIndex === -1 ? rules.length : matchIndex, 0, nextRule);
      }

      changes.push({
        type: "add",
        path: `rules[${change.params.type}]`,
        description: `新增规则「${change.params.type}${change.params.value === undefined ? "" : ` ${change.params.value}`}」，策略指向「${change.params.policy}」`,
      });
      return;
    }
    case "remove_rule": {
      const rules = getArray(document, "rules");
      const normalizedRules = ensureStringArray(rules, "rules");
      const ruleIndex = normalizedRules.findIndex((rule) => rule.includes(change.params.pattern));

      if (ruleIndex === -1) {
        throw new Error(`未找到匹配「${change.params.pattern}」的规则`);
      }

      const targetRule = normalizedRules[ruleIndex];
      if (targetRule?.startsWith("MATCH,")) {
        throw new Error("不能删除 MATCH 兜底规则");
      }

      rules.splice(ruleIndex, 1);
      assertMatchRuleAtTail(ensureStringArray(rules, "rules"));

      changes.push({
        type: "remove",
        path: `rules[${change.params.pattern}]`,
        description: `删除匹配「${change.params.pattern}」的规则`,
      });
      return;
    }
    case "update_dns": {
      const dns = getOrCreateObject(document, "dns");
      const changedFields: string[] = [];

      if (change.params.nameserver !== undefined) {
        dns.nameserver = [...change.params.nameserver];
        changedFields.push("nameserver");
      }

      if (change.params.fallback !== undefined) {
        dns.fallback = [...change.params.fallback];
        changedFields.push("fallback");
      }

      if (change.params.fakeIpFilter !== undefined) {
        dns["fake-ip-filter"] = [...change.params.fakeIpFilter];
        changedFields.push("fake-ip-filter");
      }

      if (change.params.enhancedMode !== undefined) {
        dns["enhanced-mode"] = change.params.enhancedMode;
        changedFields.push("enhanced-mode");
      }

      changes.push({
        type: "modify",
        path: "dns",
        description:
          changedFields.length === 0
            ? "更新 DNS 配置"
            : `更新 DNS 配置：${formatListWithQuotes(changedFields, DNS_FIELD_LABELS)}`,
      });
      return;
    }
    case "set_mode": {
      document.mode = change.params.mode;
      changes.push({
        type: "modify",
        path: "mode",
        description: `将运行模式切换为「${MODE_LABELS[change.params.mode]}」`,
      });
      return;
    }
    default: {
      const exhaustiveChange: never = change;
      throw new Error(`未支持的配置变更动作: ${String(exhaustiveChange)}`);
    }
  }
}

function buildDiffOperations(
  originalLines: string[],
  modifiedLines: string[],
): DiffOp[] {
  const rowCount = originalLines.length + 1;
  const columnCount = modifiedLines.length + 1;
  const dp = Array.from({ length: rowCount }, () => Array<number>(columnCount).fill(0));

  for (let row = originalLines.length - 1; row >= 0; row -= 1) {
    for (let column = modifiedLines.length - 1; column >= 0; column -= 1) {
      dp[row]![column] =
        originalLines[row] === modifiedLines[column]
          ? (dp[row + 1]![column + 1] ?? 0) + 1
          : Math.max(dp[row + 1]![column] ?? 0, dp[row]![column + 1] ?? 0);
    }
  }

  const operations: DiffOp[] = [];
  let originalIndex = 0;
  let modifiedIndex = 0;

  while (originalIndex < originalLines.length && modifiedIndex < modifiedLines.length) {
    if (originalLines[originalIndex] === modifiedLines[modifiedIndex]) {
      operations.push({
        type: "context",
        line: originalLines[originalIndex] ?? "",
      });
      originalIndex += 1;
      modifiedIndex += 1;
      continue;
    }

    if ((dp[originalIndex + 1]![modifiedIndex] ?? 0) >= (dp[originalIndex]![modifiedIndex + 1] ?? 0)) {
      operations.push({
        type: "remove",
        line: originalLines[originalIndex] ?? "",
      });
      originalIndex += 1;
      continue;
    }

    operations.push({
      type: "add",
      line: modifiedLines[modifiedIndex] ?? "",
    });
    modifiedIndex += 1;
  }

  while (originalIndex < originalLines.length) {
    operations.push({
      type: "remove",
      line: originalLines[originalIndex] ?? "",
    });
    originalIndex += 1;
  }

  while (modifiedIndex < modifiedLines.length) {
    operations.push({
      type: "add",
      line: modifiedLines[modifiedIndex] ?? "",
    });
    modifiedIndex += 1;
  }

  return operations;
}

function annotateOperations(operations: DiffOp[]): AnnotatedDiffLine[] {
  let oldLine = 1;
  let newLine = 1;

  return operations.map((operation) => {
    if (operation.type === "context") {
      const annotated = {
        type: "context" as const,
        line: operation.line,
        oldLine,
        newLine,
      };
      oldLine += 1;
      newLine += 1;
      return annotated;
    }

    if (operation.type === "remove") {
      const annotated = {
        type: "remove" as const,
        line: operation.line,
        oldLine,
        newLine: null,
      };
      oldLine += 1;
      return annotated;
    }

    const annotated = {
      type: "add" as const,
      line: operation.line,
      oldLine: null,
      newLine,
    };
    newLine += 1;
    return annotated;
  });
}

function buildUnifiedDiff(originalYaml: string, modifiedYaml: string): string {
  const originalLines = splitIntoLines(originalYaml);
  const modifiedLines = splitIntoLines(modifiedYaml);
  const operations = annotateOperations(buildDiffOperations(originalLines, modifiedLines));
  const changedIndexes = operations
    .map((operation, index) => (operation.type === "context" ? -1 : index))
    .filter((index) => index >= 0);

  const outputLines = ["--- current/config.yaml", "+++ proposed/config.yaml"];

  if (changedIndexes.length === 0) {
    outputLines.push("@@ -0,0 +0,0 @@");
    return outputLines.join("\n");
  }

  const hunks: Array<{ start: number; end: number }> = [];

  for (const changedIndex of changedIndexes) {
    const nextHunk = {
      start: Math.max(changedIndex - DIFF_CONTEXT_LINES, 0),
      end: Math.min(changedIndex + DIFF_CONTEXT_LINES, operations.length - 1),
    };
    const previousHunk = hunks.at(-1);

    if (previousHunk === undefined || nextHunk.start > previousHunk.end + 1) {
      hunks.push(nextHunk);
      continue;
    }

    previousHunk.end = Math.max(previousHunk.end, nextHunk.end);
  }

  for (const hunk of hunks) {
    const hunkLines = operations.slice(hunk.start, hunk.end + 1);
    const firstOldLine =
      hunkLines.find((line) => line.oldLine !== null)?.oldLine ??
      (hunkLines[0]?.oldLine ?? 0);
    const firstNewLine =
      hunkLines.find((line) => line.newLine !== null)?.newLine ??
      (hunkLines[0]?.newLine ?? 0);
    const oldCount = hunkLines.filter((line) => line.type !== "add").length;
    const newCount = hunkLines.filter((line) => line.type !== "remove").length;

    outputLines.push(`@@ -${firstOldLine},${oldCount} +${firstNewLine},${newCount} @@`);

    for (const hunkLine of hunkLines) {
      const prefix =
        hunkLine.type === "context"
          ? " "
          : hunkLine.type === "add"
            ? "+"
            : "-";
      outputLines.push(`${prefix}${hunkLine.line}`);
    }
  }

  return outputLines.join("\n");
}

function buildSummary(changes: DiffChange[]): string {
  if (changes.length === 0) {
    return "未检测到有效配置差异。";
  }

  const addCount = changes.filter((change) => change.type === "add").length;
  const removeCount = changes.filter((change) => change.type === "remove").length;
  const modifyCount = changes.filter((change) => change.type === "modify").length;
  const countSegments = [
    addCount > 0 ? `新增 ${addCount} 项` : null,
    removeCount > 0 ? `删除 ${removeCount} 项` : null,
    modifyCount > 0 ? `调整 ${modifyCount} 项` : null,
  ].filter((segment): segment is string => segment !== null);
  const highlights = changes
    .slice(0, 3)
    .map((change) => change.description)
    .join("；");

  return `本次将${countSegments.join("，")}：${highlights}${changes.length > 3 ? `；共 ${changes.length} 项变更` : ""}`;
}

export function isPendingConfigChange(value: unknown): value is PendingConfigChange {
  return pendingConfigChangeSchema.safeParse(value).success;
}

export function generateDiff(
  originalYaml: string,
  toolResults: PendingConfigChange[],
): ConfigDiff {
  return generateDiffFromConfigDocument(loadYamlDocument(originalYaml), toolResults);
}

export function generateDiffFromConfigDocument(
  originalConfig: unknown,
  toolResults: PendingConfigChange[],
): ConfigDiff {
  const originalDocument = loadConfigDocument(structuredClone(originalConfig));
  const document = structuredClone(originalDocument);
  const changes: DiffChange[] = [];

  for (const toolResult of toolResults) {
    applyPendingChange(document, toolResult, changes);
  }

  const original = dumpYamlDocument(originalDocument);
  const modified = dumpYamlDocument(document);

  return {
    original: normalizeYamlText(original),
    modified: normalizeYamlText(modified),
    unifiedDiff: buildUnifiedDiff(original, modified),
    summary: buildSummary(changes),
    changes,
  };
}
