import * as Accordion from "@radix-ui/react-accordion";
import { motion } from "framer-motion";
import { useEffect, useState, type ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  Bot,
  CheckCircle2,
  ChevronDown,
  Clock3,
  FileCheck2,
  FileClock,
  type LucideIcon,
  TriangleAlert,
  UserRound,
  Wrench,
  XCircle,
} from "lucide-react";
import { ConfigDiffPreview } from "./config-diff-preview";
import { useConfigApply } from "./hooks/use-config-apply";
import { markdownComponents } from "./markdown-components";
import { isPendingConfigChangeResult } from "@/lib/tauri-api";
import { cn } from "@/lib/utils";
import type { AiMessage, AiToolCall } from "@/stores/ai-store";

const timeFormatter = new Intl.DateTimeFormat("zh-CN", {
  hour: "2-digit",
  minute: "2-digit",
});

const TOOL_STATUS_META: Record<
  AiToolCall["status"],
  { label: string; className: string; icon: LucideIcon }
> = {
  pending: {
    label: "等待中",
    className: "border-border/70 bg-muted/40 text-muted-foreground",
    icon: Clock3,
  },
  executing: {
    label: "执行中",
    className: "border-primary/20 bg-primary/10 text-primary",
    icon: Wrench,
  },
  awaiting_confirmation: {
    label: "待确认",
    className: "border-cyan-500/25 bg-cyan-500/10 text-cyan-300",
    icon: FileClock,
  },
  completed: {
    label: "已完成",
    className: "border-emerald-500/20 bg-emerald-500/10 text-emerald-400",
    icon: CheckCircle2,
  },
  applied: {
    label: "已应用",
    className: "border-emerald-500/20 bg-emerald-500/10 text-emerald-300",
    icon: FileCheck2,
  },
  rejected: {
    label: "已取消",
    className: "border-slate-500/20 bg-slate-500/10 text-slate-300",
    icon: XCircle,
  },
  error: {
    label: "失败",
    className: "border-destructive/20 bg-destructive/10 text-destructive",
    icon: TriangleAlert,
  },
};

function formatStructuredValue(value: unknown) {
  if (typeof value === "string") {
    return value;
  }

  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return "无法序列化该工具输出";
  }
}

function MessageIcon({ role }: { role: AiMessage["role"] }) {
  if (role === "user") {
    return <UserRound className="size-4" />;
  }

  return <Bot className="size-4" />;
}

function RoleLabel({ role }: { role: AiMessage["role"] }) {
  const label = role === "user" ? "你" : role === "assistant" ? "AI 助手" : "系统";
  return <span>{label}</span>;
}

function StructuredPanel({
  label,
  content,
}: {
  label: string;
  content: string;
}) {
  return (
    <section className="rounded-[1rem] border border-border/70 bg-muted/25 p-3">
      <p className="mb-2 text-[11px] font-medium tracking-[0.18em] text-muted-foreground uppercase">
        {label}
      </p>
      <pre className="overflow-x-auto whitespace-pre-wrap break-words font-mono text-[12px] leading-6 text-foreground">
        {content}
      </pre>
    </section>
  );
}

function PendingConfigToolResult({ toolCall }: { toolCall: AiToolCall }) {
  const pendingResult = isPendingConfigChangeResult(toolCall.result) ? toolCall.result : null;

  if (pendingResult === null) {
    return null;
  }

  const { apply, reject, isApplying, isRejecting, error } = useConfigApply(
    toolCall.id,
    pendingResult.confirmationBatchId,
  );

  if (!pendingResult.isLatestInBatch) {
    const batchMessage =
      toolCall.status === "applied"
        ? `这项修改已随同本轮共 ${pendingResult.confirmationBatchSize} 项配置变更一起应用。`
        : toolCall.status === "rejected"
          ? `这项修改已随同本轮共 ${pendingResult.confirmationBatchSize} 项配置变更一起丢弃。`
          : `这项修改已合并到本轮最终预览；请在本轮最后一项中统一确认，共 ${pendingResult.confirmationBatchSize} 项配置变更。`;

    return (
      <div className="rounded-[1rem] border border-border/70 bg-muted/25 px-4 py-3 text-sm leading-6 text-muted-foreground">
        {batchMessage}
      </div>
    );
  }

  return (
    <ConfigDiffPreview
      diff={pendingResult.diff}
      onConfirm={apply}
      onReject={reject}
      isApplying={isApplying}
      isRejecting={isRejecting}
      error={error}
      status={toolCall.status}
    />
  );
}

function ToolCallCard({ toolCall }: { toolCall: AiToolCall }) {
  const meta = TOOL_STATUS_META[toolCall.status];
  const StatusIcon = meta.icon;
  const hasPendingDiff = isPendingConfigChangeResult(toolCall.result);

  return (
    <Accordion.Item
      value={toolCall.id}
      className="overflow-hidden rounded-[1.25rem] border border-border/70 bg-background/60"
    >
      <Accordion.Header>
        <Accordion.Trigger className="group flex w-full items-center gap-3 px-4 py-3 text-left">
          <div className="flex size-8 items-center justify-center rounded-full bg-primary/10 text-primary">
            <Wrench className="size-4" />
          </div>

          <div className="min-w-0 flex-1">
            <div className="truncate text-sm font-medium text-foreground">{toolCall.name}</div>
            <div className="mt-0.5 text-xs text-muted-foreground">工具调用 ID · {toolCall.id}</div>
          </div>

          <span
            className={cn(
              "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium tracking-[0.16em] uppercase",
              meta.className,
            )}
          >
            <StatusIcon className="size-3.5" />
            {meta.label}
          </span>

          <ChevronDown className="size-4 shrink-0 text-muted-foreground transition-transform group-data-[state=open]:rotate-180" />
        </Accordion.Trigger>
      </Accordion.Header>

      <Accordion.Content className="overflow-hidden border-t border-border/70 data-[state=closed]:animate-accordion-up data-[state=open]:animate-accordion-down">
        <div className="space-y-3 p-4">
          <div className={cn("grid gap-3", hasPendingDiff ? "xl:grid-cols-1" : "xl:grid-cols-2")}>
            <StructuredPanel label="参数" content={formatStructuredValue(toolCall.input)} />
            {!hasPendingDiff ? (
              <StructuredPanel
                label="结果"
                content={
                  toolCall.result === undefined
                    ? "等待工具返回结果..."
                    : formatStructuredValue(toolCall.result)
                }
              />
            ) : null}
          </div>

          {hasPendingDiff ? <PendingConfigToolResult toolCall={toolCall} /> : null}
        </div>
      </Accordion.Content>
    </Accordion.Item>
  );
}

function AssistantContent({ message }: { message: AiMessage }) {
  const hasText = message.content.trim().length > 0;

  if (!hasText && message.isStreaming) {
    return (
      <div className="inline-flex items-center gap-2 text-sm text-muted-foreground">
        <span>正在组织回复</span>
        <span className="h-4 w-2 animate-pulse rounded-full bg-primary/80" />
      </div>
    );
  }

  if (!hasText) {
    return null;
  }

  return (
    <div className="text-sm leading-7 text-foreground">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={markdownComponents}>
        {message.content}
      </ReactMarkdown>
      {message.isStreaming ? (
        <span className="inline-flex h-4 w-2 translate-y-[3px] animate-pulse rounded-full bg-primary/80" />
      ) : null}
    </div>
  );
}

function UserContent({ content }: { content: string }) {
  return <p className="whitespace-pre-wrap text-sm leading-7 text-primary-foreground">{content}</p>;
}

function MessageBody({ message }: { message: AiMessage }) {
  if (message.role === "user") {
    return <UserContent content={message.content} />;
  }

  return <AssistantContent message={message} />;
}

function ToolCalls({ toolCalls }: { toolCalls: AiToolCall[] }) {
  const pendingToolCallIds = toolCalls
    .filter((toolCall) => toolCall.status === "awaiting_confirmation")
    .map((toolCall) => toolCall.id);
  const pendingToolCallKey = pendingToolCallIds.join("::");
  const [expandedItems, setExpandedItems] = useState<string[]>(pendingToolCallIds);

  useEffect(() => {
    if (pendingToolCallIds.length === 0) {
      return;
    }

    setExpandedItems((currentValue) => [
      ...new Set([...currentValue, ...pendingToolCallIds]),
    ]);
  }, [pendingToolCallKey]);

  if (toolCalls.length === 0) {
    return null;
  }

  return (
    <Accordion.Root
      type="multiple"
      value={expandedItems}
      onValueChange={setExpandedItems}
      className="mt-4 space-y-2"
    >
      {toolCalls.map((toolCall) => (
        <ToolCallCard key={toolCall.id} toolCall={toolCall} />
      ))}
    </Accordion.Root>
  );
}

function BubbleFrame({
  message,
  children,
}: {
  message: AiMessage;
  children: ReactNode;
}) {
  if (message.role === "user") {
    return (
      <div className="max-w-[min(42rem,100%)] rounded-[1.75rem] bg-linear-to-br from-primary via-primary to-primary/70 px-5 py-4 shadow-[0_24px_60px_-34px_var(--color-primary)]">
        {children}
      </div>
    );
  }

  return (
    <div className="relative max-w-[min(46rem,100%)] overflow-hidden rounded-[1.9rem] border border-border/70 bg-linear-to-br from-primary/10 via-background/96 to-background/92 px-5 py-4 shadow-[0_28px_90px_-48px_rgba(15,23,42,0.6)]">
      <div className="pointer-events-none absolute inset-x-6 top-0 h-px bg-linear-to-r from-transparent via-primary/25 to-transparent" />
      <div className="pointer-events-none absolute -right-12 top-0 size-28 rounded-full bg-primary/10 blur-3xl" />
      <div className="relative">{children}</div>
    </div>
  );
}

export function ChatMessage({ message }: { message: AiMessage }) {
  const isUser = message.role === "user";

  return (
    <motion.article
      layout
      initial={{ opacity: 0, y: 14 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.22, ease: "easeOut" }}
      className={cn("flex", isUser ? "justify-end" : "justify-start")}
    >
      <BubbleFrame message={message}>
        <div className="mb-3 flex items-center gap-2 text-xs font-medium tracking-[0.18em] uppercase">
          <span
            className={cn(
              "inline-flex size-7 items-center justify-center rounded-full border",
              isUser
                ? "border-white/20 bg-white/12 text-white"
                : "border-primary/20 bg-primary/10 text-primary",
            )}
          >
            <MessageIcon role={message.role} />
          </span>

          <span className={cn(isUser ? "text-primary-foreground/80" : "text-muted-foreground")}>
            <RoleLabel role={message.role} />
          </span>

          <span
            className={cn(
              "ml-auto",
              isUser ? "text-primary-foreground/70" : "text-muted-foreground",
            )}
          >
            {timeFormatter.format(message.timestamp)}
          </span>
        </div>

        <MessageBody message={message} />
        <ToolCalls toolCalls={message.toolCalls} />
      </BubbleFrame>
    </motion.article>
  );
}
