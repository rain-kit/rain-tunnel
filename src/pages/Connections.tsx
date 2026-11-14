import { ArrowDown, ArrowUp, Gauge, Pause, Play, Search, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import TitleBar from "../components/TitleBar";
import UsagePanel from "../components/UsagePanel";
import { Button, Card, Confirm, cx, Empty, IconButton, Input, Select } from "../components/ui";
import { connApi } from "../lib/api";
import { formatBytes, formatDuration } from "../lib/format";
import { useI18n } from "../lib/i18n";
import { useToast, useTunnels } from "../lib/store";
import type { ConnectionView } from "../lib/types";

type SortKey = "start" | "up" | "down" | "target";

const protoLabel: Record<string, string> = {
  socks5: "SOCKS5",
  http: "HTTP",
  connect: "HTTPS",
};

export default function Connections() {
  const { t } = useI18n();
  const toast = useToast();
  const { tunnels } = useTunnels();
  const [list, setList] = useState<ConnectionView[]>([]);
  const [paused, setPaused] = useState(false);
  const [tunnelId, setTunnelId] = useState("");
  const [q, setQ] = useState("");
  const [sort, setSort] = useState<SortKey>("start");
  const [confirmAll, setConfirmAll] = useState(false);
  const [now, setNow] = useState(Date.now());

  useEffect(() => {
    if (paused) return;
    let alive = true;
    const load = () =>
      connApi
        .list()
        .then((l) => {
          if (alive) {
            setList(l);
            setNow(Date.now());
          }
        })
        .catch(() => {});
    load();
    const timer = setInterval(load, 1000);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [paused]);

  const rows = useMemo(() => {
    const s = q.trim().toLowerCase();
    const r = list.filter(
      (c) => (!tunnelId || c.tunnelId === tunnelId) && (!s || c.target.toLowerCase().includes(s) || c.client.toLowerCase().includes(s)),
    );
    const by: Record<SortKey, (a: ConnectionView, b: ConnectionView) => number> = {
      start: (a, b) => b.start.localeCompare(a.start),
      up: (a, b) => b.bytesUp - a.bytesUp,
      down: (a, b) => b.bytesDown - a.bytesDown,
      target: (a, b) => a.target.localeCompare(b.target),
    };
    return r.sort(by[sort]);
  }, [list, tunnelId, q, sort]);

  const close = async (c: ConnectionView) => {
    setList((l) => l.filter((x) => x.id !== c.id));
    try {
      await connApi.close(c.tunnelId, c.connId);
    } catch (e) {
      toast.error(e);
    }
  };

  const closeAll = async () => {
    try {
      const n = await connApi.closeAll();
      setList([]);
      toast.success(t("conn.closedCount", { count: n }));
    } catch (e) {
      toast.error(e);
    }
  };

  const protoName = (p: string) => (p === "local" || p === "remote" ? t(`conn.proto.${p}`) : (protoLabel[p] ?? p));

  const th = (key: SortKey | null, label: string, cls = "") => (
    <th className={cx("px-3 py-2 font-medium shadow-[inset_0_-1px_0_var(--color-line)]", cls)}>
      {key ? (
        <button className={cx("hover:text-ink", sort === key && "text-ink")} onClick={() => setSort(key)}>
          {label}
          {sort === key && " ↓"}
        </button>
      ) : (
        label
      )}
    </th>
  );

  return (
    <>
      <TitleBar
        title={t("nav.connections")}
        actions={
          <>
            <Select
              className="w-40"
              value={tunnelId}
              onChange={setTunnelId}
              options={[{ value: "", label: t("conn.allTunnels") }, ...tunnels.map((x) => ({ value: x.id, label: x.name }))]}
            />
            <div className="w-48">
              <Input
                placeholder={t("conn.searchPlaceholder")}
                value={q}
                onChange={(e) => setQ(e.target.value)}
                suffix={<Search size={14} className="mr-1.5 text-muted" />}
              />
            </div>
            <Button icon={paused ? <Play size={13} /> : <Pause size={13} />} onClick={() => setPaused((v) => !v)}>
              {t(paused ? "common.resume" : "common.pause")}
            </Button>
            <Button variant="ghost" className="text-red-600 dark:text-red-400" disabled={list.length === 0} onClick={() => setConfirmAll(true)}>
              {t("conn.closeAll")}
            </Button>
          </>
        }
      />
      <div className="flex min-h-0 flex-1 flex-col gap-3 px-6 pb-6">
        <UsagePanel tunnelId={tunnelId} className="min-h-44 flex-1" />
        <div className="flex min-h-0 flex-2 flex-col">
          <Card className="min-h-0 flex-1 overflow-y-auto">
            {rows.length === 0 ? (
              <Empty
                icon={<Gauge size={22} />}
                title={t(list.length === 0 ? "conn.empty" : "conn.noMatch")}
                desc={list.length === 0 ? t("conn.emptyDesc") : undefined}
              />
            ) : (
              <table className="w-full table-fixed text-left text-[13px]">
                <thead className="sticky top-0 z-10 bg-surface text-xs text-muted">
                  <tr>
                    {th("target", t("conn.col.target"), "w-[34%]")}
                    {th(null, t("conn.col.tunnel"), "w-[16%]")}
                    {th(null, t("conn.col.client"), "w-[16%]")}
                    {th("start", t("conn.col.duration"), "w-[11%]")}
                    {th("up", t("common.upload"), "w-[10%] text-right")}
                    {th("down", t("common.download"), "w-[10%] text-right")}
                    <th className="w-10 shadow-[inset_0_-1px_0_var(--color-line)]" />
                  </tr>
                </thead>
                <tbody className="divide-y divide-line">
                  {rows.map((c) => (
                    <tr key={c.id} className="group hover:bg-subtle/60">
                      <td className="px-3 py-2">
                        <div className="flex items-center gap-2">
                          <span className="shrink-0 rounded bg-subtle px-1.5 text-[10px] font-medium text-muted">{protoName(c.proto)}</span>
                          <span className="selectable truncate font-mono text-xs" title={c.target}>
                            {c.target}
                          </span>
                        </div>
                      </td>
                      <td className="truncate px-3 py-2 text-muted" title={c.tunnelName}>
                        {c.tunnelName}
                      </td>
                      <td className="truncate px-3 py-2 font-mono text-xs text-muted" title={c.client}>
                        {c.client}
                      </td>
                      <td className="px-3 py-2 text-xs text-muted tabular-nums">{formatDuration(c.start, now)}</td>
                      <td className="px-3 py-2 text-right text-xs tabular-nums">
                        <span className="inline-flex items-center gap-0.5">
                          <ArrowUp size={11} className="text-amber-500" />
                          {formatBytes(c.bytesUp)}
                        </span>
                      </td>
                      <td className="px-3 py-2 text-right text-xs tabular-nums">
                        <span className="inline-flex items-center gap-0.5">
                          <ArrowDown size={11} className="text-emerald-500" />
                          {formatBytes(c.bytesDown)}
                        </span>
                      </td>
                      <td className="px-1">
                        <IconButton label={t("conn.disconnect")} size="sm" tone="danger" className="opacity-0 group-hover:opacity-100" onClick={() => close(c)}>
                          <X size={14} />
                        </IconButton>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </Card>
          {rows.length > 0 && (
            <div className="mt-2 text-right text-xs text-muted">
              {t("conn.total", { count: rows.length })}
              {paused && t("conn.paused")}
            </div>
          )}
        </div>
      </div>
      <Confirm
        open={confirmAll}
        title={t("conn.closeAllTitle")}
        message={t("conn.closeAllMessage")}
        confirmText={t("conn.closeAll")}
        danger
        onConfirm={closeAll}
        onClose={() => setConfirmAll(false)}
      />
    </>
  );
}
