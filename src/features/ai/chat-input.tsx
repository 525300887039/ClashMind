import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { ArrowUp, CornerDownLeft, LoaderCircle, Sparkles } from "lucide-react";
import { cn } from "@/lib/utils";

const PRESET_QUESTIONS = [
  "帮我分析最近的流量使用情况",
  "检查代理连通性",
  "优化我的路由规则",
] as const;

const MAX_TEXTAREA_HEIGHT = 220;

interface ChatInputProps {
  isLoading: boolean;
  showPresets: boolean;
  onSubmit: (message: string) => void;
}

export function ChatInput({ isLoading, showPresets, onSubmit }: ChatInputProps) {
  const [value, setValue] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => {
    const textarea = textareaRef.current;
    if (textarea === null) {
      return;
    }

    textarea.style.height = "auto";
    const nextHeight = Math.min(textarea.scrollHeight, MAX_TEXTAREA_HEIGHT);
    textarea.style.height = `${nextHeight}px`;
    textarea.style.overflowY = textarea.scrollHeight > MAX_TEXTAREA_HEIGHT ? "auto" : "hidden";
  }, [value]);

  const submit = () => {
    const nextValue = value.trim();
    if (!nextValue || isLoading) {
      return;
    }

    onSubmit(nextValue);
    setValue("");
  };

  const handleFormSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    submit();
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.nativeEvent.isComposing) {
      return;
    }

    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      submit();
    }
  };

  const handlePresetClick = (question: string) => {
    if (isLoading) {
      return;
    }

    onSubmit(question);
    setValue("");
  };

  return (
    <div className="rounded-[2rem] border border-border/70 bg-background/95 p-3 shadow-[0_24px_80px_-40px_rgba(15,23,42,0.45)]">
      {showPresets ? (
        <div className="mb-3 flex flex-wrap gap-2">
          {PRESET_QUESTIONS.map((question, index) => (
            <button
              key={question}
              type="button"
              onClick={() => handlePresetClick(question)}
              disabled={isLoading}
              className={cn(
                "group inline-flex items-center gap-2 rounded-full border px-3 py-2 text-left text-sm transition-all",
                "border-border/70 bg-muted/30 text-muted-foreground hover:border-primary/30 hover:bg-primary/10 hover:text-foreground",
                "disabled:cursor-not-allowed disabled:opacity-60",
              )}
            >
              <span className="inline-flex size-5 items-center justify-center rounded-full bg-primary/12 text-[11px] font-semibold text-primary">
                {index + 1}
              </span>
              <span>{question}</span>
            </button>
          ))}
        </div>
      ) : null}

      <form onSubmit={handleFormSubmit} className="space-y-3">
        <div className="relative overflow-hidden rounded-[1.5rem] border border-border/70 bg-muted/15">
          <div className="pointer-events-none absolute inset-x-6 top-0 h-px bg-linear-to-r from-transparent via-primary/25 to-transparent" />
          <textarea
            ref={textareaRef}
            value={value}
            onChange={(event) => setValue(event.target.value)}
            onKeyDown={handleKeyDown}
            rows={1}
            placeholder="描述你想调整的规则、诊断目标或统计问题..."
            className="min-h-[88px] w-full resize-none bg-transparent px-5 py-4 text-sm leading-7 text-foreground outline-none placeholder:text-muted-foreground"
          />

          <div className="flex items-center justify-between gap-3 border-t border-border/70 px-4 py-3">
            <div className="flex flex-wrap items-center gap-3 text-[11px] tracking-[0.18em] text-muted-foreground uppercase">
              <span className="inline-flex items-center gap-1.5">
                <Sparkles className="size-3.5" />
                Streaming Copilot
              </span>
              <span className="inline-flex items-center gap-1.5">
                <CornerDownLeft className="size-3.5" />
                Enter 发送
              </span>
              <span>Shift + Enter 换行</span>
            </div>

            <button
              type="submit"
              disabled={isLoading || value.trim().length === 0}
              className={cn(
                "inline-flex items-center gap-2 rounded-full px-4 py-2 text-sm font-medium transition-all",
                "bg-primary text-primary-foreground shadow-[0_16px_40px_-24px_var(--color-primary)]",
                "hover:translate-y-[-1px] disabled:translate-y-0 disabled:cursor-not-allowed disabled:opacity-60",
              )}
            >
              {isLoading ? (
                <LoaderCircle className="size-4 animate-spin" />
              ) : (
                <ArrowUp className="size-4" />
              )}
              <span>{isLoading ? "处理中" : "发送"}</span>
            </button>
          </div>
        </div>
      </form>
    </div>
  );
}
