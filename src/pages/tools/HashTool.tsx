import { CircleAlert, CircleCheck, Eraser, FileCheck2, FileUp, Hash } from "lucide-react";
import { useEffect, useRef, useState, type DragEvent } from "react";
import { Button, cx, Segmented, TextArea } from "../../components/ui";
import { formatBytes } from "../../lib/format";
import { useI18n } from "../../lib/i18n";
import { useToast } from "../../lib/store";
import { digest, hashAlgos, type HashAlgo } from "../../lib/tools";
import { CopyButton, FilterChip } from "./common";

type Source = "text" | "file";

const algoBits: Record<HashAlgo, number> = {
  MD5: 128,
  "SHA-1": 160,
  "SHA-256": 256,
  "SHA-384": 384,
  "SHA-512": 512,
};

export default function HashTool() {
  const { t } = useI18n();
  const toast = useToast();
  const [source, setSource] = useState<Source>("text");
  const [text, setText] = useState("");
  const [file, setFile] = useState<{ name: string; size: number; data: Uint8Array<ArrayBuffer> } | null>(null);
  const [upper, setUpper] = useState(false);
  const [verify, setVerify] = useState("");
  const [dragging, setDragging] = useState(false);
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

  const onDrop = (e: DragEvent) => {
    e.preventDefault();
    setDragging(false);
    pick(e.dataTransfer.files[0]);
  };

  const expected = verify.trim().toLowerCase();
  const matchedAlgo = expected ? hashAlgos.find((a) => hashes[a]?.toLowerCase() === expected) : undefined;
  const hasInput = source === "text" ? !!text : !!file;

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <Segmented
          value={source}
          onChange={setSource}
          options={[
            { value: "text", label: t("tools.hash.text") },
            { value: "file", label: t("tools.hash.file") },
          ]}
        />
        <div className="flex-1" />
        <FilterChip active={upper} onClick={() => setUpper((v) => !v)}>
          {t("tools.uppercase")}
        </FilterChip>
        <Button
          size="sm"
          icon={<Eraser size={13} />}
          disabled={!hasInput}
          onClick={() => (source === "text" ? setText("") : setFile(null))}
        >
          {t("tools.clear")}
        </Button>
      </div>

      {source === "text" ? (
        <TextArea className="h-28 shrink-0" value={text} placeholder={t("tools.input")} autoFocus onChange={(e) => setText(e.target.value)} />
      ) : (
        <div
          role="button"
          tabIndex={0}
          onClick={() => picker.current?.click()}
          onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), picker.current?.click())}
          onDragOver={(e) => {
            e.preventDefault();
            setDragging(true);
          }}
          onDragLeave={(e) => {
            if (!e.currentTarget.contains(e.relatedTarget as Node)) setDragging(false);
          }}
          onDrop={onDrop}
          className={cx(
            "shrink-0 cursor-pointer rounded-xl transition-colors outline-none focus-visible:ring-3 focus-visible:ring-accent/20",
            file
              ? "flex items-center gap-3.5 border border-line bg-subtle/40 px-4 py-3 hover:border-accent/50"
              : "flex h-28 flex-col items-center justify-center gap-2 border-2 border-dashed border-line bg-subtle/30 px-4 text-center hover:border-accent/50 hover:bg-subtle/60",
            dragging && "border-accent bg-accent/8 ring-3 ring-accent/15",
          )}
        >
          {file ? (
            <>
              <div className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-emerald-500/12 text-emerald-600 dark:text-emerald-400">
                <FileCheck2 size={20} />
              </div>
              <div className="min-w-0 flex-1">
                <div className="truncate text-[13px] font-medium text-ink">{file.name}</div>
                <div className="mt-0.5 font-mono text-xs text-muted tabular-nums">{formatBytes(file.size)}</div>
              </div>
              <Button
                size="sm"
                icon={<FileUp size={13} />}
                onClick={(e) => {
                  e.stopPropagation();
                  picker.current?.click();
                }}
              >
                {t("tools.hash.changeFile")}
              </Button>
            </>
          ) : (
            <>
              <div className="flex size-9 items-center justify-center rounded-xl bg-accent/10 text-accent">
                <FileUp size={18} />
              </div>
              <div className="text-[13px] font-medium text-ink">{t("tools.hash.dropHint")}</div>
              <div className="text-xs text-muted">{t("tools.hash.dropSub")}</div>
            </>
          )}
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

      <div className="overflow-hidden rounded-xl border border-line bg-surface">
        <div className="flex flex-wrap items-center gap-2 border-b border-line bg-subtle/40 px-3.5 py-2">
          <span className="flex size-6 shrink-0 items-center justify-center rounded-md bg-accent/10 text-accent">
            <Hash size={13} />
          </span>
          <span className="text-xs font-semibold whitespace-nowrap text-ink">{t("tools.result")}</span>

          <div className="ml-auto flex items-center gap-2">
            {expected && hasInput && (
              <span
                className={cx(
                  "inline-flex items-center gap-1 rounded-md px-2 py-0.5 text-[11px] font-medium whitespace-nowrap",
                  matchedAlgo
                    ? "bg-emerald-500/12 text-emerald-600 dark:text-emerald-400"
                    : "bg-red-500/12 text-red-600 dark:text-red-400",
                )}
              >
                {matchedAlgo ? <CircleCheck size={12} /> : <CircleAlert size={12} />}
                {matchedAlgo ? t("tools.hash.matched", { algo: matchedAlgo }) : t("tools.hash.unmatched")}
              </span>
            )}
            <input
              spellCheck={false}
              autoComplete="off"
              value={verify}
              placeholder={t("tools.hash.verifyPlaceholder")}
              onChange={(e) => setVerify(e.target.value)}
              className="h-7 w-48 rounded-lg border border-line bg-surface px-2.5 font-mono text-xs text-ink outline-none transition-colors placeholder:font-sans placeholder:text-muted/65 focus:border-accent focus:ring-2 focus:ring-accent/15"
            />
          </div>
        </div>

        <div className="divide-y divide-line">
          {hashAlgos.map((a) => {
            const raw = hashes[a] ?? "";
            const shown = upper ? raw.toUpperCase() : raw;
            const matched = Boolean(expected && raw && raw.toLowerCase() === expected);
            return (
              <div
                key={a}
                className={cx(
                  "flex min-h-11 items-center gap-3 px-3.5 py-2 transition-colors",
                  matched ? "bg-emerald-500/[0.08] dark:bg-emerald-500/[0.14]" : "hover:bg-subtle/30",
                )}
              >
                <div className="w-24 shrink-0">
                  <div className="flex items-center gap-1.5">
                    <span className="font-mono text-xs font-semibold text-ink">{a}</span>
                    {matched && <CircleCheck size={12} className="text-emerald-500" />}
                  </div>
                  <div className="font-mono text-[10px] text-muted">{algoBits[a]}-bit</div>
                </div>
                <span className="selectable min-w-0 flex-1 font-mono text-[12.5px] break-all text-ink">{shown || "-"}</span>
                <CopyButton text={shown} />
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
