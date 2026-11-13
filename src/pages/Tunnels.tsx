import { Cable, Play, Plus, Search, Square } from "lucide-react";
import { useMemo, useState } from "react";
import TitleBar from "../components/TitleBar";
import TunnelCard from "../components/TunnelCard";
import { Button, Empty, Input } from "../components/ui";
import { tunnelApi } from "../lib/api";
import { isActive } from "../lib/format";
import { useI18n } from "../lib/i18n";
import { useToast, useTunnels } from "../lib/store";
import type { TunnelView } from "../lib/types";

export default function Tunnels({ onEdit }: { onEdit: (t: TunnelView | null) => void }) {
  const { t, tm } = useI18n();
  const { tunnels, loaded, reorder } = useTunnels();
  const toast = useToast();
  const [q, setQ] = useState("");
  const [dragId, setDragId] = useState<string | null>(null);
  const [order, setOrder] = useState<string[] | null>(null);

  const filtered = useMemo(() => {
    const s = q.trim().toLowerCase();
    if (!s) return tunnels;
    return tunnels.filter((x) =>
      [x.name, x.sshHost, x.sshUser, x.targetHost ?? "", String(x.listenPort), String(x.targetPort ?? "")].some((v) =>
        v.toLowerCase().includes(s),
      ),
    );
  }, [tunnels, q]);

  // 拖动过程中按临时顺序显示，松开后提交。
  const shown = order ? order.map((id) => filtered.find((x) => x.id === id)!).filter(Boolean) : filtered;
  const canDrag = !q.trim();
  const anyRunning = tunnels.some((x) => isActive(x.status.state));
  const allRunning = tunnels.length > 0 && tunnels.every((x) => isActive(x.status.state));

  const startAll = async () => {
    try {
      const failed = await tunnelApi.startAll();
      const names = Object.keys(failed);
      if (names.length) {
        const list = names.map((name) => t("tunnels.failedItem", { name, error: tm(failed[name]) })).join(t("tunnels.failedSep"));
        toast.error(t("tunnels.startFailed", { count: names.length, list }));
      }
    } catch (e) {
      toast.error(e);
    }
  };

  const dragProps = (id: string): React.HTMLAttributes<HTMLSpanElement> | undefined =>
    canDrag
      ? {
          draggable: true,
          onDragStart: (e) => {
            e.dataTransfer.effectAllowed = "move";
            const card = (e.currentTarget as HTMLElement).closest("[data-card]");
            if (card) e.dataTransfer.setDragImage(card, 20, 20);
            setDragId(id);
            setOrder(tunnels.map((x) => x.id));
          },
          onDragEnd: () => {
            if (order) reorder(order);
            setDragId(null);
            setOrder(null);
          },
        }
      : undefined;

  const onDragOver = (overId: string) => (e: React.DragEvent) => {
    if (!dragId || !order) return;
    e.preventDefault();
    if (overId === dragId) return;
    const next = order.filter((id) => id !== dragId);
    next.splice(next.indexOf(overId) + (order.indexOf(overId) > order.indexOf(dragId) ? 1 : 0), 0, dragId);
    setOrder(next);
  };

  return (
    <>
      <TitleBar
        title={t("nav.tunnels")}
        actions={
          tunnels.length > 0 && (
            <>
              <div className="w-52">
                <Input
                  placeholder={t("tunnels.searchPlaceholder")}
                  value={q}
                  onChange={(e) => setQ(e.target.value)}
                  suffix={<Search size={14} className="mr-1.5 text-muted" />}
                />
              </div>
              {anyRunning && (
                <Button icon={<Square size={13} />} onClick={() => tunnelApi.stopAll().catch(toast.error)}>
                  {t("common.stopAll")}
                </Button>
              )}
              {!allRunning && (
                <Button icon={<Play size={13} />} onClick={startAll}>
                  {t("common.startAll")}
                </Button>
              )}
              <Button variant="primary" icon={<Plus size={15} />} onClick={() => onEdit(null)}>
                {t("common.new")}
              </Button>
            </>
          )
        }
      />
      <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-6">
        {loaded && tunnels.length === 0 ? (
          <Empty
            icon={<Cable size={22} />}
            title={t("common.noTunnels")}
            desc={t("tunnels.emptyDesc")}
            action={
              <Button variant="primary" icon={<Plus size={15} />} onClick={() => onEdit(null)}>
                {t("common.newTunnel")}
              </Button>
            }
          />
        ) : shown.length === 0 && q ? (
          <Empty icon={<Search size={22} />} title={t("tunnels.noMatch")} />
        ) : (
          <div className="grid grid-cols-1 gap-3 xl:grid-cols-2">
            {shown.map((x) => (
              <div key={x.id} data-card onDragOver={onDragOver(x.id)} onDrop={(e) => e.preventDefault()}>
                <TunnelCard tunnel={x} onEdit={onEdit} dragging={dragId === x.id} dragHandle={dragProps(x.id)} />
              </div>
            ))}
          </div>
        )}
      </div>
    </>
  );
}
