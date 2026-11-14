import { ArrowDown, ArrowUp } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { trafficApi } from "../lib/api";
import { formatBytes } from "../lib/format";
import { useI18n } from "../lib/i18n";
import type { ByteCount, UsageView } from "../lib/types";
import { Card, cx } from "./ui";
import UsageChart, { type DayUsage } from "./UsageChart";

const RANGES = [7, 30] as const;
type Range = (typeof RANGES)[number];

/** 截止 today（YYYY-MM-DD）的最近 n 天，按日期升序。 */
function lastDays(today: string, n: number): string[] {
  const [y, m, d] = today.split("-").map(Number);
  const p = (v: number) => String(v).padStart(2, "0");
  return Array.from({ length: n }, (_, i) => {
    const x = new Date(y, m - 1, d - (n - 1 - i));
    return `${x.getFullYear()}-${p(x.getMonth() + 1)}-${p(x.getDate())}`;
  });
}

const sum = (list: ByteCount[]): ByteCount =>
  list.reduce((s, b) => ({ up: s.up + b.up, down: s.down + b.down }), { up: 0, down: 0 });

function Summary({ label, value, primary }: { label: string; value: ByteCount; primary?: boolean }) {
  return (
    <div className="min-w-0">
      <div className="text-xs text-muted">{label}</div>
      <div className={cx("truncate font-semibold tabular-nums", primary ? "text-xl" : "text-base")}>
        {formatBytes(value.up + value.down)}
      </div>
      <div className="flex gap-2.5 text-[11px] text-muted tabular-nums">
        <span className="inline-flex items-center gap-0.5 truncate">
          <ArrowUp size={11} className="shrink-0 text-amber-500" />
          {formatBytes(value.up)}
        </span>
        <span className="inline-flex items-center gap-0.5 truncate">
          <ArrowDown size={11} className="shrink-0 text-emerald-500" />
          {formatBytes(value.down)}
        </span>
      </div>
    </div>
  );
}

/** 累计流量和每日流量图；tunnelId 为空时汇总全部隧道。 */
export default function UsagePanel({ tunnelId, className }: { tunnelId: string; className?: string }) {
  const { t } = useI18n();
  const [range, setRange] = useState<Range>(30);
  const [usage, setUsage] = useState<UsageView | null>(null);

  useEffect(() => {
    let alive = true;
    const load = () =>
      trafficApi
        .usage(range)
        .then((u) => alive && setUsage(u))
        .catch(() => {});
    load();
    const timer = setInterval(load, 2000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [range]);

  const { days, total } = useMemo(() => {
    if (!usage) return { days: [] as DayUsage[], total: { up: 0, down: 0 } };
    const list = Object.entries(usage.tunnels)
      .filter(([id]) => !tunnelId || id === tunnelId)
      .map(([, u]) => u);
    const days = lastDays(usage.today, range).map((date) => ({
      date,
      ...sum(list.map((u) => u.days[date]).filter(Boolean)),
    }));
    return { days, total: sum(list.map((u) => u.total)) };
  }, [usage, tunnelId, range]);

  const today = days[days.length - 1] ?? { up: 0, down: 0 };
  const inRange = sum(days);

  return (
    <Card className={cx("flex gap-5 p-4", className)}>
      <div className="flex w-40 shrink-0 flex-col justify-center gap-4">
        <Summary label={t("usage.total")} value={total} primary />
        <Summary label={t("usage.today")} value={today} />
      </div>
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="mb-2 flex items-center gap-4 text-xs">
          <span className="min-w-0 flex-1 truncate">
            <span className="text-sm font-medium">{t("usage.daily")}</span>
            <span className="ml-2 text-muted tabular-nums">
              {t("usage.rangeTotal", { count: range, size: formatBytes(inRange.up + inRange.down) })}
            </span>
          </span>
          <span className="flex items-center gap-1.5 text-muted">
            <span className="h-0.5 w-3 rounded bg-up" />
            {t("common.upload")}
          </span>
          <span className="flex items-center gap-1.5 text-muted">
            <span className="h-0.5 w-3 rounded bg-down" />
            {t("common.download")}
          </span>
          <div className="inline-flex rounded-md bg-subtle p-0.5">
            {RANGES.map((n) => (
              <button
                key={n}
                type="button"
                onClick={() => setRange(n)}
                className={cx(
                  "rounded px-2 py-0.5 transition-colors",
                  range === n ? "bg-surface font-medium text-ink shadow-sm" : "text-muted hover:text-ink",
                )}
              >
                {t("usage.days", { count: n })}
              </button>
            ))}
          </div>
        </div>
        <div className="min-h-0 flex-1">
          <UsageChart days={days} />
        </div>
      </div>
    </Card>
  );
}
