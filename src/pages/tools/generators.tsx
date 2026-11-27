import { RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { Button, Field, Input, Segmented, SwitchRow, TextArea } from "../../components/ui";
import { useI18n } from "../../lib/i18n";
import { charsets, randomPassword, uniqueChars, uuidV4, type Charset } from "../../lib/tools";
import { CopyButton, ErrorText, Pane } from "./common";

const clamp = (v: string, min: number, max: number, fallback: number) => {
  const n = Math.floor(Number(v));
  return Number.isFinite(n) && v.trim() ? Math.min(max, Math.max(min, n)) : fallback;
};

/** 生成结果的展示区，选项变化或点击重新生成时刷新。 */
function Output({ generate, deps }: { generate: () => string; deps: unknown[] }) {
  const { t } = useI18n();
  const [value, setValue] = useState("");
  const refresh = () => setValue(generate());
  useEffect(refresh, deps);
  return (
    <Pane
      title={t("tools.result")}
      className="min-h-48 flex-1"
      actions={
        <>
          <Button size="sm" variant="ghost" icon={<RefreshCw size={13} />} onClick={refresh}>
            {t("tools.regenerate")}
          </Button>
          <CopyButton text={value} />
        </>
      }
    >
      <TextArea className="flex-1" readOnly value={value} />
    </Pane>
  );
}

export function UuidTool() {
  const { t } = useI18n();
  const [count, setCount] = useState("5");
  const [upper, setUpper] = useState(false);
  const [hyphen, setHyphen] = useState(true);
  const n = clamp(count, 1, 1000, 1);

  const generate = () =>
    Array.from({ length: n }, () => {
      const id = hyphen ? uuidV4() : uuidV4().replace(/-/g, "");
      return upper ? id.toUpperCase() : id;
    }).join("\n");

  return (
    <div className="flex h-full flex-col gap-3">
      <Field label={t("tools.count")} hint={t("editor.range", { min: 1, max: 1000 })} className="w-40">
        <Input type="number" min={1} max={1000} value={count} onChange={(e) => setCount(e.target.value)} />
      </Field>
      <SwitchRow title={t("tools.uppercase")} checked={upper} onChange={setUpper} />
      <SwitchRow title={t("tools.uuid.hyphen")} checked={hyphen} onChange={setHyphen} />
      <Output generate={generate} deps={[n, upper, hyphen]} />
    </div>
  );
}

const sets: Charset[] = ["upper", "lower", "digits", "symbols"];
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
  const noSymbols = enabled.symbols && !uniqueChars(symbols);

  const generate = () => Array.from({ length: n }, () => randomPassword(len, pools, excludeSimilar)).join("\n");

  return (
    <div className="flex h-full flex-col gap-3">
      <div className="flex gap-3">
        <Field label={t("tools.password.length")} hint={t("editor.range", { min: 1, max: 256 })} className="w-40">
          <Input type="number" min={1} max={256} value={opts.length} onChange={(e) => set({ length: e.target.value })} />
        </Field>
        <Field label={t("tools.count")} hint={t("editor.range", { min: 1, max: 100 })} className="w-40">
          <Input type="number" min={1} max={100} value={opts.count} onChange={(e) => set({ count: e.target.value })} />
        </Field>
      </div>
      {sets.map((s) => (
        <SwitchRow
          key={s}
          title={t(`tools.password.${s}`)}
          desc={s === "symbols" ? t("tools.password.symbolsDesc") : <span className="font-mono">{charsets[s]}</span>}
          checked={enabled[s]}
          onChange={(v) => set({ enabled: { ...enabled, [s]: v } })}
        />
      ))}
      {enabled.symbols && (
        <div className="flex flex-col gap-1.5 rounded-lg bg-subtle/60 p-2.5">
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
            <div className="text-xs text-muted">{t(`tools.password.symbolsDesc.${symbolMode}`)}</div>
          )}
        </div>
      )}
      <SwitchRow
        title={t("tools.password.excludeSimilar")}
        desc="0 O 1 l I |"
        checked={excludeSimilar}
        onChange={(excludeSimilar) => set({ excludeSimilar })}
      />
      <Output generate={generate} deps={[len, n, pools.join("\u0000"), excludeSimilar]} />
    </div>
  );
}
