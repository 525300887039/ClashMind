import { useRef } from "react";
import Editor, { loader, type OnMount } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import { useTheme } from "@/hooks/use-theme";

loader.config({ monaco });

interface ConfigEditorProps {
  value: string;
  onChange: (value: string) => void;
}

export function ConfigEditor({ value, onChange }: ConfigEditorProps) {
  const { resolvedTheme } = useTheme();
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);

  const handleMount: OnMount = (editor) => {
    editorRef.current = editor;
  };

  return (
    <Editor
      height="100%"
      language="yaml"
      theme={resolvedTheme === "dark" ? "vs-dark" : "vs"}
      value={value}
      onChange={(v) => onChange(v ?? "")}
      onMount={handleMount}
      options={{
        minimap: { enabled: false },
        wordWrap: "on",
        tabSize: 2,
        fontSize: 13,
        scrollBeyondLastLine: false,
      }}
    />
  );
}
