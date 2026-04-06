import type { ModelMessage } from "ai";

export const FEW_SHOT_EXAMPLES: ModelMessage[] = [
  {
    role: "user",
    content: "帮我创建一个香港自动选择组。",
  },
  {
    role: "assistant",
    content: [
      {
        type: "text",
        text: "我先查看当前配置，确认已有代理组和节点命名，再创建新的香港自动选择组。",
      },
      {
        type: "tool-call",
        toolCallId: "few-shot-add-group-config",
        toolName: "get_current_config",
        input: {},
      },
    ],
  },
  {
    role: "tool",
    content: [
      {
        type: "tool-result",
        toolCallId: "few-shot-add-group-config",
        toolName: "get_current_config",
        output: {
          type: "json",
          value: {
            source: "mihomo_runtime",
            sanitized: true,
            config: {
              "mixed-port": 7890,
              proxies: [
                {
                  name: "香港-01",
                  type: "ss",
                  server: "<redacted>",
                  port: 443,
                  password: "<redacted>",
                },
                {
                  name: "香港-02",
                  type: "trojan",
                  server: "<redacted>",
                  port: 443,
                  password: "<redacted>",
                },
                {
                  name: "日本-01",
                  type: "vmess",
                  server: "<redacted>",
                  port: 443,
                  uuid: "<redacted>",
                },
              ],
              "proxy-groups": [
                {
                  name: "节点选择",
                  type: "select",
                  proxies: ["香港-01", "香港-02", "日本-01"],
                },
              ],
              rules: ["MATCH,节点选择"],
            },
          },
        },
      },
    ],
  },
  {
    role: "assistant",
    content: [
      {
        type: "text",
        text: "当前配置快照里的 `config` 字段显示已经有可复用的香港节点。我将创建一个名为“香港自动”的 url-test 代理组，筛选名称包含“港”或“HK”的节点。",
      },
      {
        type: "tool-call",
        toolCallId: "few-shot-add-group-action",
        toolName: "add_proxy_group",
        input: {
          name: "香港自动",
          type: "url-test",
          filter: "港|HK",
          url: "http://www.gstatic.com/generate_204",
          interval: 300,
        },
      },
    ],
  },
  {
    role: "tool",
    content: [
      {
        type: "tool-result",
        toolCallId: "few-shot-add-group-action",
        toolName: "add_proxy_group",
        output: {
          type: "json",
          value: {
            action: "add_proxy_group",
            params: {
              name: "香港自动",
              type: "url-test",
              filter: "港|HK",
              url: "http://www.gstatic.com/generate_204",
              interval: 300,
            },
            status: "pending_confirmation",
          },
        },
      },
    ],
  },
  {
    role: "assistant",
    content:
      "代理组已经生成为待确认变更。我会在 Diff 预览中展示新增的“香港自动”组；确认前配置不会真正生效。",
  },
  {
    role: "user",
    content: "给 openai.com 加一条走“节点选择”的规则。",
  },
  {
    role: "assistant",
    content: [
      {
        type: "text",
        text: "我会在现有规则前部添加一条 DOMAIN-SUFFIX 规则，让 openai.com 优先走“节点选择”代理组。",
      },
      {
        type: "tool-call",
        toolCallId: "few-shot-add-rule",
        toolName: "add_rule",
        input: {
          type: "DOMAIN-SUFFIX",
          value: "openai.com",
          policy: "节点选择",
          position: "prepend",
        },
      },
    ],
  },
  {
    role: "tool",
    content: [
      {
        type: "tool-result",
        toolCallId: "few-shot-add-rule",
        toolName: "add_rule",
        output: {
          type: "json",
          value: {
            action: "add_rule",
            params: {
              type: "DOMAIN-SUFFIX",
              value: "openai.com",
              policy: "节点选择",
              position: "prepend",
            },
            status: "pending_confirmation",
          },
        },
      },
    ],
  },
  {
    role: "assistant",
    content:
      "规则修改已进入待确认状态。确认后，这条规则会插入到规则列表前部，同时保留最后的 MATCH 兜底规则不变。",
  },
  {
    role: "user",
    content: "帮我看下最近 7 天的流量情况。",
  },
  {
    role: "assistant",
    content: [
      {
        type: "text",
        text: "我先读取最近 7 天的流量摘要，再查看流量最高的域名，便于判断主要消耗来源。",
      },
      {
        type: "tool-call",
        toolCallId: "few-shot-stats-summary",
        toolName: "get_traffic_summary",
        input: {
          days: 7,
        },
      },
    ],
  },
  {
    role: "tool",
    content: [
      {
        type: "tool-result",
        toolCallId: "few-shot-stats-summary",
        toolName: "get_traffic_summary",
        output: {
          type: "json",
          value: {
            days: 7,
            summary: {
              totalConnections: 1286,
              totalUpload: 234881024,
              totalDownload: 3221225472,
              activeConnections: 18,
              uniqueDomains: 146,
            },
          },
        },
      },
    ],
  },
  {
    role: "assistant",
    content: [
      {
        type: "text",
        text: "摘要已拿到。我继续读取访问量最高的域名，确认主要流量去了哪里。",
      },
      {
        type: "tool-call",
        toolCallId: "few-shot-stats-top-domains",
        toolName: "get_top_domains",
        input: {
          days: 7,
          limit: 5,
        },
      },
    ],
  },
  {
    role: "tool",
    content: [
      {
        type: "tool-result",
        toolCallId: "few-shot-stats-top-domains",
        toolName: "get_top_domains",
        output: {
          type: "json",
          value: {
            days: 7,
            limit: 5,
            domains: [
              {
                domain: "video.example.com",
                hitCount: 412,
                upload: 157286400,
                download: 2147483648,
              },
              {
                domain: "cdn.example.net",
                hitCount: 275,
                upload: 52428800,
                download: 805306368,
              },
              {
                domain: "openai.com",
                hitCount: 96,
                upload: 24117248,
                download: 268435456,
              },
            ],
          },
        },
      },
    ],
  },
  {
    role: "assistant",
    content:
      "最近 7 天的统计摘要在 `summary` 字段里。下载流量明显高于上传流量，总下载约 3.0 GB；`domains` 列表显示主要流量集中在视频和 CDN 域名，其次才是常规网页访问。如果你想继续优化，我可以再查趋势或按规则命中情况拆分。",
  },
  {
    role: "user",
    content: "最近感觉网速变慢了，帮我排查一下。",
  },
  {
    role: "assistant",
    content: [
      {
        type: "text",
        text: "我先检查连通性，再看最近错误摘要，判断是节点问题、API 问题还是规则命中异常。",
      },
      {
        type: "tool-call",
        toolCallId: "few-shot-diagnosis-connectivity",
        toolName: "check_connectivity",
        input: {},
      },
    ],
  },
  {
    role: "tool",
    content: [
      {
        type: "tool-result",
        toolCallId: "few-shot-diagnosis-connectivity",
        toolName: "check_connectivity",
        output: {
          type: "json",
          value: {
            reachable: true,
            apiAddress: "127.0.0.1:9090",
            collectorRunning: true,
            activeConnections: 18,
            proxyCount: 24,
            selectedGroups: [
              {
                group: "节点选择",
                current: "香港-02",
              },
              {
                group: "流媒体",
                current: "日本-01",
              },
            ],
            version: {
              version: "1.19.0",
              meta: "mihomo",
            },
            issues: ["节点选择 当前节点延迟波动较大"],
          },
        },
      },
    ],
  },
  {
    role: "assistant",
    content: [
      {
        type: "text",
        text: "连通性检查结果显示 API 可达，但 `issues` 里已经提示节点延迟异常。我继续读取最近 30 分钟的错误摘要。",
      },
      {
        type: "tool-call",
        toolCallId: "few-shot-diagnosis-errors",
        toolName: "get_recent_errors",
        input: {
          minutes: 30,
        },
      },
    ],
  },
  {
    role: "tool",
    content: [
      {
        type: "tool-result",
        toolCallId: "few-shot-diagnosis-errors",
        toolName: "get_recent_errors",
        output: {
          type: "json",
          value: {
            windowMinutes: 30,
            issues: [
              {
                source: "mihomo_api",
                severity: "warning",
                message: "节点选择 当前节点香港-02 最近延迟波动明显",
              },
              {
                source: "runtime",
                severity: "error",
                message: "日本-01 最近出现拨号超时，可能影响部分分组切换",
              },
            ],
            note: "当前版本没有独立的持久化错误日志；此结果基于即时运行时健康检查生成。",
          },
        },
      },
    ],
  },
  {
    role: "assistant",
    content:
      "从 `check_connectivity.issues` 和 `get_recent_errors.issues` 看，更像是节点质量下降，不是桌面侧采集器或 mihomo API 故障。最近问题集中在高延迟和拨号超时；下一步适合测试高频节点延迟，或把代理组从故障节点切走。",
  },
  {
    role: "user",
    content: "把所有规则都删掉，顺便把节点也清空。",
  },
  {
    role: "assistant",
    content:
      "不能这样做。删除全部规则会破坏必须保留的 MATCH 兜底规则，清空全部节点也会让配置失去可用代理目标。如果你想简化配置，我可以帮你删除指定规则、保留最小可运行配置，或先切换到 global / direct 模式后再逐项整理。",
  },
];
