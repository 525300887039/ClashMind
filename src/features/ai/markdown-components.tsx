import type { Components } from "react-markdown";

export const markdownComponents: Components = {
  p: ({ children }) => <p className="mb-3 last:mb-0">{children}</p>,
  ul: ({ children }) => <ul className="mb-3 list-disc space-y-1.5 pl-5 last:mb-0">{children}</ul>,
  ol: ({ children }) => (
    <ol className="mb-3 list-decimal space-y-1.5 pl-5 last:mb-0">{children}</ol>
  ),
  li: ({ children }) => <li className="leading-7">{children}</li>,
  blockquote: ({ children }) => (
    <blockquote className="mb-3 border-l-2 border-primary/35 pl-4 text-muted-foreground italic last:mb-0">
      {children}
    </blockquote>
  ),
  a: ({ children, href }) => (
    <a
      href={href}
      target="_blank"
      rel="noreferrer"
      className="font-medium text-primary underline decoration-primary/40 underline-offset-4 transition-colors hover:text-primary/80"
    >
      {children}
    </a>
  ),
  code: ({ children }) => (
    <code className="rounded-md bg-muted/70 px-1.5 py-0.5 font-mono text-[0.82em] text-primary">
      {children}
    </code>
  ),
  pre: ({ children }) => (
    <pre className="mb-3 overflow-x-auto rounded-[1rem] border border-white/10 bg-slate-950/92 p-4 text-[13px] leading-6 text-slate-100 shadow-[inset_0_1px_0_rgba(255,255,255,0.05)] last:mb-0 [&>code]:bg-transparent [&>code]:p-0 [&>code]:text-inherit">
      {children}
    </pre>
  ),
  table: ({ children }) => (
    <div className="mb-3 overflow-x-auto rounded-[1rem] border border-border/70 last:mb-0">
      <table className="min-w-full border-collapse text-sm">{children}</table>
    </div>
  ),
  thead: ({ children }) => <thead className="bg-muted/50 text-left">{children}</thead>,
  th: ({ children }) => (
    <th className="border-b border-border/70 px-3 py-2 font-medium text-foreground">{children}</th>
  ),
  td: ({ children }) => <td className="border-b border-border/60 px-3 py-2">{children}</td>,
};
