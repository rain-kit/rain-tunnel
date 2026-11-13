import { ArrowDown, ArrowUp, Cable, Gauge, LayoutDashboard, ScrollText, Settings, type LucideIcon } from "lucide-react";
import { formatBytes, formatSpeed, isMac } from "../lib/format";
import { useI18n } from "../lib/i18n";
import { useTraffic, useTunnels } from "../lib/store";
import TrafficChart from "./TrafficChart";
import { cx } from "./ui";

export type Page = "home" | "tunnels" | "connections" | "logs" | "settings";

const nav: { id: Page; icon: LucideIcon }[] = [
  { id: "home", icon: LayoutDashboard },
  { id: "tunnels", icon: Cable },
  { id: "connections", icon: Gauge },
  { id: "logs", icon: ScrollText },
  { id: "settings", icon: Settings },
];

export default function Sidebar({ page, onChange }: { page: Page; onChange: (p: Page) => void }) {
  const { t } = useI18n();
  const { tunnels } = useTunnels();
  const { current, history } = useTraffic();
  const connected = tunnels.filter((t) => t.status.state === "connected").length;
  const problems = tunnels.filter((t) => t.status.state === "failed" || t.status.state === "reconnecting").length;

  return (
    <aside className="flex w-52 shrink-0 flex-col border-r border-line bg-surface/60">
      <div data-tauri-drag-region className={cx("flex items-center gap-3 px-5", isMac ? "h-14 pt-6 pb-1 box-content" : "h-16")}>
        <img src="/logo.svg" alt="" className="pointer-events-none size-8" />
        <span data-tauri-drag-region className="text-lg font-bold tracking-tight">
          Rain Tunnel
        </span>
      </div>

      <nav className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto px-2.5 pt-2 pb-4">
        {nav.map(({ id, icon: Icon }) => (
          <button
            key={id}
            onClick={() => onChange(id)}
            className={cx(
              "relative flex h-12 shrink-0 items-center justify-center rounded-lg px-2 text-base font-bold text-ink transition-colors",
              page === id ? "bg-accent/15 dark:bg-accent/35" : "hover:bg-subtle",
            )}
          >
            <Icon size={20} strokeWidth={2} className="absolute left-3.5" />
            <span>{t(`nav.${id}`)}</span>
            {id === "tunnels" && tunnels.length > 0 && (
              <span
                className={cx(
                  "absolute right-3 rounded-full px-1.5 text-xs font-medium tabular-nums",
                  problems > 0 ? "bg-amber-500/15 text-amber-600 dark:text-amber-400" : "bg-subtle text-muted",
                )}
                title={problems > 0 ? t("common.problems", { count: problems }) : t("sidebar.connected", { count: connected })}
              >
                {connected}/{tunnels.length}
              </span>
            )}
          </button>
        ))}
      </nav>

      <div className="shrink-0 px-5 pb-4" data-tauri-drag-region>
        <div className="-mx-1">
          <TrafficChart history={history} height={48} compact />
        </div>
        <div className="mt-2 flex justify-between gap-1 text-[11px] whitespace-nowrap tabular-nums">
          <span className="flex items-center gap-0.5 text-amber-600 dark:text-amber-400" title={t("common.upload")}>
            <ArrowUp size={12} />
            {formatSpeed(current?.up ?? 0)}
          </span>
          <span className="flex items-center gap-0.5 text-emerald-600 dark:text-emerald-400" title={t("common.download")}>
            <ArrowDown size={12} />
            {formatSpeed(current?.down ?? 0)}
          </span>
        </div>
        <div className="mt-1.5 flex justify-between text-[11px] text-muted tabular-nums">
          <span>{t("common.connections", { count: current?.active ?? 0 })}</span>
          <span title={t("sidebar.memoryTitle")}>{t("sidebar.memory", { size: formatBytes(current?.memory ?? 0, 0) })}</span>
        </div>
      </div>
    </aside>
  );
}
