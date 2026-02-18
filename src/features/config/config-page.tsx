import { useState, useEffect } from "react";
import { Save, RotateCw } from "lucide-react";
import { toast, Toaster } from "sonner";
import { useAppStore } from "@/stores/app-store";
import { useReadConfig, useWriteConfig, useReloadConfig } from "./hooks/use-config";
import { ConfigEditor } from "./config-editor";

export function ConfigPage() {
  const configDir = useAppStore((s) => s.mihomoConfigDir);
  const apiAddress = useAppStore((s) => s.apiAddress);
  const configPath = `${configDir}/config.yaml`;

  const { data: savedContent, isLoading, error } = useReadConfig(configPath);
  const writeMut = useWriteConfig();
  const reloadMut = useReloadConfig();

  const [content, setContent] = useState("");
  const [dirty, setDirty] = useState(false);

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
    try {
      await writeMut.mutateAsync({ path: configPath, content });
      await reloadMut.mutateAsync(apiAddress);
      setDirty(false);
      toast.success("配置已保存并重载");
    } catch (err) {
      toast.error(`保存失败: ${err}`);
    }
  };

  if (isLoading) {
    return <div className="p-4 text-sm text-muted-foreground">加载中...</div>;
  }

  if (error) {
    return (
      <div className="p-4 text-sm text-destructive">
        加载失败: {error.message}
      </div>
    );
  }

  const saving = writeMut.isPending || reloadMut.isPending;

  return (
    <div className="flex h-full flex-col">
      <Toaster position="top-center" richColors />
      <div className="flex items-center justify-between border-b border-border px-4 py-2">
        <span className="text-sm text-muted-foreground">{configPath}</span>
        <button
          disabled={!dirty || saving}
          onClick={handleSave}
          className="inline-flex items-center gap-1.5 rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground disabled:opacity-50"
        >
          {saving ? <RotateCw size={14} className="animate-spin" /> : <Save size={14} />}
          {saving ? "保存中..." : "保存并重载"}
        </button>
      </div>
      <div className="flex-1">
        <ConfigEditor value={content} onChange={handleChange} />
      </div>
    </div>
  );
}
