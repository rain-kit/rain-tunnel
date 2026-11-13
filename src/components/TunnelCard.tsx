import { ArrowDown, ArrowRight, ArrowUp, Copy, CopyPlus, GripVertical, Link2, MoreHorizontal, Pencil, Play, Square, Terminal, Trash2 } from "lucide-react";
import { useState } from "react";
import { tunnelApi } from "../lib/api";
import { formatBytes, formatDuration, isActive, isWindows, proxyEnvCommand, proxyUrl, routeOf, shellLabel } from "../lib/format";
import { useI18n } from "../lib/i18n";
import { useSettings, useToast } from "../lib/store";
import type { TunnelView } from "../lib/types";
import { StatusBadge, TypeBadge } from "./badges";
import { Button, Card, Confirm, cx, IconButton, Menu, type MenuItem } from "./ui";

interface Props {
  tunnel: TunnelView;
  onEdit: (t: TunnelView) => void;
  dragging?: boolean;
  dragHandle?: React.HTMLAttributes<HTMLSpanElement>;
}

export default function TunnelCard({ tunnel, onEdit, dragging, dragHandle }: Props) {
  const { t, tm, tx } = useI18n();
  const toast = useToast();
  const shell = useSettings().settings?.terminal ?? (isWindows ? "powershell" : "bash");
  const [busy, setBusy] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const route = routeOf(tunnel);
  const st = tunnel.status.state;
  const active = isActive(st);

  const run = async (fn: () => Promise<unknown>, ok?: string) => {
    setBusy(true);
    try {
      await fn();
      if (ok) toast.success(ok);
    } catch (e) {
      toast.error(e);
    } finally {
      setBusy(false);
    }
  };

  const copy = async (text: string, ok = t("common.copiedText", { text })) => {
    try {
      await navigator.clipboard.writeText(text);
      toast.success(ok);
    } catch (e) {
      toast.error(e);
    }
  };

  const items: MenuItem[] = [
    { label: t("common.edit"), icon: <Pencil size={14} />, onClick: () => onEdit(tunnel) },
    {
      label: t("card.duplicate"),
      icon: <CopyPlus size={14} />,
      onClick: () => run(() => tunnelApi.duplicate(tunnel.id), t("card.duplicated")),
    },
  ];
  if (tunnel.type === "dynamic") {
    items.push({
      label: t("card.copyEnv"),
      icon: <Terminal size={14} />,
      onClick: () => copy(proxyEnvCommand(tunnel, shell), t("card.envCopied", { shell: shellLabel[shell] })),
    });
    items.push({ label: t("card.copySocks"), icon: <Link2 size={14} />, onClick: () => copy(proxyUrl(tunnel)) });
    if (tunnel.enableHttp) {
      items.push({ label: t("card.copyHttp"), icon: <Copy size={14} />, onClick: () => copy(proxyUrl(tunnel, "http")) });
    }
  }
  items.push({ label: t("common.delete"), icon: <Trash2 size={14} />, danger: true, onClick: () => setConfirmDelete(true) });

  return (
    <Card className={cx("group p-4 transition-shadow", dragging ? "opacity-40" : "hover:shadow-md")}>
      <div className="flex items-start gap-2">
        <span
          {...dragHandle}
          className="-ml-1.5 mt-0.5 cursor-grab text-muted/0 transition-colors group-hover:text-muted/60 active:cursor-grabbing"
          title={t("card.dragSort")}
        >
          <GripVertical size={16} />
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="truncate font-semibold" title={tunnel.name}>
              {tunnel.name}
            </span>
            <TypeBadge type={tunnel.type} />
            {tunnel.autoStart && <span className="text-[11px] text-muted">{t("common.autoStart")}</span>}
          </div>
          <div className="mt-1.5 flex items-center gap-1.5 overflow-hidden font-mono text-xs whitespace-nowrap text-muted [&>span]:truncate">
            <span title={route.from}>{route.from}</span>
            <ArrowRight size={12} className="shrink-0" />
            <span title={`${tunnel.sshUser}@${route.via}`}>{route.via}</span>
            <ArrowRight size={12} className="shrink-0" />
            <span title={route.to}>{route.to}</span>
          </div>
        </div>
        <div className="flex items-center gap-1">
          {active ? (
            <Button size="sm" icon={<Square size={12} fill="currentColor" />} disabled={busy} onClick={() => run(() => tunnelApi.stop(tunnel.id))}>
              {t("common.stop")}
            </Button>
          ) : (
            <Button
              size="sm"
              variant="primary"
              icon={<Play size={12} fill="currentColor" />}
              disabled={busy}
              onClick={() => run(() => tunnelApi.start(tunnel.id))}
            >
              {t("common.start")}
            </Button>
          )}
          <Menu
            items={items}
            trigger={(open) => (
              <IconButton label={t("common.more")} size="sm" onClick={open}>
                <MoreHorizontal size={16} />
              </IconButton>
            )}
          />
        </div>
      </div>

      <div className="mt-3 flex items-center gap-4 border-t border-line pt-3 text-xs">
        <StatusBadge state={st} attempt={tunnel.status.attempt} />
        {st === "connected" && <span className="text-muted">{t("card.connectedFor", { duration: formatDuration(tunnel.status.since) })}</span>}
        <span className="flex-1" />
        {active && (
          <span className="flex items-center gap-3 text-muted tabular-nums">
            <span>{t("common.connections", { count: tunnel.activeConns })}</span>
            <span className="flex items-center gap-0.5" title={t("card.totalUp")}>
              <ArrowUp size={12} className="text-amber-500" />
              {formatBytes(tunnel.bytesUp)}
            </span>
            <span className="flex items-center gap-0.5" title={t("card.totalDown")}>
              <ArrowDown size={12} className="text-emerald-500" />
              {formatBytes(tunnel.bytesDown)}
            </span>
          </span>
        )}
      </div>
      {tunnel.status.error && (st === "failed" || st === "reconnecting") && (
        <div
          className={cx(
            "selectable mt-2.5 rounded-lg px-2.5 py-1.5 text-xs break-words",
            st === "failed" ? "bg-red-500/8 text-red-700 dark:text-red-300" : "bg-amber-500/10 text-amber-700 dark:text-amber-300",
          )}
        >
          {tm(tunnel.status.error)}
        </div>
      )}

      <Confirm
        open={confirmDelete}
        title={t("card.deleteTitle")}
        message={tx("card.deleteMessage", { name: <span className="font-medium text-ink">{tunnel.name}</span> })}
        confirmText={t("common.delete")}
        danger
        onConfirm={() => run(() => tunnelApi.remove(tunnel.id), t("common.deleted"))}
        onClose={() => setConfirmDelete(false)}
      />
    </Card>
  );
}
