import { Check, CircleAlert, Copy, Fingerprint, RefreshCw, ShieldCheck } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { Button, cx, Input, Segmented, TextArea } from "../../components/ui";
import { useI18n } from "../../lib/i18n";
import { charsets, randomPassword, uniqueChars, uuidV4, type Charset } from "../../lib/tools";
import { CopyButton, ErrorText, FilterChip, useCopy } from "./common";

const clamp = (v: string, min: number, max: number, fallback: number) => {
  const n = Math.floor(Number(v));
  return Number.isFinite(n) && v.trim() ? Math.min(max, Math.max(min, n)) : fallback;
};

function QuickChip({ active, onClick, children }: { active: boolean; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cx(
        "h-6.5 rounded-md border px-2 font-mono text-[11px] font-medium tabular-nums transition-colors",
        active
          ? "border-accent/40 bg-accent/12 text-accent dark:bg-accent/25 dark:text-sky-300"
          : "border-line bg-surface text-muted hover:bg-subtle hover:text-ink",
      )}
    >
      {children}
    </button>
  );
}

function GeneratedPanel({ items, emptyMessage }: { items: string[]; emptyMessage?: string }) {
  const { t } = useI18n();
  const copy = useCopy();
  const all = items.join("\n");

  return (
    <div className="flex min-h-44 flex-1 flex-col overflow-hidden rounded-xl border border-line bg-surface">
      <div className="flex shrink-0 items-center gap-2 border-b border-line bg-subtle/40 px-3.5 py-1.5">
        <span className="text-xs font-semibold text-ink">{t("tools.result")}</span>
        {items.length > 0 && (
          <span className="rounded bg-surface px-1.5 py-0.5 font-mono text-[11px] text-muted ring-1 ring-line tabular-nums">
            {items.length}
          </span>
        )}
        <div className="ml-auto">
          <Button size="sm" variant="ghost" icon={<Copy size={13} />} disabled={!items.length} onClick={() => copy(all)}>
            {t("tools.copyAll")}
          </Button>
        </div>
      </div>

      {emptyMessage ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 bg-red-500/[0.03] px-6 text-center">
          <CircleAlert size={20} className="text-red-500" />
          <span className="text-xs text-red-600 dark:text-red-400">{emptyMessage}</span>
        </div>
      ) : items.length > 100 ? (
        <TextArea className="flex-1 rounded-none border-0" readOnly value={all} />
      ) : (
        <div className="min-h-0 flex-1 divide-y divide-line/60 overflow-y-auto">
          {items.map((item, idx) => (
            <div key={idx} className="flex items-center gap-3 px-3.5 py-1.5 transition-colors hover:bg-subtle/35">
              <span className="w-6 shrink-0 text-right font-mono text-[11px] text-muted/65 tabular-nums select-none">{idx + 1}</span>
              <span className="selectable min-w-0 flex-1 font-mono text-[13px] break-all text-ink">{item}</span>
              <CopyButton text={item} />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

const uuidPresets = [1, 5, 10, 50];

export function UuidTool() {
  const { t } = useI18n();
  const [count, setCount] = useState("5");
  const [upper, setUpper] = useState(false);
  const [hyphen, setHyphen] = useState(true);
  const [items, setItems] = useState<string[]>([]);
  const n = clamp(count, 1, 1000, 1);

  const refresh = () =>
    setItems(
      Array.from({ length: n }, () => {
        const id = hyphen ? uuidV4() : uuidV4().replace(/-/g, "");
        return upper ? id.toUpperCase() : id;
      }),
    );

  useEffect(refresh, [n, upper, hyphen]);

  return (
    <div className="flex h-full flex-col gap-3">
      <div className="flex shrink-0 flex-wrap items-center gap-2.5 rounded-xl border border-line bg-subtle/35 p-3">
        <span className="flex size-7 shrink-0 items-center justify-center rounded-lg bg-accent/10 text-accent">
          <Fingerprint size={15} />
        </span>
        <span className="text-xs font-medium whitespace-nowrap text-muted">{t("tools.count")}</span>
        <Input
          type="number"
          min={1}
          max={1000}
          mono
          value={count}
          className="h-7 w-20 px-2 text-xs"
          onChange={(e) => setCount(e.target.value)}
        />
        <div className="flex items-center gap-1">
          {uuidPresets.map((p) => (
            <QuickChip key={p} active={n === p} onClick={() => setCount(String(p))}>
              {p}
            </QuickChip>
          ))}
        </div>

        <span className="mx-0.5 h-5 w-px bg-line" />

        <FilterChip active={hyphen} onClick={() => setHyphen((v) => !v)}>
          {t("tools.uuid.hyphen")}
        </FilterChip>
        <FilterChip active={upper} onClick={() => setUpper((v) => !v)}>
          {t("tools.uppercase")}
        </FilterChip>

        <div className="ml-auto">
          <Button size="sm" variant="primary" icon={<RefreshCw size={13} />} onClick={refresh}>
            {t("tools.regenerate")}
          </Button>
        </div>
      </div>

      <GeneratedPanel items={items} />
    </div>
  );
}

const sets: Charset[] = ["upper", "lower", "digits", "symbols"];
const setPreview: Record<Charset, string> = {
  upper: "A–Z",
  lower: "a–z",
  digits: "0–9",
  symbols: "!@#$%*",
};
const lenPresets = [12, 16, 24, 32];
type SymbolMode = "all" | "custom";

interface PasswordOptions {
  length: string;
  count: string;
  enabled: Record<Charset, boolean>;
  excludeSimilar: boolean;
  symbolMode: SymbolMode;
  customSymbols: string;
}

const OPTIONS_KEY = "tools.password";
const defaultOptions: PasswordOptions = {
  length: "16",
  count: "5",
  enabled: { upper: true, lower: true, digits: true, symbols: false },
  excludeSimilar: false,
  symbolMode: "all",
  customSymbols: charsets.symbols,
};

function loadOptions(): PasswordOptions {
  try {
    const saved = JSON.parse(localStorage.getItem(OPTIONS_KEY) ?? "{}");
    return { ...defaultOptions, ...saved, enabled: { ...defaultOptions.enabled, ...saved.enabled } };
  } catch {
    return defaultOptions;
  }
}

export function PasswordTool() {
  const { t } = useI18n();
  const [opts, setOpts] = useState(loadOptions);
  const [items, setItems] = useState<string[]>([]);
  const set = (patch: Partial<PasswordOptions>) =>
    setOpts((cur) => {
      const next = { ...cur, ...patch };
      localStorage.setItem(OPTIONS_KEY, JSON.stringify(next));
      return next;
    });

  const { enabled, excludeSimilar, symbolMode } = opts;
  const symbols = symbolMode === "custom" ? opts.customSymbols : charsets.symbols;
  const len = clamp(opts.length, 1, 256, 16);
  const n = clamp(opts.count, 1, 100, 1);
  const chosen = sets.filter((s) => enabled[s]);
  const pools = chosen.map((s) => (s === "symbols" ? symbols : charsets[s]));
  const similarRe = /[0O1lI|]/g;
  const activePool = uniqueChars(
    pools.map((p) => (excludeSimilar ? p.replace(similarRe, "") : p)).join(""),
  );
  const noSymbols = enabled.symbols && !uniqueChars(symbols);
  const noCharset = chosen.length === 0 || !activePool;
  const entropy = activePool.length > 1 ? Math.round(len * Math.log2(activePool.length)) : 0;
  const strength =
    entropy < 40
      ? { key: "weak" as const, cls: "bg-red-500/12 text-red-600 dark:text-red-400" }
      : entropy < 60
        ? { key: "fair" as const, cls: "bg-amber-500/12 text-amber-600 dark:text-amber-400" }
        : entropy < 90
          ? { key: "good" as const, cls: "bg-emerald-500/12 text-emerald-600 dark:text-emerald-400" }
          : { key: "strong" as const, cls: "bg-sky-500/12 text-sky-600 dark:text-sky-400" };

  const refresh = () => {
    if (noCharset || noSymbols) return setItems([]);
    setItems(Array.from({ length: n }, () => randomPassword(len, pools, excludeSimilar)));
  };

  useEffect(refresh, [len, n, pools.join("\u0000"), excludeSimilar]);

  return (
    <div className="@container flex h-full flex-col gap-3">
      <div className="flex shrink-0 flex-col gap-2.5 rounded-xl border border-line bg-subtle/35 p-3">
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-xs font-medium whitespace-nowrap text-muted">{t("tools.password.length")}</span>
          <input
            type="range"
            min={4}
            max={64}
            value={Math.min(64, len)}
            onChange={(e) => set({ length: e.target.value })}
            className="h-1.5 w-24 cursor-pointer accent-accent"
          />
          <Input
            type="number"
            min={1}
            max={256}
            mono
            value={opts.length}
            className="h-7 w-16 px-2 text-xs"
            onChange={(e) => set({ length: e.target.value })}
          />
          <div className="flex items-center gap-1">
            {lenPresets.map((p) => (
              <QuickChip key={p} active={len === p} onClick={() => set({ length: String(p) })}>
                {p}
              </QuickChip>
            ))}
          </div>

          <span className="mx-0.5 h-5 w-px bg-line" />

          <span className="text-xs font-medium whitespace-nowrap text-muted">{t("tools.count")}</span>
          <Input
            type="number"
            min={1}
            max={100}
            mono
            value={opts.count}
            className="h-7 w-16 px-2 text-xs"
            onChange={(e) => set({ count: e.target.value })}
          />

          {!noCharset && !noSymbols && (
            <span className={cx("ml-auto inline-flex items-center gap-1 rounded-md px-2 py-0.5 text-[11px] font-medium whitespace-nowrap tabular-nums", strength.cls)}>
              <ShieldCheck size={12} />
              {t(`tools.password.strength.${strength.key}`)} · {entropy} bit
            </span>
          )}
        </div>

        <div className="grid grid-cols-2 gap-2 @min-[520px]:grid-cols-4">
          {sets.map((s) => {
            const on = enabled[s];
            return (
              <button
                key={s}
                type="button"
                aria-pressed={on}
                onClick={() => set({ enabled: { ...enabled, [s]: !on } })}
                className={cx(
                  "flex items-center gap-2 rounded-lg border px-2.5 py-2 text-left transition-colors",
                  on ? "border-accent/45 bg-surface shadow-2xs" : "border-line bg-surface/50 text-muted hover:bg-surface",
                )}
              >
                <span
                  className={cx(
                    "flex size-4 shrink-0 items-center justify-center rounded text-white transition-colors",
                    on ? "bg-accent" : "border border-line bg-subtle",
                  )}
                >
                  {on && <Check size={11} strokeWidth={3} />}
                </span>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-xs font-medium text-ink">{t(`tools.password.${s}`)}</div>
                  <div className="truncate font-mono text-[10px] text-muted">{setPreview[s]}</div>
                </div>
              </button>
            );
          })}
        </div>

        {enabled.symbols && (
          <div className="flex flex-col gap-1.5 rounded-lg border border-line/80 bg-surface/80 p-2.5">
            <div className="flex items-center gap-2">
              <Segmented<SymbolMode>
                value={symbolMode}
                onChange={(symbolMode) => set({ symbolMode })}
                options={[
                  { value: "all", label: t("tools.password.symbols.all") },
                  { value: "custom", label: t("tools.password.symbols.custom") },
                ]}
              />
              <div className="min-w-0 flex-1">
                <Input
                  mono
                  value={symbols}
                  readOnly={symbolMode === "all"}
                  invalid={noSymbols}
                  placeholder={t("tools.password.symbolsPlaceholder")}
                  className={symbolMode === "all" ? "text-muted" : undefined}
                  onChange={(e) => set({ customSymbols: e.target.value })}
                />
              </div>
            </div>
            {noSymbols ? (
              <ErrorText>{t("tools.password.noSymbols")}</ErrorText>
            ) : (
              <div className="text-[11px] text-muted">{t(`tools.password.symbolsDesc.${symbolMode}`)}</div>
            )}
          </div>
        )}

        <div className="flex flex-wrap items-center gap-2">
          <FilterChip active={excludeSimilar} onClick={() => set({ excludeSimilar: !excludeSimilar })}>
            <span>{t("tools.password.excludeSimilar")}</span>
            <span className="font-mono text-[11px] opacity-75">0O1lI|</span>
          </FilterChip>
          <div className="ml-auto">
            <Button size="sm" variant="primary" icon={<RefreshCw size={13} />} disabled={noCharset || noSymbols} onClick={refresh}>
              {t("tools.regenerate")}
            </Button>
          </div>
        </div>
      </div>

      <GeneratedPanel
        items={items}
        emptyMessage={noSymbols ? t("tools.password.noSymbols") : noCharset ? t("tools.password.noCharset") : undefined}
      />
    </div>
  );
}
