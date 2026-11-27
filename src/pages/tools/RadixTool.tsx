import { Calculator, Eraser } from "lucide-react";
import { useState } from "react";
import { Button, cx, Input, Segmented } from "../../components/ui";
import { useI18n } from "../../lib/i18n";
import { parseBigInt } from "../../lib/tools";
import { CopyButton, ErrorText, FilterChip } from "./common";

const bases = [
  { radix: 2, tag: "BIN" },
  { radix: 8, tag: "OCT" },
  { radix: 10, tag: "DEC" },
  { radix: 16, tag: "HEX" },
  { radix: 32, tag: "B32" },
  { radix: 36, tag: "B36" },
] as const;

function groupDigits(s: string, radix: number): string {
  if (!s) return "";
  const neg = s.startsWith("-");
  const body = neg ? s.slice(1) : s;
  const step = radix === 10 || radix === 8 ? 3 : 4;
  const sep = radix === 10 ? "," : " ";
  const parts: string[] = [];
  for (let i = body.length; i > 0; i -= step) {
    parts.unshift(body.slice(Math.max(0, i - step), i));
  }
  return (neg ? "-" : "") + parts.join(sep);
}

export default function RadixTool() {
  const { t } = useI18n();
  const [value, setValue] = useState("255");
  const [base, setBase] = useState("10");
  const [upper, setUpper] = useState(false);
  const [group, setGroup] = useState(true);

  let parsed: bigint | null = null;
  try {
    if (value.trim()) parsed = parseBigInt(value, Number(base));
  } catch {
    parsed = null;
  }
  const invalid = !!value.trim() && parsed === null;

  return (
    <div className="flex flex-col gap-3">
      <div className="rounded-xl border border-line bg-subtle/35 p-3.5">
        <div className="mb-2.5 flex flex-wrap items-center gap-2">
          <span className="flex size-6 shrink-0 items-center justify-center rounded-md bg-accent/10 text-accent">
            <Calculator size={13} />
          </span>
          <span className="text-xs font-semibold whitespace-nowrap text-ink">{t("tools.radix.from")}</span>
          <Segmented
            value={base}
            onChange={setBase}
            options={bases.map((b) => ({
              value: String(b.radix),
              label: (
                <span title={t("tools.radix.base", { base: b.radix })} className="font-mono text-xs tabular-nums">
                  {b.radix}
                </span>
              ),
            }))}
          />
          <div className="flex-1" />
          <FilterChip active={group} onClick={() => setGroup((v) => !v)}>
            {t("tools.radix.group")}
          </FilterChip>
          <FilterChip active={upper} onClick={() => setUpper((v) => !v)}>
            {t("tools.uppercase")}
          </FilterChip>
          <Button size="sm" icon={<Eraser size={13} />} disabled={!value} onClick={() => setValue("")}>
            {t("tools.clear")}
          </Button>
        </div>

        <Input mono autoFocus value={value} invalid={invalid} placeholder="255 / 0xff / 0b11111111" onChange={(e) => setValue(e.target.value)} />
        {invalid && (
          <div className="mt-1.5">
            <ErrorText>{t("tools.radix.invalid", { base })}</ErrorText>
          </div>
        )}
      </div>

      <div className="divide-y divide-line overflow-hidden rounded-xl border border-line bg-surface">
        {bases.map((b) => {
          const raw = parsed === null ? "" : upper ? parsed.toString(b.radix).toUpperCase() : parsed.toString(b.radix);
          const shown = raw && group ? groupDigits(raw, b.radix) : raw;
          const active = String(b.radix) === base;
          return (
            <div
              key={b.radix}
              className={cx(
                "flex min-h-12 items-center gap-3 px-3.5 py-2 transition-colors",
                active ? "bg-accent/[0.05] dark:bg-accent/[0.10]" : "hover:bg-subtle/30",
              )}
            >
              <button
                type="button"
                onClick={() => {
                  setBase(String(b.radix));
                  if (raw) setValue(raw);
                }}
                className="flex w-28 shrink-0 items-center gap-2 text-left"
              >
                <span
                  className={cx(
                    "w-9 rounded px-1.5 py-0.5 text-center font-mono text-[10px] font-semibold",
                    active ? "bg-accent text-white" : "bg-subtle text-muted",
                  )}
                >
                  {b.tag}
                </span>
                <span className="text-xs whitespace-nowrap text-muted">{t("tools.radix.base", { base: b.radix })}</span>
              </button>
              <span className="selectable min-w-0 flex-1 font-mono text-[13px] break-all text-ink tabular-nums">{shown || "-"}</span>
              <CopyButton text={raw} />
            </div>
          );
        })}
      </div>
    </div>
  );
}
