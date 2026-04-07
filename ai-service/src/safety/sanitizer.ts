import yaml from "js-yaml";
import { isRecord } from "../utils.js";

interface ConfigDocument {
  [key: string]: unknown;
}

interface ProxyDocument {
  [key: string]: unknown;
  name?: unknown;
}

export interface SanitizeMapping {
  proxyName: string;
  field: string;
  placeholder: string;
  originalValue: string;
}

const SERVER_FIELD = "server";
const REDACTED_PLACEHOLDER = "[REDACTED]";
const SENSITIVE_PROXY_FIELDS = [
  SERVER_FIELD,
  "password",
  "uuid",
  "key",
  "private-key",
  "public-key",
  "pre-shared-key",
  "token",
  "obfs-password",
] as const;

function loadConfigDocument(yamlContent: string): ConfigDocument {
  const parsed = yaml.load(yamlContent);

  if (parsed === undefined || parsed === null) {
    return {};
  }

  if (!isRecord(parsed)) {
    throw new Error("配置根节点必须是 YAML 对象");
  }

  return parsed;
}

function dumpConfigDocument(document: ConfigDocument): string {
  return yaml.dump(document, {
    lineWidth: -1,
    noArrayIndent: false,
    noCompatMode: true,
    noRefs: true,
    sortKeys: false,
  });
}

function isProxyDocument(value: unknown): value is ProxyDocument {
  return isRecord(value);
}

function getProxyName(proxy: ProxyDocument, index: number): string {
  if (typeof proxy.name === "string" && proxy.name.length > 0) {
    return proxy.name;
  }

  return `proxy_${index + 1}`;
}

function getProxies(document: ConfigDocument): unknown[] {
  const { proxies } = document;

  if (proxies === undefined) {
    return [];
  }

  if (!Array.isArray(proxies)) {
    throw new Error("proxies 必须是数组");
  }

  return proxies;
}

export class ConfigSanitizer {
  sanitize(yamlContent: string): {
    sanitized: string;
    mappings: SanitizeMapping[];
  } {
    const document = loadConfigDocument(yamlContent);
    const mappings: SanitizeMapping[] = [];
    let serverCounter = 0;

    for (const [index, proxyValue] of getProxies(document).entries()) {
      if (!isProxyDocument(proxyValue)) {
        continue;
      }

      const proxyName = getProxyName(proxyValue, index);
      for (const field of SENSITIVE_PROXY_FIELDS) {
        const fieldValue = proxyValue[field];
        if (typeof fieldValue !== "string") {
          continue;
        }

        const placeholder =
          field === SERVER_FIELD ? `SERVER_${++serverCounter}` : REDACTED_PLACEHOLDER;

        mappings.push({
          proxyName,
          field,
          placeholder,
          originalValue: fieldValue,
        });
        proxyValue[field] = placeholder;
      }
    }

    return {
      sanitized: dumpConfigDocument(document),
      mappings,
    };
  }

  restore(yamlContent: string, mappings: readonly SanitizeMapping[]): string {
    if (mappings.length === 0) {
      return yamlContent;
    }

    const document = loadConfigDocument(yamlContent);
    const proxies = getProxies(document);

    for (const mapping of mappings) {
      const proxy = proxies.find((proxyValue) => {
        if (!isProxyDocument(proxyValue)) {
          return false;
        }

        return proxyValue.name === mapping.proxyName;
      });

      if (!isProxyDocument(proxy)) {
        continue;
      }

      const currentValue = proxy[mapping.field];
      if (currentValue === mapping.placeholder) {
        proxy[mapping.field] = mapping.originalValue;
      }
    }

    return dumpConfigDocument(document);
  }
}
