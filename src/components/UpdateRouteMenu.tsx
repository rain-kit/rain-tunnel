import { Globe, Waypoints } from "lucide-react";
import type { ReactNode } from "react";
import { useI18n } from "../lib/i18n";
import { useTunnels } from "../lib/store";
import { Menu, type MenuItem } from "./ui";

/** 检查更新、直接更新前选择访问 GitHub 的方式：直接访问，或经已连接的动态转发隧道。 */
export default function UpdateRouteMenu({
  onPick,
  children,
}: {
  onPick: (via?: string) => void;
  children: (open: () => void) => ReactNode;
}) {
  const { t } = useI18n();
  const { tunnels } = useTunnels();
  const proxies = tunnels.filter((tn) => tn.type === "dynamic" && tn.status?.state === "connected");
  const items: MenuItem[] = [
    { label: t("update.direct"), icon: <Globe size={14} />, onClick: () => onPick() },
    ...proxies.map((tn) => ({
      label: t("update.viaTunnel", { name: tn.name }),
      icon: <Waypoints size={14} />,
      onClick: () => onPick(tn.id),
    })),
  ];
  if (!proxies.length) items.push({ label: t("update.noTunnel"), disabled: true, onClick: () => {} });
  return <Menu trigger={children} items={items} />;
}
