import { ArrowLeftRight, CircleCheck, CircleAlert, Eraser } from "lucide-react";
import { useDeferredValue, useMemo, useState } from "react";
import { Button, Card, cx, Switch, TextArea } from "../../components/ui";
import { useI18n } from "../../lib/i18n";
import { compareText, splitLines, type DiffCell, type DiffKind } from "../../lib/textDiff";
import { Pane } from "./common";

const cellTone: Record<DiffKind, [string, string]> = {
  same: ["", ""],
  change: ["bg-red-500/8", "bg-green-500/8"],
  remove: ["bg-red-500/8", "bg-subtle/60"],
  add: ["bg-subtle/60", "bg-green-500/8"],
};

function Cell({ cell, tone, mark }: { cell?: DiffCell; tone: string; mark: string }) {
  return (
    <div className={cx("flex min-w-0", tone)}>
      <span className="w-10 shrink-0 pr-2 text-right text-muted/70 select-none">{cell?.no ?? ""}</span>
      <span className="selectable min-w-0 flex-1 pr-2 break-all whitespace-pre-wrap">
        {cell?.parts.map((p, i) => (
          <span key={i} className={p.changed ? cx("rounded-sm", mark) : undefined}>
            {p.text}
          </span>
        ))}
        {cell && cell.parts.every((p) => !p.text) && "\u00a0"}
      </span>
    </div>
  );
}

export default function DiffTool() {
  const { t } = useI18n();
  const [left, setLeft] = useState("");
  const [right, setRight] = useState("");
  const [ignoreWhitespace, setIgnoreWhitespace] = useState(false);
  const [ignoreCase, setIgnoreCase] = useState(false);
  const a = useDeferredValue(left);
  const b = useDeferredValue(right);
  const result = useMemo(() => compareText(a, b, { ignoreWhitespace, ignoreCase }), [a, b, ignoreWhitespace, ignoreCase]);
  const empty = !left && !right;

  const option = (label: string, checked: boolean, onChange: (v: boolean) => void) => (
    <label className="flex items-center gap-2 text-[13px]">
      <Switch checked={checked} onChange={onChange} label={label} />
      {label}
    </label>
  );

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
        {option(t("tools.diff.ignoreWhitespace"), ignoreWhitespace, setIgnoreWhitespace)}
        {option(t("tools.diff.ignoreCase"), ignoreCase, setIgnoreCase)}
        <div className="flex-1" />
        <div className="flex gap-2">
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
      </div>
      <div className="grid grid-cols-2 gap-3">
        <Pane title={t("tools.diff.left", { lines: splitLines(left).length })}>
          <TextArea className="h-72" value={left} autoFocus onChange={(e) => setLeft(e.target.value)} />
        </Pane>
        <Pane title={t("tools.diff.right", { lines: splitLines(right).length })}>
          <TextArea className="h-72" value={right} onChange={(e) => setRight(e.target.value)} />
        </Pane>
      </div>

      {empty ? (
        <div className="py-6 text-center text-xs text-muted">{t("tools.diff.hint")}</div>
      ) : (
        <>
          <div
            className={cx(
              "flex items-center gap-2 rounded-lg px-3 py-2 text-[13px]",
              result.same ? "bg-green-500/10 text-green-700 dark:text-green-400" : "bg-amber-500/10 text-amber-700 dark:text-amber-400",
            )}
          >
            {result.same ? <CircleCheck size={15} /> : <CircleAlert size={15} />}
            {result.same
              ? t("tools.diff.same")
              : t("tools.diff.summary", { changed: result.changed, removed: result.removed, added: result.added })}
          </div>
          {result.rows.length > 0 && (
            <Card className="overflow-hidden font-mono text-[12px] leading-relaxed">
              {result.rows.map((r, i) => {
                const [lt, rt] = cellTone[r.kind];
                return (
                  <div key={i} className="grid grid-cols-2 divide-x divide-line border-b border-line/60 last:border-0">
                    <Cell cell={r.left} tone={lt} mark="bg-red-500/30" />
                    <Cell cell={r.right} tone={rt} mark="bg-green-500/30" />
                  </div>
                );
              })}
            </Card>
          )}
        </>
      )}
    </div>
  );
}
