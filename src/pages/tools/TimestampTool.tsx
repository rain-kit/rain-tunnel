import { ArrowDownToLine, CalendarClock, Clock, Copy, Pause, Play } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { Button, cx, Input } from "../../components/ui";
import { useI18n } from "../../lib/i18n";
import { formatDateTime, parseDateTime, parseTimestamp } from "../../lib/tools";
import { CopyButton, ErrorText, useCopy } from "./common";

function getTzLabel(d = new Date()): string {
  const min = -d.getTimezoneOffset();
  const sign = min >= 0 ? "+" : "-";
  const abs = Math.abs(min);
  const hh = Math.floor(abs / 60);
  const mm = abs % 60;
  return mm ? `UTC${sign}${hh}:${String(mm).padStart(2, "0")}` : `UTC${sign}${hh}`;
}

function formatRelative(targetMs: number, baseMs: number, locale: string): string {
  const diffSec = Math.round((targetMs - baseMs) / 1000);
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  if (Math.abs(diffSec) < 3) return rtf.format(0, "second");
  const units: [Intl.RelativeTimeFormatUnit, number][] = [
    ["year", 31536000],
    ["month", 2592000],
    ["day", 86400],
    ["hour", 3600],
    ["minute", 60],
    ["second", 1],
  ];
  for (const [unit, sec] of units) {
    if (Math.abs(diffSec) >= sec) return rtf.format(Math.round(diffSec / sec), unit);
  }
  return rtf.format(diffSec, "second");
}

function NowTile({ label, value }: { label: string; value: string }) {
  const { t } = useI18n();
  const copy = useCopy();
  return (
    <button
      type="button"
      onClick={() => copy(value)}
      title={t("tools.copy")}
      className="group flex items-center justify-between gap-2 rounded-lg border border-line bg-surface px-3 py-2 text-left shadow-2xs transition-colors hover:border-accent/45"
    >
      <div className="min-w-0">
        <div className="truncate text-[11px] whitespace-nowrap text-muted">{label}</div>
        <div className="mt-0.5 truncate font-mono text-sm font-semibold text-ink tabular-nums">{value}</div>
      </div>
      <span className="flex size-7 shrink-0 items-center justify-center rounded-md text-muted transition-colors group-hover:bg-subtle group-hover:text-accent">
        <Copy size={13} />
      </span>
    </button>
  );
}

function OutputCell({ label, value }: { label: ReactNode; value: string }) {
  return (
    <div className="flex items-center justify-between gap-2 px-3.5 py-2.5 transition-colors hover:bg-subtle/30">
      <div className="min-w-0">
        <div className="truncate text-[11px] whitespace-nowrap text-muted">{label}</div>
        <div className="selectable mt-0.5 truncate font-mono text-[13px] font-medium text-ink tabular-nums">{value || "-"}</div>
      </div>
      <CopyButton text={value} />
    </div>
  );
}

export default function TimestampTool() {
  const { t, locale } = useI18n();
  const copy = useCopy();
  const [now, setNow] = useState(Date.now);
  const [paused, setPaused] = useState(false);
  const [ts, setTs] = useState(() => String(Math.floor(Date.now() / 1000)));
  const [date, setDate] = useState(() => formatDateTime(new Date()));

  useEffect(() => {
    if (paused) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [paused]);

  const parsedTs = parseTimestamp(ts);
  const parsedDate = parseDateTime(date);
  const nowSec = String(Math.floor(now / 1000));
  const nowMs = String(now);
  const nowLocal = formatDateTime(new Date(now));
  const tz = getTzLabel(new Date(now));

  const fillBothNow = () => {
    const cur = Date.now();
    setNow(cur);
    const asMs = parsedTs?.unit === "ms";
    setTs(asMs ? String(cur) : String(Math.floor(cur / 1000)));
    setDate(formatDateTime(new Date(cur), asMs));
  };

  return (
    <div className="@container flex flex-col gap-3.5">
      <div className="rounded-xl border border-accent/20 bg-gradient-to-r from-accent/[0.06] via-subtle/50 to-subtle/30 p-3.5 dark:border-accent/25 dark:from-accent/[0.12] dark:via-subtle/60 dark:to-subtle/30">
        <div className="flex flex-wrap items-center gap-3">
          <div className="flex size-10 shrink-0 items-center justify-center rounded-xl bg-accent/12 text-accent">
            <Clock size={19} />
          </div>
          <div className="min-w-0">
            <div className="flex items-center gap-2 whitespace-nowrap">
              <span className={cx("size-2 shrink-0 rounded-full", paused ? "bg-amber-500" : "bg-emerald-500 animate-pulse")} />
              <span className="text-xs font-medium text-muted">{t("tools.time.now")}</span>
              <span className="rounded bg-surface px-1.5 py-0.5 font-mono text-[10px] font-medium text-muted ring-1 ring-line">{tz}</span>
            </div>
            <button
              type="button"
              onClick={() => copy(nowLocal)}
              title={t("tools.copy")}
              className="group mt-0.5 inline-flex items-center gap-1.5 font-mono text-lg font-semibold tracking-tight whitespace-nowrap text-ink tabular-nums transition-colors hover:text-accent"
            >
              {nowLocal}
              <Copy size={13} className="text-muted opacity-0 transition-opacity group-hover:opacity-100" />
            </button>
          </div>
          <div className="ml-auto flex items-center gap-1.5">
            <Button
              size="sm"
              icon={paused ? <Play size={13} /> : <Pause size={13} />}
              onClick={() => {
                if (paused) setNow(Date.now());
                setPaused((v) => !v);
              }}
            >
              {t(paused ? "common.resume" : "common.pause")}
            </Button>
            <Button size="sm" variant="primary" icon={<ArrowDownToLine size={13} />} onClick={fillBothNow}>
              {t("tools.time.useNow")}
            </Button>
          </div>
        </div>

        <div className="mt-3 grid grid-cols-1 gap-2.5 @min-[420px]:grid-cols-2">
          <NowTile label={t("tools.time.seconds")} value={nowSec} />
          <NowTile label={t("tools.time.millis")} value={nowMs} />
        </div>
      </div>

      <div className="overflow-hidden rounded-xl border border-line bg-surface">
        <div className="border-b border-line bg-subtle/35 px-3.5 py-2.5">
          <div className="mb-2 flex flex-wrap items-center gap-2">
            <span className="flex size-6 shrink-0 items-center justify-center rounded-md bg-accent/10 text-accent">
              <Clock size={13} />
            </span>
            <span className="text-xs font-semibold whitespace-nowrap text-ink">{t("tools.time.tsToDate")}</span>
            {parsedTs && (
              <>
                <span className="rounded-md bg-accent/10 px-1.5 py-0.5 text-[11px] font-medium whitespace-nowrap text-accent">
                  {t(parsedTs.unit === "ms" ? "tools.time.asMillis" : "tools.time.asSeconds")}
                </span>
                <span className="rounded-md bg-surface px-1.5 py-0.5 text-[11px] whitespace-nowrap text-muted ring-1 ring-line">
                  {formatRelative(parsedTs.date.getTime(), now, locale)}
                </span>
              </>
            )}
            <button
              type="button"
              onClick={() => setTs(parsedTs?.unit === "ms" ? String(Date.now()) : String(Math.floor(Date.now() / 1000)))}
              className="ml-auto rounded-md border border-line bg-surface px-2 py-0.5 text-[11px] font-medium whitespace-nowrap text-ink shadow-2xs transition-colors hover:bg-subtle"
            >
              {t("tools.time.useNow")}
            </button>
          </div>
          <Input
            mono
            value={ts}
            invalid={!!ts.trim() && !parsedTs}
            placeholder="1700000000"
            onChange={(e) => setTs(e.target.value)}
          />
          {ts.trim() && !parsedTs && (
            <div className="mt-1.5">
              <ErrorText>{t("tools.time.invalidTs")}</ErrorText>
            </div>
          )}
        </div>
        <div className="grid grid-cols-1 divide-y divide-line @min-[480px]:grid-cols-2 @min-[480px]:divide-x @min-[480px]:divide-y-0">
          <OutputCell
            label={`${t("tools.time.local")} (${tz})`}
            value={parsedTs ? formatDateTime(parsedTs.date, parsedTs.unit === "ms") : ""}
          />
          <OutputCell label="UTC (ISO 8601)" value={parsedTs?.date.toISOString() ?? ""} />
        </div>
      </div>

      <div className="overflow-hidden rounded-xl border border-line bg-surface">
        <div className="border-b border-line bg-subtle/35 px-3.5 py-2.5">
          <div className="mb-2 flex flex-wrap items-center gap-2">
            <span className="flex size-6 shrink-0 items-center justify-center rounded-md bg-violet-500/10 text-violet-600 dark:text-violet-400">
              <CalendarClock size={13} />
            </span>
            <span className="text-xs font-semibold whitespace-nowrap text-ink">{t("tools.time.dateToTs")}</span>
            {parsedDate && (
              <span className="rounded-md bg-surface px-1.5 py-0.5 text-[11px] whitespace-nowrap text-muted ring-1 ring-line">
                {formatRelative(parsedDate.getTime(), now, locale)}
              </span>
            )}
            <button
              type="button"
              onClick={() => setDate(formatDateTime(new Date()))}
              className="ml-auto rounded-md border border-line bg-surface px-2 py-0.5 text-[11px] font-medium whitespace-nowrap text-ink shadow-2xs transition-colors hover:bg-subtle"
            >
              {t("tools.time.useNow")}
            </button>
          </div>
          <Input
            mono
            value={date}
            invalid={!!date.trim() && !parsedDate}
            placeholder="2024-01-01 08:00:00"
            onChange={(e) => setDate(e.target.value)}
          />
          {date.trim() && !parsedDate && (
            <div className="mt-1.5">
              <ErrorText>{t("tools.time.invalidDate")}</ErrorText>
            </div>
          )}
        </div>
        <div className="grid grid-cols-1 divide-y divide-line @min-[480px]:grid-cols-2 @min-[480px]:divide-x @min-[480px]:divide-y-0">
          <OutputCell label={t("tools.time.seconds")} value={parsedDate ? String(Math.floor(parsedDate.getTime() / 1000)) : ""} />
          <OutputCell label={t("tools.time.millis")} value={parsedDate ? String(parsedDate.getTime()) : ""} />
        </div>
      </div>
    </div>
  );
}
