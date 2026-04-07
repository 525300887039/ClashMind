import yaml from "js-yaml";
import { z } from "zod";
import { isRecord } from "../utils.js";

const PROXY_TYPES = [
  "ss",
  "vmess",
  "vless",
  "trojan",
  "hysteria2",
  "tuic",
  "wireguard",
  "ssh",
  "http",
  "socks5",
] as const;

const PROXY_GROUP_TYPES = [
  "select",
  "url-test",
  "fallback",
  "load-balance",
  "relay",
] as const;

const RULE_TYPES = [
  "DOMAIN",
  "DOMAIN-SUFFIX",
  "DOMAIN-KEYWORD",
  "DOMAIN-REGEX",
  "IP-CIDR",
  "IP-CIDR6",
  "SRC-IP-CIDR",
  "GEOIP",
  "GEOSITE",
  "PROCESS-NAME",
  "PROCESS-PATH",
  "RULE-SET",
  "MATCH",
] as const;

const RULE_TYPE_SET = new Set<string>(RULE_TYPES);
const BUILTIN_PROXY_REFERENCES = new Set(["DIRECT", "REJECT"]);

const modeSchema = z.enum(["rule", "global", "direct"]);

const proxyBaseSchema = z
  .object({
    name: z.string().min(1, "代理节点名称不能为空"),
    type: z.enum(PROXY_TYPES),
    server: z.string().min(1, "服务器地址不能为空"),
    port: z.number().int().min(1).max(65535),
  })
  .passthrough();

const proxyGroupBaseSchema = z
  .object({
    name: z.string().min(1, "代理组名称不能为空"),
    type: z.enum(PROXY_GROUP_TYPES),
    proxies: z.array(z.string().min(1)).optional(),
    use: z.array(z.string().min(1)).optional(),
    filter: z.string().min(1).optional(),
    url: z.string().min(1).optional(),
    interval: z.number().int().min(10).optional(),
    tolerance: z.number().int().min(0).optional(),
  })
  .passthrough();

const proxyGroupUpdateSchema = proxyGroupBaseSchema.partial();

const dnsBaseSchema = z
  .object({
    enable: z.boolean().optional(),
    "enhanced-mode": z.enum(["fake-ip", "redir-host"]).optional(),
    nameserver: z.array(z.string().min(1)).optional(),
    fallback: z.array(z.string().min(1)).optional(),
    "fake-ip-filter": z.array(z.string().min(1)).optional(),
  })
  .passthrough();

export interface ValidationError {
  path: string;
  message: string;
}

export interface ValidationWarning {
  path: string;
  message: string;
}

export interface ValidationResult {
  valid: boolean;
  errors: ValidationError[];
  warnings: ValidationWarning[];
}

export interface ValidationErrorResult {
  action: "validation_error";
  errors: ValidationError[];
  warnings: ValidationWarning[];
}

export const ProxySchema = proxyBaseSchema;

export const ProxyGroupSchema = proxyGroupBaseSchema.superRefine((group, context) => {
  if (
    group.proxies === undefined &&
    group.use === undefined &&
    group.filter === undefined
  ) {
    context.addIssue({
      code: z.ZodIssueCode.custom,
      path: ["proxies"],
      message: "代理组必须指定 proxies、use 或 filter 之一",
    });
  }
});

export const RuleSchema = z
  .string()
  .trim()
  .min(1, "规则不能为空")
  .superRefine((rule, context) => {
    const parts = rule.split(",").map((part) => part.trim());
    const type = parts[0];

    if (type === undefined || !RULE_TYPE_SET.has(type)) {
      context.addIssue({
        code: z.ZodIssueCode.custom,
        message: "无效的规则类型",
      });
      return;
    }

    if (type === "MATCH") {
      if (parts.length < 2 || parts[1] === undefined || parts[1].length === 0) {
        context.addIssue({
          code: z.ZodIssueCode.custom,
          message: "MATCH 规则格式应为 MATCH,POLICY",
        });
      }
      return;
    }

    if (parts.length < 3) {
      context.addIssue({
        code: z.ZodIssueCode.custom,
        message: "规则格式应为 TYPE,VALUE,POLICY",
      });
      return;
    }

    if (parts[1] === undefined || parts[1].length === 0) {
      context.addIssue({
        code: z.ZodIssueCode.custom,
        message: "规则 VALUE 不能为空",
      });
    }

    if (parts[2] === undefined || parts[2].length === 0) {
      context.addIssue({
        code: z.ZodIssueCode.custom,
        message: "规则 POLICY 不能为空",
      });
    }
  });

export const DnsSchema = dnsBaseSchema;

export const MihomoConfigSchema = z
  .object({
    port: z.number().int().min(0).max(65535).optional(),
    "socks-port": z.number().int().min(0).max(65535).optional(),
    "mixed-port": z.number().int().min(0).max(65535).optional(),
    mode: modeSchema.optional(),
    "log-level": z
      .enum(["silent", "error", "warning", "info", "debug"])
      .optional(),
    "allow-lan": z.boolean().optional(),
    proxies: z.array(ProxySchema).optional(),
    "proxy-groups": z.array(ProxyGroupSchema).optional(),
    rules: z.array(RuleSchema).optional(),
    dns: DnsSchema.optional(),
  })
  .passthrough();

function toPathString(path: Array<string | number>): string {
  if (path.length === 0) {
    return "";
  }

  return path.reduce<string>((result, segment) => {
    if (typeof segment === "number") {
      return `${result}[${segment}]`;
    }

    return result.length === 0 ? segment : `${result}.${segment}`;
  }, "");
}

function createValidationResult(
  errors: ValidationError[] = [],
  warnings: ValidationWarning[] = [],
): ValidationResult {
  return {
    valid: errors.length === 0,
    errors,
    warnings,
  };
}

function createValidationIssue(
  path: string,
  message: string,
): ValidationError {
  return { path, message };
}

function createValidationWarning(
  path: string,
  message: string,
): ValidationWarning {
  return { path, message };
}

function mapZodErrors(error: z.ZodError<unknown>): ValidationError[] {
  return error.issues.map((issue) =>
    createValidationIssue(toPathString(issue.path), issue.message),
  );
}

function validateWithSchema<TSchema extends z.ZodType>(
  schema: TSchema,
  input: unknown,
): ValidationResult {
  const parsed = schema.safeParse(input);
  if (parsed.success) {
    return createValidationResult();
  }

  return createValidationResult(mapZodErrors(parsed.error));
}

function loadYamlObject(
  yamlContent: string,
  label: string,
): { document: Record<string, unknown> | null; errors: ValidationError[] } {
  let parsed: unknown;

  try {
    parsed = yaml.load(yamlContent);
  } catch (error) {
    const message = error instanceof Error ? error.message : "未知 YAML 解析错误";
    return {
      document: null,
      errors: [
        createValidationIssue(
          "",
          label.length > 0 ? `${label} YAML 语法错误: ${message}` : `YAML 语法错误: ${message}`,
        ),
      ],
    };
  }

  if (parsed === undefined || parsed === null) {
    return {
      document: {},
      errors: [],
    };
  }

  if (!isRecord(parsed)) {
    return {
      document: null,
      errors: [
        createValidationIssue(
          "",
          label.length > 0 ? `${label}根节点必须是对象` : "YAML 根节点必须是对象",
        ),
      ],
    };
  }

  return {
    document: parsed,
    errors: [],
  };
}

function getRuleType(rule: string): string {
  const [type = ""] = rule.split(",", 1);
  return type.trim();
}

function validateMatchRulePlacement(
  config: Record<string, unknown>,
  errors: ValidationError[],
  warnings: ValidationWarning[],
): void {
  const { rules } = config;
  if (!Array.isArray(rules) || rules.some((rule) => typeof rule !== "string")) {
    return;
  }

  const matchIndexes = rules.reduce<number[]>((indexes, rule, index) => {
    if (getRuleType(rule) === "MATCH") {
      indexes.push(index);
    }
    return indexes;
  }, []);

  if (matchIndexes.length === 0) {
    warnings.push(createValidationWarning("rules", "建议添加 MATCH 兜底规则"));
    return;
  }

  const lastRuleIndex = rules.length - 1;
  for (const matchIndex of matchIndexes) {
    if (matchIndex !== lastRuleIndex) {
      errors.push(
        createValidationIssue(`rules[${matchIndex}]`, "MATCH 规则必须放在最后"),
      );
    }
  }
}

function collectReferenceNames(config: Record<string, unknown>): Set<string> {
  const names = new Set(BUILTIN_PROXY_REFERENCES);

  const proxies = config.proxies;
  if (Array.isArray(proxies)) {
    for (const proxy of proxies) {
      if (isRecord(proxy) && typeof proxy.name === "string" && proxy.name.length > 0) {
        names.add(proxy.name);
      }
    }
  }

  const proxyGroups = config["proxy-groups"];
  if (Array.isArray(proxyGroups)) {
    for (const group of proxyGroups) {
      if (isRecord(group) && typeof group.name === "string" && group.name.length > 0) {
        names.add(group.name);
      }
    }
  }

  return names;
}

function validateProxyGroupReferences(
  config: Record<string, unknown>,
  warnings: ValidationWarning[],
): void {
  const proxyGroups = config["proxy-groups"];
  if (!Array.isArray(proxyGroups)) {
    return;
  }

  const knownReferences = collectReferenceNames(config);

  proxyGroups.forEach((group, groupIndex) => {
    if (!isRecord(group) || !Array.isArray(group.proxies)) {
      return;
    }

    group.proxies.forEach((proxy, proxyIndex) => {
      if (typeof proxy !== "string") {
        return;
      }

      if (!knownReferences.has(proxy)) {
        warnings.push(
          createValidationWarning(
            `proxy-groups[${groupIndex}].proxies[${proxyIndex}]`,
            `引用的节点或策略「${proxy}」不存在`,
          ),
        );
      }
    });
  });
}

export function createValidationErrorResult(
  validation: ValidationResult,
): ValidationErrorResult {
  return {
    action: "validation_error",
    errors: validation.errors,
    warnings: validation.warnings,
  };
}

export function validateConfig(yamlContent: string): ValidationResult {
  const errors: ValidationError[] = [];
  const warnings: ValidationWarning[] = [];
  const { document, errors: yamlErrors } = loadYamlObject(yamlContent, "");

  if (yamlErrors.length > 0 || document === null) {
    return createValidationResult(yamlErrors, warnings);
  }

  const parsedConfig = MihomoConfigSchema.safeParse(document);
  if (!parsedConfig.success) {
    errors.push(...mapZodErrors(parsedConfig.error));
  }

  validateMatchRulePlacement(document, errors, warnings);
  validateProxyGroupReferences(document, warnings);

  return createValidationResult(errors, warnings);
}

export function validateProxyFragment(proxy: unknown): ValidationResult {
  return validateWithSchema(ProxySchema, proxy);
}

export function validateProxyGroupFragment(group: unknown): ValidationResult {
  return validateWithSchema(ProxyGroupSchema, group);
}

export function validateProxyGroupUpdateFragment(group: unknown): ValidationResult {
  return validateWithSchema(proxyGroupUpdateSchema, group);
}

export function validateRuleFragment(rule: unknown): ValidationResult {
  return validateWithSchema(RuleSchema, rule);
}

export function validateDnsFragment(dns: unknown): ValidationResult {
  return validateWithSchema(DnsSchema, dns);
}

export function validateModeFragment(mode: unknown): ValidationResult {
  return validateWithSchema(modeSchema, mode);
}

export function validateBeforeApply(
  originalYaml: string,
  modifiedYaml: string,
): ValidationResult {
  const validation = validateConfig(modifiedYaml);
  const originalLoaded = loadYamlObject(originalYaml, "原始配置");
  const modifiedLoaded = loadYamlObject(modifiedYaml, "修改后配置");

  if (originalLoaded.errors.length > 0) {
    validation.errors.push(...originalLoaded.errors);
  }

  if (modifiedLoaded.errors.length > 0) {
    validation.errors.push(...modifiedLoaded.errors);
  }

  if (originalLoaded.document !== null && modifiedLoaded.document !== null) {
    const originalProxies = Array.isArray(originalLoaded.document.proxies)
      ? originalLoaded.document.proxies
      : [];
    const modifiedProxies = Array.isArray(modifiedLoaded.document.proxies)
      ? modifiedLoaded.document.proxies
      : [];

    if (originalProxies.length > 0 && modifiedProxies.length === 0) {
      validation.errors.push(
        createValidationIssue("proxies", "不能删除所有代理节点"),
      );
    }
  }

  validation.valid = validation.errors.length === 0;
  return validation;
}
