import { motion } from "framer-motion";
import { cn } from "@/lib/utils";

export interface PageHeaderProps {
  eyebrow: string;
  eyebrowIcon: React.ComponentType<{ className?: string }>;
  title: string;
  description: string;
  actions?: React.ReactNode;
  children?: React.ReactNode;
}

export function PageHeader({
  eyebrow,
  eyebrowIcon: EyebrowIcon,
  title,
  description,
  actions,
  children,
}: PageHeaderProps) {
  return (
    <header
      className={cn(
        "relative overflow-hidden rounded-[2rem] border border-border/70",
        "bg-linear-to-br from-primary/12 via-background to-background p-6 shadow-lg",
      )}
    >
      <div className="pointer-events-none absolute -right-10 top-0 size-40 rounded-full bg-primary/12 blur-3xl" />
      <div className="pointer-events-none absolute bottom-0 left-0 h-24 w-56 bg-linear-to-r from-primary/10 to-transparent" />

      <div className="relative flex flex-col gap-6">
        <div className="flex flex-col gap-4 xl:flex-row xl:items-end xl:justify-between">
          <div className="max-w-3xl">
            <motion.div
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.22, ease: "easeOut" }}
              className="inline-flex items-center gap-2 rounded-full border border-primary/15 bg-primary/10 px-3 py-1 text-xs font-medium tracking-[0.18em] text-primary uppercase"
            >
              <EyebrowIcon className="size-3.5" />
              {eyebrow}
            </motion.div>
            <motion.h1
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.22, ease: "easeOut", delay: 0.06 }}
              className="mt-4 text-3xl font-semibold tracking-tight text-foreground"
            >
              {title}
            </motion.h1>
            <motion.p
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.22, ease: "easeOut", delay: 0.12 }}
              className="mt-2 max-w-2xl text-sm leading-6 text-muted-foreground"
            >
              {description}
            </motion.p>
          </div>

          {actions ? (
            <div className="flex flex-wrap items-center gap-2">{actions}</div>
          ) : null}
        </div>

        {children}
      </div>
    </header>
  );
}
