import jsQR from "jsqr";
import { CircleAlert, ClipboardCopy, Eraser, ImageDown, ImageUp, QrCode, ScanLine } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type DragEvent } from "react";
import { encode } from "uqr";
import { Button, cx, Segmented, TextArea } from "../../components/ui";
import { toolsApi } from "../../lib/api";
import { isMac } from "../../lib/format";
import { useI18n } from "../../lib/i18n";
import { useToast } from "../../lib/store";
import { CopyButton, ErrorText, Pane } from "./common";

type Mode = "generate" | "decode";
type Ecc = "L" | "M" | "Q" | "H";

const MODULE_PX = 8;
const BORDER = 2;
/** 识别前把大图缩到这个尺寸以内，太大的图识别很慢。 */
const DECODE_MAX = 1600;

const eccList: { value: Ecc; short: string }[] = [
  { value: "L", short: "L 7%" },
  { value: "M", short: "M 15%" },
  { value: "Q", short: "Q 25%" },
  { value: "H", short: "H 30%" },
];

function drawQr(text: string, ecc: Ecc): { canvas: HTMLCanvasElement; modules: number } {
  const { data, size } = encode(text, { ecc, border: BORDER });
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = size * MODULE_PX;
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "#000";
  data.forEach((row, y) => row.forEach((dark, x) => dark && ctx.fillRect(x * MODULE_PX, y * MODULE_PX, MODULE_PX, MODULE_PX)));
  return { canvas, modules: size - BORDER * 2 };
}

const toBlob = (canvas: HTMLCanvasElement) =>
  new Promise<Blob>((resolve, reject) => canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("toBlob"))), "image/png"));

function Generate({ text, setText, ecc }: { text: string; setText: (v: string) => void; ecc: Ecc }) {
  const { t } = useI18n();
  const toast = useToast();
  const qr = useMemo<{ canvas: HTMLCanvasElement; modules: number; url: string } | { error: true } | null>(() => {
    if (!text) return null;
    try {
      const { canvas, modules } = drawQr(text, ecc);
      return { canvas, modules, url: canvas.toDataURL("image/png") };
    } catch {
      return { error: true };
    }
  }, [text, ecc]);
  const ok = qr && "canvas" in qr ? qr : null;
  const tooLong = Boolean(qr && "error" in qr);
  const byteLen = useMemo(() => (text ? new TextEncoder().encode(text).length : 0), [text]);

  const copy = async () => {
    if (!ok) return;
    try {
      await navigator.clipboard.write([new ClipboardItem({ "image/png": toBlob(ok.canvas) })]);
      toast.success(t("tools.copied"));
    } catch (e) {
      toast.error(e);
    }
  };
  const save = async () => {
    if (!ok) return;
    try {
      const data = new Uint8Array(await (await toBlob(ok.canvas)).arrayBuffer());
      const path = await toolsApi.savePng("qrcode.png", data);
      if (path) toast.success(t("tools.qr.saved", { path }));
    } catch (e) {
      toast.error(e);
    }
  };

  return (
    <div className="grid min-h-0 flex-1 grid-cols-[1fr_236px] gap-4">
      <Pane
        title={t("tools.qr.content")}
        className="min-h-56"
        actions={
          <>
            {byteLen > 0 && <span className="mr-1 font-mono text-[11px] text-muted tabular-nums">{t("tools.qr.bytes", { count: byteLen })}</span>}
            <CopyButton text={text} />
          </>
        }
      >
        <TextArea
          className="flex-1"
          value={text}
          invalid={tooLong}
          autoFocus
          placeholder={t("tools.qr.contentPlaceholder")}
          onChange={(e) => setText(e.target.value)}
        />
        {tooLong && <ErrorText>{t("tools.qr.tooLong")}</ErrorText>}
      </Pane>

      <div className="flex flex-col justify-between gap-3 rounded-xl border border-line bg-gradient-to-b from-subtle/60 to-subtle/30 p-4">
        <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-2.5">
          {ok ? (
            <>
              <div className="rounded-2xl bg-white p-3.5 shadow-md ring-1 ring-black/6">
                <img src={ok.url} alt="" className="size-40 [image-rendering:pixelated]" />
              </div>
              <span className="font-mono text-[11px] text-muted tabular-nums">
                {ok.modules} × {ok.modules} · ECC {ecc}
              </span>
            </>
          ) : tooLong ? (
            <div className="flex size-44 flex-col items-center justify-center gap-2 rounded-2xl border border-dashed border-red-500/35 bg-red-500/5 px-4 text-center text-red-600 dark:text-red-400">
              <CircleAlert size={22} />
              <span className="text-xs leading-relaxed">{t("tools.qr.tooLong")}</span>
            </div>
          ) : (
            <div className="flex size-44 flex-col items-center justify-center gap-2.5 rounded-2xl border border-dashed border-line bg-surface/75 px-4 text-center shadow-2xs">
              <div className="flex size-10 items-center justify-center rounded-2xl bg-accent/10 text-accent">
                <QrCode size={20} />
              </div>
              <span className="text-xs leading-relaxed text-muted">{t("tools.qr.preview")}</span>
            </div>
          )}
        </div>

        <div className="grid grid-cols-2 gap-2">
          <Button variant="primary" size="sm" className="w-full px-2" icon={<ClipboardCopy size={13} />} disabled={!ok} onClick={copy}>
            {t("tools.qr.copyImage")}
          </Button>
          <Button size="sm" className="w-full px-2" icon={<ImageDown size={13} />} disabled={!ok} onClick={save}>
            {t("tools.qr.saveImage")}
          </Button>
        </div>
      </div>
    </div>
  );
}

async function decodeImage(blob: Blob): Promise<{ text: string | null; preview: string }> {
  const bitmap = await createImageBitmap(blob);
  const scale = Math.min(1, DECODE_MAX / Math.max(bitmap.width, bitmap.height));
  const w = Math.max(1, Math.round(bitmap.width * scale));
  const h = Math.max(1, Math.round(bitmap.height * scale));
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, w, h);
  ctx.drawImage(bitmap, 0, 0, w, h);
  bitmap.close();
  const preview = canvas.toDataURL("image/png");
  const code = jsQR(ctx.getImageData(0, 0, w, h).data, w, h, { inversionAttempts: "attemptBoth" });
  if (!code) return { text: null, preview };
  try {
    return { text: new TextDecoder("utf-8", { fatal: true }).decode(new Uint8Array(code.binaryData)), preview };
  } catch {
    return { text: code.data, preview };
  }
}

function Decode({
  image,
  result,
  onPick,
  onLoad,
}: {
  image: string | null;
  result: { text: string | null } | null;
  onPick: () => void;
  onLoad: (blob: Blob | null | undefined) => void;
}) {
  const { t, tx } = useI18n();
  const [dragging, setDragging] = useState(false);

  useEffect(() => {
    const onPaste = (e: ClipboardEvent) => {
      const file = Array.from(e.clipboardData?.files ?? []).find((f) => f.type.startsWith("image/"));
      if (!file) return;
      e.preventDefault();
      onLoad(file);
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, [onLoad]);

  const onDrop = (e: DragEvent) => {
    e.preventDefault();
    setDragging(false);
    const file = Array.from(e.dataTransfer.files).find((f) => f.type.startsWith("image/"));
    if (file) onLoad(file);
  };

  const shortcutKey = (
    <kbd className="rounded border border-line bg-surface px-1.5 py-0.5 font-mono text-[11px] font-medium text-ink shadow-2xs">
      {isMac ? "⌘V" : "Ctrl+V"}
    </kbd>
  );

  return (
    <div className="grid min-h-0 flex-1 grid-cols-2 gap-4">
      <Pane title={t("tools.qr.image")} className="min-h-56">
        <div
          role="button"
          tabIndex={0}
          onClick={onPick}
          onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), onPick())}
          onDragOver={(e) => {
            e.preventDefault();
            setDragging(true);
          }}
          onDragLeave={(e) => {
            if (!e.currentTarget.contains(e.relatedTarget as Node)) setDragging(false);
          }}
          onDrop={onDrop}
          className={cx(
            "group relative flex min-h-0 flex-1 cursor-pointer flex-col items-center justify-center overflow-hidden rounded-xl transition-colors outline-none focus-visible:ring-3 focus-visible:ring-accent/20",
            image
              ? "border border-line bg-subtle/45 p-3 hover:border-accent/50"
              : "border-2 border-dashed border-line bg-subtle/30 p-6 text-center hover:border-accent/50 hover:bg-subtle/60",
            dragging && "border-accent bg-accent/8 ring-3 ring-accent/15",
          )}
        >
          {image ? (
            <>
              <img src={image} alt="" className="max-h-full max-w-full rounded-lg object-contain shadow-xs" />
              <div className="pointer-events-none absolute bottom-3 flex items-center gap-1.5 rounded-full bg-ink/80 px-3 py-1 text-xs font-medium text-surface opacity-0 backdrop-blur-xs transition-opacity group-hover:opacity-100">
                <ImageUp size={13} />
                {t("tools.qr.pickImage")}
              </div>
            </>
          ) : (
            <div className="flex flex-col items-center gap-2.5">
              <div className="flex size-11 items-center justify-center rounded-2xl bg-accent/10 text-accent">
                <ImageUp size={20} />
              </div>
              <div className="text-[13px] font-medium text-ink">{t("tools.qr.dropHint")}</div>
              <div className="flex flex-wrap items-center justify-center gap-1 text-xs text-muted">
                {tx("tools.qr.pasteShortcut", { keys: shortcutKey })}
              </div>
            </div>
          )}
        </div>
      </Pane>

      <Pane title={t("tools.result")} className="min-h-56" actions={<CopyButton text={result?.text ?? ""} />}>
        {!image ? (
          <div className="flex flex-1 flex-col items-center justify-center gap-2.5 rounded-xl border border-line bg-subtle/25 px-6 text-center text-muted">
            <div className="flex size-11 items-center justify-center rounded-2xl bg-subtle text-muted/70">
              <ScanLine size={20} />
            </div>
            <span className="text-xs">{t("tools.qr.resultHint")}</span>
          </div>
        ) : result?.text === null ? (
          <div className="flex flex-1 flex-col items-center justify-center gap-2.5 rounded-xl border border-red-500/25 bg-red-500/5 px-6 text-center">
            <div className="flex size-10 items-center justify-center rounded-2xl bg-red-500/10 text-red-600 dark:text-red-400">
              <CircleAlert size={20} />
            </div>
            <span className="text-[13px] font-medium text-red-600 dark:text-red-400">{t("tools.qr.notFound")}</span>
          </div>
        ) : (
          <TextArea className="flex-1" readOnly value={result?.text ?? ""} />
        )}
      </Pane>
    </div>
  );
}

export default function QrTool() {
  const { t } = useI18n();
  const toast = useToast();
  const [mode, setMode] = useState<Mode>("generate");
  const [text, setText] = useState("");
  const [ecc, setEcc] = useState<Ecc>("M");
  const [image, setImage] = useState<string | null>(null);
  const [result, setResult] = useState<{ text: string | null } | null>(null);
  const picker = useRef<HTMLInputElement>(null);

  const load = async (blob: Blob | null | undefined) => {
    if (!blob || !blob.type.startsWith("image/")) return;
    try {
      const decoded = await decodeImage(blob);
      setImage(decoded.preview);
      setResult({ text: decoded.text });
    } catch (e) {
      toast.error(e);
    }
  };

  return (
    <div className="flex h-full flex-col gap-3">
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        <Segmented
          value={mode}
          onChange={setMode}
          options={[
            { value: "generate", label: t("tools.qr.generate") },
            { value: "decode", label: t("tools.qr.decode") },
          ]}
        />
        <div className="flex-1" />
        {mode === "generate" ? (
          <>
            <span className="text-xs text-muted">{t("tools.qr.ecc")}</span>
            <Segmented<Ecc>
              value={ecc}
              onChange={setEcc}
              options={eccList.map((o) => ({
                value: o.value,
                label: (
                  <span title={t(`tools.qr.ecc.${o.value}`)} className="font-mono text-xs tabular-nums">
                    {o.short}
                  </span>
                ),
              }))}
            />
            <Button size="sm" icon={<Eraser size={13} />} disabled={!text} onClick={() => setText("")}>
              {t("tools.clear")}
            </Button>
          </>
        ) : (
          <>
            <Button size="sm" icon={<ImageUp size={13} />} onClick={() => picker.current?.click()}>
              {t("tools.qr.pickImage")}
            </Button>
            <Button
              size="sm"
              icon={<Eraser size={13} />}
              disabled={!image}
              onClick={() => {
                setImage(null);
                setResult(null);
              }}
            >
              {t("tools.clear")}
            </Button>
            <input
              ref={picker}
              type="file"
              accept="image/*"
              hidden
              onChange={(e) => {
                load(e.target.files?.[0]);
                e.target.value = "";
              }}
            />
          </>
        )}
      </div>

      {mode === "generate" ? (
        <Generate text={text} setText={setText} ecc={ecc} />
      ) : (
        <Decode image={image} result={result} onPick={() => picker.current?.click()} onLoad={load} />
      )}
    </div>
  );
}
