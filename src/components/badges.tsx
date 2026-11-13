import { stateLabel, typeLabel, typeShort } from "../lib/format";
import { useI18n } from "../lib/i18n";
import type { TunnelState, TunnelType } from "../lib/types";
import { cx } from "./ui";

const stateStyle: Record<TunnelState, { dot: string; text: string }> = {
  stopped: { dot: "bg-zinc-400 dark:bg-zinc-500", text: "text-muted" },
  connecting: { dot: "bg-sky-500 animate-pulse", text: "text-sky-600 dark:text-sky-400" },
  connected: { dot: "bg-emerald-500", text: "text-emerald-600 dark:text-emerald-400" },
  reconnecting: { dot: "bg-amber-500 animate-pulse", text: "text-amber-600 dark:text-amber-400" },
  failed: { dot: "bg-red-500", text: "text-red-600 dark:text-red-400" },
};

export function StatusDot({ state, className }: { state: TunnelState; className?: string }) {
  return <span className={cx("inline-block size-2 shrink-0 rounded-full", stateStyle[state].dot, className)} />;
}

export function StatusBadge({ state, attempt }: { state: TunnelState; attempt?: number }) {
  const { t } = useI18n();
  const s = stateStyle[state];
  return (
    <span className={cx("inline-flex items-center gap-1.5 text-xs font-medium", s.text)}>
      <StatusDot state={state} />
      {stateLabel(state)}
      {state === "reconnecting" && attempt ? t("state.attempt", { count: attempt }) : ""}
    </span>
  );
}

export const typeStyle: Record<TunnelType, string> = {
  local: "bg-blue-500/10 text-blue-600 dark:text-blue-400",
  remote: "bg-violet-500/10 text-violet-600 dark:text-violet-400",
  dynamic: "bg-teal-500/10 text-teal-600 dark:text-teal-400",
};

export function TypeBadge({ type }: { type: TunnelType }) {
  useI18n();
  return (
    <span className={cx("inline-flex h-5 items-center gap-1 rounded-md px-1.5 text-[11px] font-medium", typeStyle[type])}>
      {typeLabel(type)}
      <span className="font-mono opacity-70">{typeShort[type]}</span>
    </span>
  );
}
