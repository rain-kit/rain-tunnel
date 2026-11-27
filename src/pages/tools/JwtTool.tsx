import { CircleAlert, CircleCheck, Clock, Eraser, KeyRound } from "lucide-react";
import { useMemo, useState } from "react";
import { Button, cx, TextArea } from "../../components/ui";
import { useI18n } from "../../lib/i18n";
import { decodeJwt, formatDateTime } from "../../lib/tools";
import { CopyButton, ErrorText } from "./common";

const timeClaims = ["exp", "iat", "nbf"] as const;

export default function JwtTool() {
  const { t } = useI18n();
  const [token, setToken] = useState("");
  const jwt = useMemo(() => {
    try {
      return token.trim() ? decodeJwt(token) : null;
    } catch {
      return null;
    }
  }, [token]);
  const invalid = !!token.trim() && !jwt;
  const headerObj = (jwt && typeof jwt.header === "object" && jwt.header !== null ? jwt.header : null) as Record<string, unknown> | null;
  const alg = typeof headerObj?.alg === "string" ? headerObj.alg : undefined;
  const header = jwt ? JSON.stringify(jwt.header, null, 2) : "";
  const payload = jwt ? JSON.stringify(jwt.payload, null, 2) : "";
  const claimCount = jwt ? Object.keys(jwt.payload).length : 0;
  const claims = timeClaims.filter((c) => typeof jwt?.payload[c] === "number");
  const now = Date.now() / 1000;

  return (
    <div className="@container flex h-full flex-col gap-3">
      <div className="shrink-0 rounded-xl border border-line bg-subtle/35 p-3">
        <div className="mb-2 flex flex-wrap items-center gap-2">
          <span className="flex size-6 shrink-0 items-center justify-center rounded-md bg-accent/10 text-accent">
            <KeyRound size={13} />
          </span>
          <span className="text-xs font-semibold text-ink">Token</span>
          <span className="hidden text-xs text-muted @min-[420px]:inline">· {t("tools.jwt.hint")}</span>
          <div className="ml-auto flex items-center gap-1">
            <CopyButton text={token.trim()} />
            <Button size="sm" icon={<Eraser size={13} />} disabled={!token} onClick={() => setToken("")}>
              {t("tools.clear")}
            </Button>
          </div>
        </div>
        <TextArea
          className="h-20"
          autoFocus
          value={token}
          invalid={invalid}
          placeholder="eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9..."
          onChange={(e) => setToken(e.target.value)}
        />
        {invalid && (
          <div className="mt-1.5">
            <ErrorText>{t("tools.jwt.invalid")}</ErrorText>
          </div>
        )}
      </div>

      {claims.length > 0 && (
        <div className="flex shrink-0 flex-wrap gap-2.5">
          {claims.map((c) => {
            const sec = jwt!.payload[c] as number;
            const state = c === "exp" ? (sec < now ? "expired" : "valid") : c === "nbf" && sec > now ? "notYet" : null;
            return (
              <div key={c} className="flex min-w-44 flex-1 items-center justify-between gap-2 rounded-xl border border-line bg-surface px-3 py-2">
                <div className="min-w-0">
                  <div className="flex items-center gap-1.5 text-[11px] whitespace-nowrap text-muted">
                    <Clock size={11} />
                    <span className="font-mono font-semibold text-ink">{c}</span>
                    <span>· {t(`tools.jwt.${c}`)}</span>
                  </div>
                  <div className="selectable mt-0.5 truncate font-mono text-xs font-medium text-ink tabular-nums">
                    {formatDateTime(new Date(sec * 1000))}
                  </div>
                </div>
                {state && (
                  <span
                    className={cx(
                      "inline-flex shrink-0 items-center gap-1 rounded-md px-1.5 py-0.5 text-[11px] font-medium whitespace-nowrap",
                      state === "valid"
                        ? "bg-emerald-500/12 text-emerald-600 dark:text-emerald-400"
                        : "bg-red-500/12 text-red-600 dark:text-red-400",
                    )}
                  >
                    {state === "valid" ? <CircleCheck size={11} /> : <CircleAlert size={11} />}
                    {t(`tools.jwt.${state}`)}
                  </span>
                )}
              </div>
            );
          })}
        </div>
      )}

      {!jwt ? (
        <div
          className={cx(
            "flex min-h-48 flex-1 flex-col items-center justify-center gap-2.5 rounded-xl border px-6 text-center",
            invalid ? "border-red-500/25 bg-red-500/[0.04]" : "border-dashed border-line bg-subtle/25 text-muted",
          )}
        >
          <div
            className={cx(
              "flex size-10 items-center justify-center rounded-2xl",
              invalid ? "bg-red-500/10 text-red-600 dark:text-red-400" : "bg-accent/10 text-accent",
            )}
          >
            {invalid ? <CircleAlert size={20} /> : <KeyRound size={19} />}
          </div>
          <span className={cx("max-w-sm text-xs leading-relaxed", invalid && "text-red-600 dark:text-red-400")}>
            {t(invalid ? "tools.jwt.invalid" : "tools.jwt.emptyHint")}
          </span>
        </div>
      ) : (
        <div className="grid min-h-48 flex-1 grid-cols-1 gap-3 @min-[480px]:grid-cols-[2fr_3fr]">
          <div className="flex min-h-0 flex-col overflow-hidden rounded-xl border border-line bg-surface">
            <div className="flex shrink-0 items-center gap-2 border-b border-line bg-sky-500/[0.06] px-3 py-1.5 dark:bg-sky-500/[0.10]">
              <span className="size-2 rounded-full bg-sky-500" />
              <span className="text-xs font-semibold text-ink">Header</span>
              {alg && (
                <span className="rounded bg-sky-500/12 px-1.5 py-0.5 font-mono text-[11px] font-medium text-sky-600 dark:text-sky-400">
                  {alg}
                </span>
              )}
              <div className="ml-auto">
                <CopyButton text={header} />
              </div>
            </div>
            <pre className="selectable min-h-0 flex-1 overflow-auto p-3 font-mono text-[12.5px] leading-relaxed text-ink">{header}</pre>
          </div>

          <div className="flex min-h-0 flex-col overflow-hidden rounded-xl border border-line bg-surface">
            <div className="flex shrink-0 items-center gap-2 border-b border-line bg-violet-500/[0.06] px-3 py-1.5 dark:bg-violet-500/[0.10]">
              <span className="size-2 rounded-full bg-violet-500" />
              <span className="text-xs font-semibold text-ink">Payload</span>
              <span className="rounded bg-violet-500/12 px-1.5 py-0.5 text-[11px] font-medium whitespace-nowrap text-violet-600 dark:text-violet-400">
                {t("tools.jwt.claims", { count: claimCount })}
              </span>
              <div className="ml-auto">
                <CopyButton text={payload} />
              </div>
            </div>
            <pre className="selectable min-h-0 flex-1 overflow-auto p-3 font-mono text-[12.5px] leading-relaxed text-ink">{payload}</pre>
          </div>
        </div>
      )}
    </div>
  );
}
