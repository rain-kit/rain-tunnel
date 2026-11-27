import { FileUp } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Button, Card, Segmented, Switch, TextArea } from "../../components/ui";
import { formatBytes } from "../../lib/format";
import { useI18n } from "../../lib/i18n";
import { useToast } from "../../lib/store";
import { digest, hashAlgos, type HashAlgo } from "../../lib/tools";
import { Pane, ResultRow } from "./common";

type Source = "text" | "file";

export default function HashTool() {
  const { t } = useI18n();
  const toast = useToast();
  const [source, setSource] = useState<Source>("text");
  const [text, setText] = useState("");
  const [file, setFile] = useState<{ name: string; size: number; data: Uint8Array<ArrayBuffer> } | null>(null);
  const [upper, setUpper] = useState(false);
  const [hashes, setHashes] = useState<Partial<Record<HashAlgo, string>>>({});
  const picker = useRef<HTMLInputElement>(null);

  const data = source === "text" ? (text ? new TextEncoder().encode(text) : null) : (file?.data ?? null);
  const key = source === "text" ? text : file;

  useEffect(() => {
    if (!data) return setHashes({});
    let stale = false;
    Promise.all(hashAlgos.map(async (a) => [a, await digest(a, data)] as const))
      .then((list) => !stale && setHashes(Object.fromEntries(list)))
      .catch(toast.error);
    return () => {
      stale = true;
    };
  }, [source, key]);

  const pick = async (f: File | undefined) => {
    if (!f) return;
    try {
      setFile({ name: f.name, size: f.size, data: new Uint8Array(await f.arrayBuffer()) });
    } catch (e) {
      toast.error(e);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center gap-2">
        <Segmented
          value={source}
          onChange={setSource}
          options={[
            { value: "text", label: t("tools.hash.text") },
            { value: "file", label: t("tools.hash.file") },
          ]}
        />
        <div className="flex-1" />
        <span className="text-[13px]">{t("tools.uppercase")}</span>
        <Switch checked={upper} onChange={setUpper} label={t("tools.uppercase")} />
      </div>
      {source === "text" ? (
        <TextArea className="h-32" value={text} autoFocus onChange={(e) => setText(e.target.value)} />
      ) : (
        <div className="flex items-center gap-3">
          <Button icon={<FileUp size={14} />} onClick={() => picker.current?.click()}>
            {t("tools.hash.pickFile")}
          </Button>
          <span className="min-w-0 truncate text-xs text-muted">
            {file ? t("tools.hash.fileInfo", { name: file.name, size: formatBytes(file.size) }) : t("tools.hash.noFile")}
          </span>
          <input
            ref={picker}
            type="file"
            hidden
            onChange={(e) => {
              pick(e.target.files?.[0]);
              e.target.value = "";
            }}
          />
        </div>
      )}
      <Pane title={t("tools.result")}>
        <Card className="px-4">
          {hashAlgos.map((a) => {
            const v = hashes[a] ?? "";
            return <ResultRow key={a} label={a} value={upper ? v.toUpperCase() : v} />;
          })}
        </Card>
      </Pane>
    </div>
  );
}
