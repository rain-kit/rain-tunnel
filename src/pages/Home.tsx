import { ArrowDown, ArrowUp, Cable, LineChart, List, Network, Play, Plus, Square } from "lucide-react";
import { useState, type ReactNode } from "react";
import type { Page } from "../components/Sidebar";
import TitleBar from "../components/TitleBar";
import TrafficChart from "../components/TrafficChart";
import UsageChart from "../components/UsageChart";
import { Button, Card, cx, Segmented, Select } from "../components/ui";
import { tunnelApi } from "../lib/api";
import { formatBytes, formatSpeed, isActive } from "../lib/format";
import { useI18n } from "../lib/i18n";
import { useToast, useTraffic, useTunnels } from "../lib/store";
import { useDailyUsage, type DayUsage } from "../lib/usage";

const CHART_HEIGHT = 180;

function Stat({
  label,
  value,
  sub,
  icon,
  tone,
  onClick,
}: {
  label: string;
  value: ReactNode;
  sub?: ReactNode;
  icon: ReactNode;
  tone: string;
  onClick?: () => void;
}) {
  const body = (
    <Card className={cx("flex items-start gap-3 p-4 text-left", onClick && "h-full transition-colors hover:border-accent/40")}>
      <div className={cx("flex size-9 shrink-0 items-center justify-center rounded-xl", tone)}>{icon}</div>
      <div className="min-w-0">
        <div className="text-xs text-muted">{label}</div>
        <div className="mt-0.5 truncate text-xl font-semibold tabular-nums">{value}</div>
        {sub && <div className="mt-0.5 truncate text-xs text-muted tabular-nums">{sub}</div>}
      </div>
    </Card>
  );
  if (!onClick) return body;
  return (
    <button type="button" className="block w-full cursor-pointer rounded-xl" onClick={onClick}>
      {body}
    </button>
  );
}

function Legend() {
  const { t } = useI18n();
  return (
    <>
      <span className="flex items-center gap-1.5 text-muted">
        <span className="h-0.5 w-3 rounded bg-up" />
        {t("common.upload")}
      </span>
      <span className="flex items-center gap-1.5 text-muted">
        <span className="h-0.5 w-3 rounded bg-down" />
        {t("common.download")}
      </span>
    </>
  );
}

function DailyTable({ days }: { days: DayUsage[] }) {
  const { t } = useI18n();
  const rows = [...days].reverse();
  const th = "px-3 py-2 font-medium shadow-[inset_0_-1px_0_var(--color-line)]";
  return (
    <div className="overflow-y-auto" style={{ height: CHART_HEIGHT }}>
      <table className="w-full table-fixed text-left text-[13px]">
        <thead className="sticky top-0 z-10 bg-surface text-xs text-muted">
          <tr>
            <th className={th}>{t("usage.col.date")}</th>
            <th className={cx(th, "text-right")}>{t("common.upload")}</th>
            <th className={cx(th, "text-right")}>{t("common.download")}</th>
            <th className={cx(th, "text-right")}>{t("usage.col.sum")}</th>
          </tr>
        </thead>
        <tbody className="divide-y divide-line text-xs tabular-nums">
          {rows.map((d, i) => (
            <tr key={d.date} className="hover:bg-subtle/60">
              <td className="px-3 py-1.5">
                <span className="inline-flex items-center gap-2">
                  {d.date}
                  {i === 0 && <span className="rounded bg-accent/10 px-1.5 text-[10px] font-medium text-accent">{t("usage.today")}</span>}
                </span>
              </td>
              <td className="px-3 py-1.5 text-right text-amber-600 dark:text-amber-400">{formatBytes(d.up)}</td>
              <td className="px-3 py-1.5 text-right text-emerald-600 dark:text-emerald-400">{formatBytes(d.down)}</td>
              <td className="px-3 py-1.5 text-right font-medium">{formatBytes(d.up + d.down)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function DailyCard() {
  const { t } = useI18n();
  const { tunnels } = useTunnels();
  const [tunnelId, setTunnelId] = useState("");
  const [view, setView] = useState<"chart" | "table">("chart");
  const filterable = tunnels.length > 1;
  const { days, total } = useDailyUsage(filterable ? tunnelId : "");
  return (
    <Card className="mt-3 p-4">
      <div className="mb-2 flex items-center gap-3 text-xs">
        <span className="flex min-w-0 flex-1 items-center gap-2.5 truncate tabular-nums">
          <span className="text-sm font-medium">{t("usage.daily")}</span>
          <span className="text-muted">
            {t("usage.total")} <span className="font-semibold text-ink">{formatBytes(total.up + total.down)}</span>
          </span>
          <span className="inline-flex items-center gap-0.5 font-semibold text-ink">
            <ArrowUp size={11} className="text-amber-500" />
            {formatBytes(total.up)}
          </span>
          <span className="inline-flex items-center gap-0.5 font-semibold text-ink">
            <ArrowDown size={11} className="text-emerald-500" />
            {formatBytes(total.down)}
          </span>
        </span>
        {view === "chart" && <Legend />}
        {filterable && (
          <Select
            className="w-40"
            value={tunnelId}
            onChange={setTunnelId}
            options={[{ value: "", label: t("conn.allTunnels") }, ...tunnels.map((x) => ({ value: x.id, label: x.name }))]}
          />
        )}
        <Segmented
          value={view}
          onChange={setView}
          options={[
            { value: "chart", label: <><LineChart size={14} />{t("usage.view.chart")}</> },
            { value: "table", label: <><List size={14} />{t("usage.view.table")}</> },
          ]}
        />
      </div>
      {view === "chart" ? (
        <div style={{ height: CHART_HEIGHT }}>
          <UsageChart days={days} />
        </div>
      ) : (
        <DailyTable days={days} />
      )}
    </Card>
  );
}

export default function Home({ onNew, onNavigate }: { onNew: () => void; onNavigate: (page: Page) => void }) {
  const { t } = useI18n();
  const { tunnels } = useTunnels();
  const { current, history } = useTraffic();
  const toast = useToast();
  const running = tunnels.filter((x) => isActive(x.status.state)).length;
  const connected = tunnels.filter((x) => x.status.state === "connected").length;
  const problems = tunnels.filter((x) => x.status.state === "failed" || x.status.state === "reconnecting").length;

  return (
    <>
      <TitleBar
        title={t("nav.home")}
        actions={
          <>
            {tunnels.length > 0 &&
              (running > 0 ? (
                <Button icon={<Square size={13} />} onClick={() => tunnelApi.stopAll().catch(toast.error)}>
                  {t("common.stopAll")}
                </Button>
              ) : (
                <Button icon={<Play size={13} />} onClick={() => tunnelApi.startAll().catch(toast.error)}>
                  {t("common.startAll")}
                </Button>
              ))}
            <Button variant="primary" icon={<Plus size={15} />} onClick={onNew}>
              {t("common.newTunnel")}
            </Button>
          </>
        }
      />
      <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-6">
        <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
          <Stat
            label={t("nav.tunnels")}
            value={`${connected} / ${tunnels.length}`}
            sub={
              problems > 0 ? (
                <span className="text-amber-600 dark:text-amber-400">{t("common.problems", { count: problems })}</span>
              ) : (
                t("home.running", { count: running })
              )
            }
            icon={<Cable size={18} />}
            tone="bg-accent/10 text-accent"
            onClick={() => onNavigate("tunnels")}
          />
          <Stat
            label={t("home.activeConns")}
            value={current?.active ?? 0}
            icon={<Network size={18} />}
            tone="bg-violet-500/10 text-violet-600 dark:text-violet-400"
            onClick={() => onNavigate("connections")}
          />
          <Stat
            label={t("common.upload")}
            value={formatSpeed(current?.up ?? 0)}
            sub={t("home.total", { size: formatBytes(current?.totalUp ?? 0) })}
            icon={<ArrowUp size={18} />}
            tone="bg-amber-500/10 text-amber-600 dark:text-amber-400"
          />
          <Stat
            label={t("common.download")}
            value={formatSpeed(current?.down ?? 0)}
            sub={t("home.total", { size: formatBytes(current?.totalDown ?? 0) })}
            icon={<ArrowDown size={18} />}
            tone="bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"
          />
        </div>

        <Card className="mt-3 p-4">
          <div className="mb-2 flex items-center gap-4 text-xs">
            <span className="flex-1 text-sm font-medium">{t("home.realtime")}</span>
            <Legend />
          </div>
          <TrafficChart history={history} height={CHART_HEIGHT} />
        </Card>

        <DailyCard />
      </div>
    </>
  );
}
