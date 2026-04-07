import { useEffect, useRef } from "react";
import { motion } from "framer-motion";
import { Bot, Settings2, Sparkles, Trash2, Wrench } from "lucide-react";
import { Toaster } from "sonner";
import { ReportViewer } from "./report-viewer";
import { ChatInput } from "./chat-input";
import { ChatMessage } from "./chat-message";
import { useAiChat } from "./hooks/use-ai-chat";
import { isAiConfigured, useAiSettingsQuery } from "./hooks/use-ai-settings";
import { SnapshotList } from "./snapshot-list";
import { useAppStore } from "@/stores/app-store";
import { cn } from "@/lib/utils";
import { PageHeader } from "@/components/ui/page-header";

const CAPABILITY_CARDS = [
  {
    title: "流式对话",
    description: "边生成边展示结果，适合诊断、追因和规则推演。",
    icon: Sparkles,
  },
  {
    title: "报告解读",
    description: "日报与周报使用 Markdown 呈现，方便保存和复盘。",
    icon: Bot,
  },
  {
    title: "可审计动作",
    description: "工具调用轨迹和配置预览保持可展开、可确认。",
    icon: Wrench,
  },
] as const;

export function ChatPanel() {
  const { messages, isLoading, error, sendMessage, clearMessages } = useAiChat();
  const setCurrentPage = useAppStore((state) => state.setCurrentPage);
  const aiSettingsQuery = useAiSettingsQuery();
  const aiSettings = aiSettingsQuery.data;
  const scrollViewportRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const viewport = scrollViewportRef.current;
    if (viewport === null) {
      return;
    }

    const frame = window.requestAnimationFrame(() => {
      viewport.scrollTo({ top: viewport.scrollHeight, behavior: "auto" });
    });

    return () => window.cancelAnimationFrame(frame);
  }, [messages]);

  const hasMessages = messages.length > 0;
  const isConfigured = isAiConfigured(aiSettings);
  const modelBadgeLabel = isConfigured && aiSettings
    ? `${aiSettings.provider} / ${aiSettings.model}`
    : aiSettingsQuery.isLoading
      ? "读取 AI 配置中"
      : "未配置 AI";

  return (
    <motion.section
      initial={{ opacity: 0, y: 18 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.28, ease: "easeOut" }}
      className="flex h-[calc(100vh-5rem)] flex-col gap-4"
    >
      <Toaster position="top-center" richColors />

      <PageHeader
        eyebrow="Conversational Config Copilot"
        eyebrowIcon={Sparkles}
        title="AI 助手"
        description="让对话、统计报告和配置协作在同一个工作面板里展开。左侧处理流式会话与工具轨迹，右侧固定为 AI 统计报告工作台，适合先读信号、再落动作。"
        actions={
          <>
            <span className="inline-flex items-center gap-2 rounded-full border border-border/70 bg-background/70 px-3 py-2 text-xs font-medium tracking-[0.16em] text-muted-foreground uppercase">
              <Bot className="size-3.5 text-primary" />
              {modelBadgeLabel}
            </span>

            <button
              type="button"
              onClick={() => clearMessages()}
              disabled={!hasMessages || isLoading}
              className={cn(
                "inline-flex items-center gap-2 rounded-full border px-3 py-2 text-sm transition-colors",
                "border-border/70 bg-background/80 text-muted-foreground hover:border-destructive/25 hover:bg-destructive/5 hover:text-destructive",
                "disabled:cursor-not-allowed disabled:opacity-50",
              )}
            >
              <Trash2 className="size-4" />
              清空
            </button>

            <button
              type="button"
              onClick={() => setCurrentPage("settings")}
              className="inline-flex items-center gap-2 rounded-full border border-border/70 bg-background/80 px-3 py-2 text-sm text-muted-foreground transition-colors hover:border-primary/25 hover:bg-primary/5 hover:text-foreground"
            >
              <Settings2 className="size-4" />
              设置
            </button>
          </>
        }
      />

      <div className="grid min-h-0 flex-1 gap-4 xl:grid-cols-[minmax(0,1.06fr)_minmax(24rem,0.94fr)]">
        <div className="flex min-h-0 flex-col overflow-hidden rounded-[2rem] border border-border/70 bg-background/95 shadow-[0_28px_100px_-52px_rgba(15,23,42,0.55)]">
          <div className="flex items-center justify-between border-b border-border/70 px-5 py-4">
            <div>
              <p className="text-xs font-medium tracking-[0.18em] text-muted-foreground uppercase">
                Session Feed
              </p>
              <h2 className="mt-1 text-lg font-semibold text-foreground">消息面板</h2>
            </div>

            <div
              className={cn(
                "inline-flex items-center gap-2 rounded-full border px-3 py-1.5 text-xs font-medium tracking-[0.16em] uppercase",
                isLoading
                  ? "border-primary/20 bg-primary/10 text-primary"
                  : "border-border/70 bg-muted/40 text-muted-foreground",
              )}
            >
              <span
                className={cn(
                  "size-2 rounded-full",
                  isLoading ? "animate-pulse bg-primary" : "bg-emerald-400",
                )}
              />
              {isLoading ? "流式输出中" : "已就绪"}
            </div>
          </div>

          <div ref={scrollViewportRef} className="min-h-0 flex-1 overflow-y-auto px-4 py-5">
            {hasMessages ? (
              <div className="space-y-4">
                {messages.map((message) => (
                  <ChatMessage key={message.id} message={message} />
                ))}
              </div>
            ) : (
              <div className="relative flex h-full min-h-[24rem] items-center justify-center overflow-hidden rounded-[1.6rem] border border-dashed border-border/70 bg-muted/15 px-6 py-10">
                <div className="pointer-events-none absolute left-1/2 top-0 h-44 w-44 -translate-x-1/2 rounded-full bg-primary/10 blur-3xl" />

                <div className="relative max-w-2xl text-center">
                  <div className="mx-auto inline-flex size-14 items-center justify-center rounded-[1.25rem] border border-primary/20 bg-primary/10 text-primary shadow-[0_18px_50px_-30px_var(--color-primary)]">
                    <Bot className="size-6" />
                  </div>
                  <h3 className="mt-5 text-2xl font-semibold tracking-tight text-foreground">
                    开始一段面向配置的对话
                  </h3>
                  <p className="mt-3 text-sm leading-7 text-muted-foreground">
                    你可以让 AI 分析流量、检查代理连通性，或者为某个规则方案生成下一步建议。消息区会实时展示工具调用轨迹，便于你判断模型究竟做了什么。
                  </p>
                </div>
              </div>
            )}
          </div>

          <div className="border-t border-border/70 px-4 py-4">
            {error ? (
              <div className="mb-3 rounded-[1.25rem] border border-destructive/20 bg-destructive/5 px-4 py-3 text-sm text-destructive">
                {error}
              </div>
            ) : null}

            {!isConfigured ? (
              <div className="mb-3 rounded-[1.25rem] border border-amber-500/20 bg-amber-500/8 px-4 py-3 text-sm text-amber-200">
                AI 尚未配置。请先前往设置页填写 Provider、模型和凭据，再开始对话。
              </div>
            ) : null}

            <ChatInput
              isLoading={isLoading}
              disabled={!isConfigured}
              disabledHint={!isConfigured ? "未配置 AI" : undefined}
              showPresets={!hasMessages}
              onSubmit={sendMessage}
            />
          </div>
        </div>

        <aside className="min-h-0 overflow-y-auto pr-1">
          <div className="flex flex-col gap-4">
            <ReportViewer />

            <div className="grid gap-3 md:grid-cols-3 xl:grid-cols-1">
              {CAPABILITY_CARDS.map(({ title, description, icon: Icon }, index) => (
                <motion.article
                  key={title}
                  initial={{ opacity: 0, x: 18 }}
                  animate={{ opacity: 1, x: 0 }}
                  transition={{ delay: 0.08 * index, duration: 0.24, ease: "easeOut" }}
                  className="relative overflow-hidden rounded-[1.5rem] border border-border/70 bg-linear-to-br from-background to-muted/20 p-5 shadow-[0_24px_90px_-50px_rgba(15,23,42,0.45)]"
                >
                  <div className="pointer-events-none absolute -right-8 top-0 size-24 rounded-full bg-primary/10 blur-3xl" />
                  <div className="relative">
                    <div className="inline-flex size-10 items-center justify-center rounded-[1rem] bg-primary/10 text-primary">
                      <Icon className="size-5" />
                    </div>
                    <h3 className="mt-4 text-base font-semibold text-foreground">{title}</h3>
                    <p className="mt-2 text-sm leading-6 text-muted-foreground">{description}</p>
                  </div>
                </motion.article>
              ))}
            </div>

            <SnapshotList />
          </div>
        </aside>
      </div>
    </motion.section>
  );
}
