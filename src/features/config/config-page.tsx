import { useState, useEffect, lazy, Suspense } from "react";
import { FileCog, Save, RotateCw, Loader2 } from "lucide-react";
import { motion } from "framer-motion";
import { toast, Toaster } from "sonner";
import { api } from "@/lib/tauri-api";
import { useAppStore } from "@/stores/app-store";
import { PageHeader, SectionCard, ActionButton } from "@/components/ui";
import { useReadConfig, useWriteConfig, useReloadConfig } from "./hooks/use-config";

const ConfigEditor = lazy(() =>
  import("./config-editor").then((m) => ({ default: m.ConfigEditor }))
);

export function ConfigPage() {
  const configDir = useAppStore((s) => s.mihomoConfigDir);
  const configPath = `${configDir}/config.yaml`;

  const { data: savedContent, isLoading, error } = useReadConfig(configPath);
  const writeMut = useWriteConfig();
  const reloadMut = useReloadConfig();

  const [content, setContent] = useState("");
  const [dirty, setDirty] = useState(false);
  const [isSnapshotting, setIsSnapshotting] = useState(false);

  useEffect(() => {
    if (savedContent !== undefined) {
      setContent(savedContent);
      setDirty(false);
    }
  }, [savedContent]);

  const handleChange = (value: string) => {
    setContent(value);
    setDirty(value !== savedContent);
  };

  const handleSave = async () => {
    setIsSnapshotting(true);
    try {
      await api.ai.createSnapshot("手动保存前自动备份", configPath);
      await writeMut.mutateAsync({ path: configPath, content });
      await reloadMut.mutateAsync();
      setDirty(false);
      toast.success("配置已保存并重载");
    } catch (err) {
      toast.error(`保存失败: ${err}`);
    } finally {
      setIsSnapshotting(false);
    }
  };

  const saving = isSnapshotting || writeMut.isPending || reloadMut.isPending;

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="size-4 animate-spin" />
          加载配置中...
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-full items-center justify-center">
        <p className="text-sm text-destructive">加载失败: {error.message}</p>
      </div>
    );
  }

  return (
    <motion.section initial={{ opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.24, ease: "easeOut" }} className="flex h-full flex-col gap-6">
      <Toaster position="top-center" richColors />

      <PageHeader
        eyebrow="Config Editor"
        eyebrowIcon={FileCog}
        title="配置"
        description="编辑 mihomo 核心配置文件，保存后自动重载生效"
        actions={
          <>
            <span className="rounded-full border border-border/70 bg-muted/30 px-3 py-1.5 text-xs font-mono text-muted-foreground">
              {configPath}
            </span>

            {dirty && (
              <span className="inline-flex items-center gap-1.5 text-xs text-amber-500">
                <span className="size-2 rounded-full bg-amber-500" />
                已修改
              </span>
            )}

            <ActionButton
              tone="primary"
              disabled={!dirty || saving}
              onClick={handleSave}
            >
              {saving ? (
                <RotateCw className="size-3.5 animate-spin" />
              ) : (
                <Save className="size-3.5" />
              )}
              {saving ? "保存中..." : "保存并重载"}
            </ActionButton>
          </>
        }
      />

      <SectionCard className="min-h-0 flex-1 overflow-hidden p-0">
        <Suspense
          fallback={
            <div className="flex h-full items-center justify-center">
              <div className="flex items-center gap-2 text-sm text-muted-foreground">
                <Loader2 className="size-4 animate-spin" />
                编辑器加载中...
              </div>
            </div>
          }
        >
          <ConfigEditor value={content} onChange={handleChange} />
        </Suspense>
      </SectionCard>
    </motion.section>
  );
}
