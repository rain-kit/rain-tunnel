import { useEffect, useMemo, useState } from "react";
import { trafficApi } from "./api";
import type { ByteCount, UsageView } from "./types";

/** 每日流量显示的天数。 */
const USAGE_DAYS = 30;

export interface DayUsage extends ByteCount {
  /** YYYY-MM-DD */
  date: string;
}

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

/** 每 2 秒刷新最近 USAGE_DAYS 天的每日流量和累计流量；tunnelId 为空时汇总全部隧道。 */
export function useDailyUsage(tunnelId: string) {
  const [usage, setUsage] = useState<UsageView | null>(null);

  useEffect(() => {
    let alive = true;
    const load = () =>
      trafficApi
        .usage(USAGE_DAYS)
        .then((u) => alive && setUsage(u))
        .catch(() => {});
    load();
    const timer = setInterval(load, 2000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);

  return useMemo(() => {
    if (!usage) return { days: [] as DayUsage[], total: { up: 0, down: 0 } };
    const list = Object.entries(usage.tunnels)
      .filter(([id]) => !tunnelId || id === tunnelId)
      .map(([, u]) => u);
    const days = lastDays(usage.today, USAGE_DAYS).map((date) => ({
      date,
      ...sum(list.map((u) => u.days[date]).filter(Boolean)),
    }));
    return { days, total: sum(list.map((u) => u.total)) };
  }, [usage, tunnelId]);
}
