import { ArrowDown, ArrowUp, Cable, Network, Play, Plus, Square } from "lucide-react";
import type { ReactNode } from "react";
import { StatusDot, TypeBadge } from "../components/badges";
import TitleBar from "../components/TitleBar";
import TrafficChart from "../components/TrafficChart";
import { Button, Card, cx, Empty, IconButton } from "../components/ui";
import { tunnelApi } from "../lib/api";
import { formatBytes, formatSpeed, isActive, routeOf, stateLabel } from "../lib/format";
import { useI18n } from "../lib/i18n";
import { useToast, useTraffic, useTunnels } from "../lib/store";
import type { TunnelView } from "../lib/types";

function Stat({ label, value, sub, icon, tone }: { label: string; value: ReactNode; sub?: ReactNode; icon: ReactNode; tone: string }) {
  return (
    <Card className="flex items-start gap-3 p-4">
      <div className={cx("flex size-9 shrink-0 items-center justify-center rounded-xl", tone)}>{icon}</div>
      <div className="min-w-0">
        <div className="text-xs text-muted">{label}</div>
        <div className="mt-0.5 truncate text-xl font-semibold tabular-nums">{value}</div>
        {sub && <div className="mt-0.5 truncate text-xs text-muted tabular-nums">{sub}</div>}
      </div>
    </Card>
  );
}

function QuickRow({ tunnel }: { tunnel: TunnelView }) {
  const { t } = useI18n();
  const toast = useToast();
  const active = isActive(tunnel.status.state);
  const route = routeOf(tunnel);
  const toggle = () => (active ? tunnelApi.stop(tunnel.id) : tunnelApi.start(tunnel.id)).catch(toast.error);
  return (
    <div className="flex items-center gap-3 px-4 py-2.5">
      <StatusDot state={tunnel.status.state} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-[13px] font-medium">{tunnel.name}</span>
          <TypeBadge type={tunnel.type} />
        </div>
        <div className="truncate font-mono text-[11px] text-muted">{route.from}</div>
      </div>
      <span className="w-20 text-right text-xs text-muted">{stateLabel(tunnel.status.state)}</span>
      <span className="w-24 text-right text-xs text-muted tabular-nums">
        {active ? t("common.connections", { count: tunnel.activeConns }) : ""}
      </span>
      <IconButton label={t(active ? "common.stop" : "common.start")} size="sm" onClick={toggle}>
        {active ? <Square size={13} /> : <Play size={14} />}
      </IconButton>
    </div>
  );
}

export default function Home({ onNew, onOpenTunnels }: { onNew: () => void; onOpenTunnels: () => void }) {
  const { t } = useI18n();
  const { tunnels, loaded } = useTunnels();
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
          tunnels.length > 0 && (
            <>
              {running > 0 ? (
                <Button icon={<Square size={13} />} onClick={() => tunnelApi.stopAll().catch(toast.error)}>
                  {t("common.stopAll")}
                </Button>
              ) : (
                <Button icon={<Play size={13} />} onClick={() => tunnelApi.startAll().catch(toast.error)}>
                  {t("common.startAll")}
                </Button>
              )}
              <Button variant="primary" icon={<Plus size={15} />} onClick={onNew}>
                {t("common.newTunnel")}
              </Button>
            </>
          )
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
          />
          <Stat
            label={t("home.activeConns")}
            value={current?.active ?? 0}
            icon={<Network size={18} />}
            tone="bg-violet-500/10 text-violet-600 dark:text-violet-400"
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
            <span className="flex items-center gap-1.5 text-muted">
              <span className="h-0.5 w-3 rounded bg-up" />
              {t("common.upload")}
            </span>
            <span className="flex items-center gap-1.5 text-muted">
              <span className="h-0.5 w-3 rounded bg-down" />
              {t("common.download")}
            </span>
          </div>
          <TrafficChart history={history} height={180} />
        </Card>

        <Card className="mt-3">
          <div className="flex items-center border-b border-line px-4 py-3">
            <span className="flex-1 text-sm font-medium">{t("nav.tunnels")}</span>
            {tunnels.length > 0 && (
              <button className="text-xs text-accent hover:underline" onClick={onOpenTunnels}>
                {t("home.manageAll")}
              </button>
            )}
          </div>
          {loaded && tunnels.length === 0 ? (
            <Empty
              icon={<Cable size={22} />}
              title={t("common.noTunnels")}
              desc={t("home.emptyDesc")}
              action={
                <Button variant="primary" icon={<Plus size={15} />} onClick={onNew}>
                  {t("common.newTunnel")}
                </Button>
              }
            />
          ) : (
            <div className="divide-y divide-line">
              {tunnels.map((x) => (
                <QuickRow key={x.id} tunnel={x} />
              ))}
            </div>
          )}
        </Card>
      </div>
    </>
  );
}
