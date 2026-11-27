import {
  ArrowLeftRight,
  ChevronDown,
  ChevronUp,
  ChevronsUpDown,
  CircleAlert,
  CircleCheck,
  Columns2,
  Diff,
  Eraser,
  Rows2,
} from "lucide-react";
import { useDeferredValue, useEffect, useMemo, useState } from "react";
import { Button, cx, TextArea } from "../../components/ui";
import { useI18n } from "../../lib/i18n";
import { compareText, splitLines, type DiffCell, type DiffPart, type DiffRow } from "../../lib/textDiff";
import { CopyButton, FilterChip, Pane } from "./common";

type ViewMode = "split" | "unified";
type Tone = "same" | "remove" | "add";

const CONTEXT = 2;

const rowTone: Record<Tone, { row: string; num: string; sign: string; mark: string; char: string }> = {
  same: {
    row: "hover:bg-subtle/40",
    num: "bg-subtle/30 text-muted/70",
    sign: "text-muted/50",
    mark: "",
    char: " ",
  },
  remove: {
    row: "bg-red-500/[0.07] dark:bg-red-500/[0.11]",
    num: "bg-red-500/15 text-red-700/80 dark:bg-red-500/22 dark:text-red-300/85",
    sign: "text-red-600 dark:text-red-400",
    mark: "bg-red-500/25 text-red-950 dark:bg-red-500/40 dark:text-red-100",
    char: "-",
  },
  add: {
    row: "bg-emerald-500/[0.07] dark:bg-emerald-500/[0.11]",
    num: "bg-emerald-500/15 text-emerald-700/80 dark:bg-emerald-500/22 dark:text-emerald-300/85",
    sign: "text-emerald-600 dark:text-emerald-400",
    mark: "bg-emerald-500/25 text-emerald-950 dark:bg-emerald-500/40 dark:text-emerald-100",
    char: "+",
  },
};

function Parts({ parts, mark }: { parts: DiffPart[]; mark: string }) {
  return (
    <>
      {parts.map((p, i) => (
        <span key={i} className={p.changed ? cx("rounded-[3px] px-[1px]", mark) : undefined}>
          {p.text}
        </span>
      ))}
      {parts.every((p) => !p.text) && "\u00a0"}
    </>
  );
}

function SplitCell({ cell, tone }: { cell?: DiffCell; tone: Tone }) {
  if (!cell) return <div className="diff-empty min-h-[22px] min-w-0" />;
  const s = rowTone[tone];
  return (
    <div className={cx("flex min-h-[22px] min-w-0", s.row)}>
      <span className={cx("w-10 shrink-0 border-r border-line/50 px-2 py-0.5 text-right text-[11px] tabular-nums select-none", s.num)}>
        {cell.no}
      </span>
      <span className={cx("w-5 shrink-0 py-0.5 text-center font-semibold select-none", s.sign)}>{s.char}</span>
      <span className="selectable min-w-0 flex-1 py-0.5 pr-3 break-all whitespace-pre-wrap">
        <Parts parts={cell.parts} mark={s.mark} />
      </span>
    </div>
  );
}

interface UnifiedLine {
  leftNo?: number;
  rightNo?: number;
  parts: DiffPart[];
  tone: Tone;
}

function UnifiedRow({ line }: { line: UnifiedLine }) {
  const s = rowTone[line.tone];
  return (
    <div className={cx("flex min-h-[22px] min-w-0", s.row)}>
      <span className={cx("w-10 shrink-0 border-r border-line/40 px-1.5 py-0.5 text-right text-[11px] tabular-nums select-none", s.num)}>
        {line.leftNo ?? ""}
      </span>
      <span className={cx("w-10 shrink-0 border-r border-line/50 px-1.5 py-0.5 text-right text-[11px] tabular-nums select-none", s.num)}>
        {line.rightNo ?? ""}
      </span>
      <span className={cx("w-5 shrink-0 py-0.5 text-center font-semibold select-none", s.sign)}>{s.char}</span>
      <span className="selectable min-w-0 flex-1 py-0.5 pr-3 break-all whitespace-pre-wrap">
        <Parts parts={line.parts} mark={s.mark} />
      </span>
    </div>
  );
}

type DisplayChunk = { type: "rows"; rows: DiffRow[] } | { type: "fold"; start: number; count: number };

function buildChunks(rows: DiffRow[], onlyChanges: boolean, expanded: Set<number>): DisplayChunk[] {
  if (!onlyChanges) return [{ type: "rows", rows }];
  const keep = rows.map((_, idx) => {
    for (let m = Math.max(0, idx - CONTEXT); m <= Math.min(rows.length - 1, idx + CONTEXT); m++) {
      if (rows[m].kind !== "same") return true;
    }
    return false;
  });

  const chunks: DisplayChunk[] = [];
  let i = 0;
  while (i < rows.length) {
    if (keep[i]) {
      const start = i;
      while (i < rows.length && keep[i]) i++;
      chunks.push({ type: "rows", rows: rows.slice(start, i) });
    } else {
      const start = i;
      while (i < rows.length && !keep[i]) i++;
      const count = i - start;
      if (count < 3 || expanded.has(start)) {
        chunks.push({ type: "rows", rows: rows.slice(start, i) });
      } else {
        chunks.push({ type: "fold", start, count });
      }
    }
  }
  return chunks;
}

function toUnifiedLines(rows: DiffRow[]): UnifiedLine[] {
  const out: UnifiedLine[] = [];
  let i = 0;
  while (i < rows.length) {
    const r = rows[i];
    if (r.kind === "change") {
      const start = i;
      while (i < rows.length && rows[i].kind === "change") i++;
      for (let k = start; k < i; k++) {
        const c = rows[k].left!;
        out.push({ leftNo: c.no, parts: c.parts, tone: "remove" });
      }
      for (let k = start; k < i; k++) {
        const c = rows[k].right!;
        out.push({ rightNo: c.no, parts: c.parts, tone: "add" });
      }
      continue;
    }
    if (r.kind === "same") out.push({ leftNo: r.left!.no, rightNo: r.right!.no, parts: r.left!.parts, tone: "same" });
    else if (r.kind === "remove") out.push({ leftNo: r.left!.no, parts: r.left!.parts, tone: "remove" });
    else if (r.kind === "add") out.push({ rightNo: r.right!.no, parts: r.right!.parts, tone: "add" });
    i++;
  }
  return out;
}

export default function DiffTool() {
  const { t } = useI18n();
  const [left, setLeft] = useState("");
  const [right, setRight] = useState("");
  const [view, setView] = useState<ViewMode>("split");
  const [ignoreWhitespace, setIgnoreWhitespace] = useState(false);
  const [ignoreCase, setIgnoreCase] = useState(false);
  const [onlyChanges, setOnlyChanges] = useState(false);
  const [collapsed, setCollapsed] = useState(false);
  const [expanded, setExpanded] = useState<Set<number>>(() => new Set());

  const a = useDeferredValue(left);
  const b = useDeferredValue(right);
  const result = useMemo(() => compareText(a, b, { ignoreWhitespace, ignoreCase }), [a, b, ignoreWhitespace, ignoreCase]);
  const empty = !left && !right;

  useEffect(() => setExpanded(new Set()), [a, b, ignoreWhitespace, ignoreCase, onlyChanges]);
  useEffect(() => {
    if (empty) setCollapsed(false);
  }, [empty]);

  const chunks = useMemo(() => buildChunks(result.rows, onlyChanges && !result.same, expanded), [result, onlyChanges, expanded]);
  const leftCount = splitLines(left).length;
  const rightCount = splitLines(right).length;

  const paneTitle = (label: string, count: number) => (
    <span className="inline-flex items-center gap-1.5 whitespace-nowrap">
      <span>{label}</span>
      <span className="rounded bg-subtle px-1.5 py-0.5 font-mono text-[11px] font-normal text-muted tabular-nums">
        {t("tools.diff.lines", { count })}
      </span>
    </span>
  );

  return (
    <div className="flex h-full flex-col gap-3">
      <div className="flex shrink-0 flex-wrap items-center gap-1.5">
        <FilterChip active={ignoreWhitespace} onClick={() => setIgnoreWhitespace((v) => !v)}>
          {t("tools.diff.ignoreWhitespace")}
        </FilterChip>
        <FilterChip active={ignoreCase} onClick={() => setIgnoreCase((v) => !v)}>
          {t("tools.diff.ignoreCase")}
        </FilterChip>

        <div className="flex-1" />

        {!empty && (
          <Button
            size="sm"
            variant="ghost"
            icon={collapsed ? <ChevronDown size={13} /> : <ChevronUp size={13} />}
            onClick={() => setCollapsed((v) => !v)}
          >
            {t(collapsed ? "tools.diff.expandInput" : "tools.diff.collapseInput")}
          </Button>
        )}
        <Button
          size="sm"
          icon={<ArrowLeftRight size={13} />}
          disabled={empty}
          onClick={() => {
            setLeft(right);
            setRight(left);
          }}
        >
          {t("tools.diff.swap")}
        </Button>
        <Button
          size="sm"
          icon={<Eraser size={13} />}
          disabled={empty}
          onClick={() => {
            setLeft("");
            setRight("");
          }}
        >
          {t("tools.clear")}
        </Button>
      </div>

      {!collapsed && (
        <div className={cx("grid grid-cols-2 gap-3", empty ? "min-h-48 flex-1" : "shrink-0")}>
          <Pane title={paneTitle(t("tools.diff.original"), leftCount)} actions={<CopyButton text={left} />}>
            <TextArea
              className={empty ? "flex-1" : "h-28"}
              value={left}
              autoFocus
              onChange={(e) => setLeft(e.target.value)}
            />
          </Pane>
          <Pane title={paneTitle(t("tools.diff.modified"), rightCount)} actions={<CopyButton text={right} />}>
            <TextArea className={empty ? "flex-1" : "h-28"} value={right} onChange={(e) => setRight(e.target.value)} />
          </Pane>
        </div>
      )}

      {empty ? (
        <div className="flex shrink-0 items-center justify-center gap-2 rounded-xl border border-dashed border-line bg-subtle/35 py-3.5 text-xs text-muted">
          <Diff size={14} className="text-muted/70" />
          <span>{t("tools.diff.hint")}</span>
        </div>
      ) : (
        <div className="flex min-h-56 flex-1 flex-col overflow-hidden rounded-xl border border-line bg-surface">
          <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-line bg-subtle/50 px-3 py-1.5 text-xs">
            {result.same ? (
              <span className="inline-flex items-center gap-1.5 font-medium text-emerald-700 dark:text-emerald-400">
                <CircleCheck size={14} />
                {t("tools.diff.same")}
              </span>
            ) : (
              <>
                <span className="inline-flex items-center gap-1.5 font-medium text-ink">
                  <CircleAlert size={14} className="shrink-0 text-amber-500" />
                  <span>{t("tools.diff.summary", { changed: result.changed, removed: result.removed, added: result.added })}</span>
                </span>
                <div className="flex items-center gap-1 font-mono text-[11px] font-semibold tabular-nums">
                  {result.added > 0 && (
                    <span className="rounded bg-emerald-500/12 px-1.5 py-0.5 text-emerald-600 dark:text-emerald-400">
                      +{result.added}
                    </span>
                  )}
                  {result.removed > 0 && (
                    <span className="rounded bg-red-500/12 px-1.5 py-0.5 text-red-600 dark:text-red-400">
                      -{result.removed}
                    </span>
                  )}
                  {result.changed > 0 && (
                    <span className="rounded bg-amber-500/12 px-1.5 py-0.5 text-amber-600 dark:text-amber-400">
                      ~{result.changed}
                    </span>
                  )}
                </div>
              </>
            )}

            <div className="ml-auto flex items-center gap-1.5">
              {!result.same && (
                <FilterChip size="sm" active={onlyChanges} onClick={() => setOnlyChanges((v) => !v)}>
                  {t("tools.diff.onlyChanges")}
                </FilterChip>
              )}
              <div className="inline-flex h-6.5 rounded-lg border border-line bg-surface p-0.5">
                {(
                  [
                    { value: "split", icon: Columns2, label: t("tools.diff.split") },
                    { value: "unified", icon: Rows2, label: t("tools.diff.unified") },
                  ] as const
                ).map(({ value, icon: Icon, label }) => (
                  <button
                    key={value}
                    type="button"
                    onClick={() => setView(value)}
                    className={cx(
                      "inline-flex items-center gap-1 rounded-md px-2 text-[11px] whitespace-nowrap transition-colors",
                      view === value ? "bg-subtle font-medium text-ink" : "text-muted hover:text-ink",
                    )}
                  >
                    <Icon size={12} />
                    {label}
                  </button>
                ))}
              </div>
            </div>
          </div>

          {result.rows.length > 0 && (
            <div className="min-h-0 flex-1 overflow-y-auto font-mono text-[12px] leading-[20px]">
              {chunks.map((chunk, ci) => {
                if (chunk.type === "fold") {
                  return (
                    <button
                      key={`fold-${chunk.start}`}
                      type="button"
                      onClick={() => setExpanded((prev) => new Set(prev).add(chunk.start))}
                      className="flex w-full items-center justify-center gap-1.5 border-y border-line/60 bg-subtle/60 py-1 font-sans text-[11px] text-muted transition-colors hover:bg-subtle hover:text-ink"
                    >
                      <ChevronsUpDown size={12} />
                      <span>{t("tools.diff.folded", { count: chunk.count })}</span>
                    </button>
                  );
                }

                if (view === "unified") {
                  return toUnifiedLines(chunk.rows).map((line, li) => <UnifiedRow key={`${ci}-${li}`} line={line} />);
                }

                return chunk.rows.map((r, ri) => {
                  const lt: Tone = r.kind === "change" || r.kind === "remove" ? "remove" : "same";
                  const rt: Tone = r.kind === "change" || r.kind === "add" ? "add" : "same";
                  return (
                    <div key={`${ci}-${ri}`} className="grid grid-cols-2 divide-x divide-line">
                      <SplitCell cell={r.left} tone={lt} />
                      <SplitCell cell={r.right} tone={rt} />
                    </div>
                  );
                });
              })}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
