import jsQR from "jsqr";
import { ClipboardCopy, ImageDown, ImageUp } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { encode } from "uqr";
import { Button, Card, Segmented, Select, TextArea } from "../../components/ui";
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

function drawQr(text: string, ecc: Ecc): HTMLCanvasElement {
  const { data, size } = encode(text, { ecc, border: BORDER });
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = size * MODULE_PX;
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.fillStyle = "#000";
  data.forEach((row, y) => row.forEach((dark, x) => dark && ctx.fillRect(x * MODULE_PX, y * MODULE_PX, MODULE_PX, MODULE_PX)));
  return canvas;
}

const toBlob = (canvas: HTMLCanvasElement) =>
  new Promise<Blob>((resolve, reject) => canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("toBlob"))), "image/png"));

function Generate() {
  const { t } = useI18n();
  const toast = useToast();
  const [text, setText] = useState("");
  const [ecc, setEcc] = useState<Ecc>("M");
  const qr = useMemo(() => {
    if (!text) return null;
    try {
      const canvas = drawQr(text, ecc);
      return { canvas, url: canvas.toDataURL("image/png") };
    } catch {
      return { error: true as const };
    }
  }, [text, ecc]);
  const canvas = qr && "canvas" in qr ? qr.canvas : null;

  const copy = async () => {
    if (!canvas) return;
    try {
      await navigator.clipboard.write([new ClipboardItem({ "image/png": toBlob(canvas) })]);
      toast.success(t("tools.copied"));
    } catch (e) {
      toast.error(e);
    }
  };
  const save = async () => {
    if (!canvas) return;
    try {
      const data = new Uint8Array(await (await toBlob(canvas)).arrayBuffer());
      const path = await toolsApi.savePng("qrcode.png", data);
      if (path) toast.success(t("tools.qr.saved", { path }));
    } catch (e) {
      toast.error(e);
    }
  };

  return (
    <div className="grid grid-cols-[1fr_auto] gap-4">
      <div className="flex min-w-0 flex-col gap-3">
        <Pane title={t("tools.qr.content")}>
          <TextArea className="h-48" value={text} autoFocus placeholder={t("tools.qr.contentPlaceholder")} onChange={(e) => setText(e.target.value)} />
          {qr && "error" in qr && <ErrorText>{t("tools.qr.tooLong")}</ErrorText>}
        </Pane>
        <div className="flex items-center gap-2">
          <span className="text-[13px]">{t("tools.qr.ecc")}</span>
          <Select<Ecc>
            className="w-44"
            value={ecc}
            onChange={setEcc}
            options={(["L", "M", "Q", "H"] as const).map((v) => ({ value: v, label: t(`tools.qr.ecc.${v}`) }))}
          />
        </div>
      </div>
      <div className="flex w-56 flex-col items-center gap-3">
        <Card className="flex size-56 items-center justify-center overflow-hidden bg-white p-2">
          {canvas ? (
            <img src={(qr as { url: string }).url} alt="" className="size-full [image-rendering:pixelated]" />
          ) : (
            <span className="px-4 text-center text-xs text-zinc-400">{t("tools.qr.preview")}</span>
          )}
        </Card>
        <div className="flex gap-2">
          <Button size="sm" icon={<ClipboardCopy size={13} />} disabled={!canvas} onClick={copy}>
            {t("tools.qr.copyImage")}
          </Button>
          <Button size="sm" icon={<ImageDown size={13} />} disabled={!canvas} onClick={save}>
            {t("tools.qr.saveImage")}
          </Button>
        </div>
      </div>
    </div>
  );
}

async function decodeImage(blob: Blob): Promise<string | null> {
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
  const code = jsQR(ctx.getImageData(0, 0, w, h).data, w, h, { inversionAttempts: "attemptBoth" });
  if (!code) return null;
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(new Uint8Array(code.binaryData));
  } catch {
    return code.data;
  }
}

function Decode() {
  const { t } = useI18n();
  const toast = useToast();
  const [image, setImage] = useState<string | null>(null);
  const [result, setResult] = useState<{ text: string | null } | null>(null);
  const picker = useRef<HTMLInputElement>(null);

  const load = async (blob: Blob | null | undefined) => {
    if (!blob || !blob.type.startsWith("image/")) return;
    setImage(URL.createObjectURL(blob));
    setResult(null);
    try {
      setResult({ text: await decodeImage(blob) });
    } catch (e) {
      toast.error(e);
    }
  };

  useEffect(() => {
    const onPaste = (e: ClipboardEvent) => {
      const file = Array.from(e.clipboardData?.files ?? []).find((f) => f.type.startsWith("image/"));
      if (!file) return;
      e.preventDefault();
      load(file);
    };
    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, []);

  useEffect(() => () => void (image && URL.revokeObjectURL(image)), [image]);

  return (
    <div className="grid grid-cols-[auto_1fr] gap-4">
      <div className="flex w-56 flex-col items-center gap-3">
        <Card className="flex size-56 items-center justify-center overflow-hidden p-2">
          {image ? (
            <img src={image} alt="" className="max-h-full max-w-full object-contain" />
          ) : (
            <span className="px-4 text-center text-xs text-muted">{t("tools.qr.pasteHint", { keys: isMac ? "⌘V" : "Ctrl+V" })}</span>
          )}
        </Card>
        <Button size="sm" icon={<ImageUp size={13} />} onClick={() => picker.current?.click()}>
          {t("tools.qr.pickImage")}
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
      </div>
      <Pane title={t("tools.result")} actions={<CopyButton text={result?.text ?? ""} />}>
        <TextArea className="h-56" readOnly value={result?.text ?? ""} invalid={result?.text === null} />
        {result?.text === null && <ErrorText>{t("tools.qr.notFound")}</ErrorText>}
      </Pane>
    </div>
  );
}

export default function QrTool() {
  const { t } = useI18n();
  const [mode, setMode] = useState<Mode>("generate");
  return (
    <div className="flex flex-col gap-3">
      <div>
        <Segmented
          value={mode}
          onChange={setMode}
          options={[
            { value: "generate", label: t("tools.qr.generate") },
            { value: "decode", label: t("tools.qr.decode") },
          ]}
        />
      </div>
      {mode === "generate" ? <Generate /> : <Decode />}
    </div>
  );
}
