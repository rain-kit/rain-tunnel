import { ArrowLeftRight, CircleAlert, Copy, Eraser } from "lucide-react";
import { useMemo, useState, type ReactNode } from "react";
import { Button, cx, IconButton, Segmented, TextArea } from "../../components/ui";
import { useI18n } from "../../lib/i18n";
import { useToast } from "../../lib/store";
import type { MessageKey } from "../../locales/zh-CN";

export function useCopy() {
  const { t } = useI18n();
  const toast = useToast();
  return async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toast.success(t("tools.copied"));
    } catch (e) {
      toast.error(e);
    }
  };
}

export function CopyButton({ text }: { text: string }) {
  const { t } = useI18n();
  const copy = useCopy();
  return (
    <IconButton label={t("tools.copy")} size="sm" disabled={!text} onClick={() => copy(text)}>
      <Copy size={14} />
    </IconButton>
  );
}

/** 工具栏开关筛选按钮。 */
export function FilterChip({
  active,
  size = "md",
  onClick,
  children,
}: {
  active: boolean;
  size?: "sm" | "md";
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      className={cx(
        "inline-flex items-center gap-1 rounded-lg border font-medium whitespace-nowrap transition-colors",
        size === "sm" ? "h-6.5 px-2 text-[11px]" : "h-7 px-2.5 text-xs",
        active
          ? "border-accent/40 bg-accent/12 text-accent dark:bg-accent/25 dark:text-sky-300"
          : "border-line bg-surface text-muted hover:bg-subtle hover:text-ink",
      )}
    >
      {children}
    </button>
  );
}

/** 带标题的区块，标题右侧放操作按钮。 */
export function Pane({ title, actions, className, children }: { title: ReactNode; actions?: ReactNode; className?: string; children: ReactNode }) {
  return (
    <div className={cx("flex min-h-0 min-w-0 flex-col gap-1.5", className)}>
      <div className="flex h-7 shrink-0 items-center gap-1">
        <span className="flex-1 truncate text-xs font-medium text-muted">{title}</span>
        {actions}
      </div>
      {children}
    </div>
  );
}

/** 一行只读结果，右侧带复制按钮。 */
export function ResultRow({ label, value, extra }: { label: ReactNode; value: string; extra?: ReactNode }) {
  return (
    <div className="flex min-h-10 items-center gap-3 border-b border-line py-1 last:border-0">
      <span className="w-32 shrink-0 text-xs text-muted">{label}</span>
      <span className="selectable min-w-0 flex-1 font-mono text-[13px] break-all">{value || "-"}</span>
      {extra}
      <CopyButton text={value} />
    </div>
  );
}

export function ErrorText({ children }: { children: ReactNode }) {
  return <div className="shrink-0 text-xs text-red-600 dark:text-red-400">{children}</div>;
}

export interface Mode {
  value: string;
  label: MessageKey;
  run: (input: string) => string;
  /** 出错时显示这条文案，不设置则显示原始错误。 */
  error?: MessageKey;
}

/** 左边输入、右边实时输出的转换工具。 */
export function Converter({ modes, placeholder }: { modes: Mode[]; placeholder?: string }) {
  const { t } = useI18n();
  const [mode, setMode] = useState(modes[0].value);
  const [input, setInput] = useState("");
  const current = modes.find((m) => m.value === mode)!;
  const result = useMemo(() => {
    if (!input) return { output: "" };
    try {
      return { output: current.run(input) };
    } catch (e) {
      return { output: "", error: e instanceof Error ? e.message : String(e) };
    }
  }, [input, current]);

  return (
    <div className="flex h-full flex-col gap-3">
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        <Segmented value={mode} onChange={setMode} options={modes.map((m) => ({ value: m.value, label: t(m.label) }))} />
        <div className="flex-1" />
        <Button size="sm" icon={<ArrowLeftRight size={13} />} disabled={!result.output} onClick={() => setInput(result.output)}>
          {t("tools.useOutput")}
        </Button>
        <Button size="sm" icon={<Eraser size={13} />} disabled={!input} onClick={() => setInput("")}>
          {t("tools.clear")}
        </Button>
      </div>
      <div className="grid min-h-56 flex-1 grid-cols-2 gap-3.5">
        <Pane
          title={t("tools.input")}
          actions={
            <>
              {input.length > 0 && (
                <span className="mr-1 font-mono text-[11px] text-muted tabular-nums">{t("tools.chars", { count: input.length })}</span>
              )}
              <CopyButton text={input} />
            </>
          }
        >
          <TextArea className="flex-1" value={input} placeholder={placeholder} autoFocus onChange={(e) => setInput(e.target.value)} />
        </Pane>
        <Pane
          title={t("tools.output")}
          actions={
            <>
              {result.output.length > 0 && (
                <span className="mr-1 font-mono text-[11px] text-muted tabular-nums">{t("tools.chars", { count: result.output.length })}</span>
              )}
              <CopyButton text={result.output} />
            </>
          }
        >
          {!input ? (
            <div className="flex flex-1 flex-col items-center justify-center gap-2.5 rounded-xl border border-dashed border-line bg-subtle/25 px-6 text-center text-muted">
              <div className="flex size-10 items-center justify-center rounded-2xl bg-subtle text-muted/70">
                <ArrowLeftRight size={18} />
              </div>
              <span className="text-xs">{t("tools.outputHint")}</span>
            </div>
          ) : result.error ? (
            <div className="flex flex-1 flex-col items-center justify-center gap-2.5 rounded-xl border border-red-500/25 bg-red-500/[0.04] p-6 text-center">
              <div className="flex size-10 items-center justify-center rounded-2xl bg-red-500/10 text-red-600 dark:text-red-400">
                <CircleAlert size={20} />
              </div>
              <span className="max-w-xs text-xs leading-relaxed break-words text-red-600 dark:text-red-400">
                {current.error ? t(current.error) : t("tools.invalid", { cause: result.error })}
              </span>
            </div>
          ) : (
            <TextArea className="flex-1 bg-subtle/30" readOnly value={result.output} />
          )}
        </Pane>
      </div>
    </div>
  );
}
