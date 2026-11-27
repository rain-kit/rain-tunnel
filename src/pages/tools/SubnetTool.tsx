import { Network } from "lucide-react";
import { useState, type ReactNode } from "react";
import { cx, Input } from "../../components/ui";
import { useI18n } from "../../lib/i18n";
import { calcSubnet, type Subnet } from "../../lib/tools";
import { CopyButton, ErrorText } from "./common";

const commonPrefixes = [8, 16, 24, 28, 30, 32];

type IpType = "private" | "loopback" | "linkLocal" | "multicast" | "public";

function classifyIp(ip: string): IpType {
  const [a, b] = ip.split(".").map(Number);
  if (a === 127) return "loopback";
  if (a === 10 || (a === 172 && b >= 16 && b <= 31) || (a === 192 && b === 168)) return "private";
  if (a === 169 && b === 254) return "linkLocal";
  if (a >= 224 && a <= 239) return "multicast";
  return "public";
}

function ipBits(ip: string): string {
  return ip
    .split(".")
    .map((o) => Number(o).toString(2).padStart(8, "0"))
    .join("");
}

function StatCard({ label, value, copyValue, tone }: { label: string; value: string; copyValue?: string; tone?: string }) {
  return (
    <div className="flex items-center justify-between gap-2 rounded-xl border border-line bg-surface px-3.5 py-2.5">
      <div className="min-w-0">
        <div className="truncate text-[11px] whitespace-nowrap text-muted">{label}</div>
        <div className={cx("selectable mt-0.5 truncate font-mono text-sm font-semibold tabular-nums", tone ?? "text-ink")}>
          {value || "-"}
        </div>
      </div>
      <CopyButton text={copyValue ?? value} />
    </div>
  );
}

function DetailCell({ label, value }: { label: ReactNode; value: string }) {
  return (
    <div className="flex items-center justify-between gap-2 px-3.5 py-2.5 transition-colors hover:bg-subtle/30">
      <div className="min-w-0">
        <div className="truncate text-[11px] whitespace-nowrap text-muted">{label}</div>
        <div className="selectable mt-0.5 truncate font-mono text-[13px] font-medium text-ink tabular-nums">{value || "-"}</div>
      </div>
      <CopyButton text={value} />
    </div>
  );
}

export default function SubnetTool() {
  const { t } = useI18n();
  const [value, setValue] = useState("192.168.1.10/24");

  let subnet: Subnet | null = null;
  try {
    if (value.trim()) subnet = calcSubnet(value);
  } catch {
    subnet = null;
  }
  const invalid = !!value.trim() && !subnet;
  const ipMatch = /^\s*(\d{1,3}(?:\.\d{1,3}){3})/.exec(value);
  const rawIp = subnet ? (ipMatch?.[1] ?? subnet.network) : "";
  const ipType = rawIp ? classifyIp(rawIp) : null;
  const bits = rawIp ? ipBits(rawIp) : "";

  const applyPrefix = (p: number) => {
    const ip = ipMatch?.[1] ?? "192.168.1.10";
    setValue(`${ip}/${p}`);
  };

  return (
    <div className="@container flex flex-col gap-3">
      <div className="rounded-xl border border-line bg-subtle/35 p-3.5">
        <div className="mb-2 flex flex-wrap items-center gap-2">
          <span className="flex size-6 shrink-0 items-center justify-center rounded-md bg-accent/10 text-accent">
            <Network size={13} />
          </span>
          <span className="text-xs font-semibold whitespace-nowrap text-ink">{t("tools.subnet.input")}</span>
          {ipType && (
            <span className="rounded-md bg-accent/10 px-1.5 py-0.5 text-[11px] font-medium whitespace-nowrap text-accent">
              {t(`tools.subnet.type.${ipType}`)}
            </span>
          )}
          <div className="ml-auto flex items-center gap-1">
            {commonPrefixes.map((p) => (
              <button
                key={p}
                type="button"
                onClick={() => applyPrefix(p)}
                className={cx(
                  "h-6 rounded-md border px-1.5 font-mono text-[11px] font-medium tabular-nums transition-colors",
                  subnet?.prefix === p
                    ? "border-accent/40 bg-accent/12 text-accent dark:bg-accent/25 dark:text-sky-300"
                    : "border-line bg-surface text-muted hover:bg-subtle hover:text-ink",
                )}
              >
                /{p}
              </button>
            ))}
          </div>
        </div>

        <Input mono autoFocus value={value} invalid={invalid} placeholder="10.0.0.1/16" onChange={(e) => setValue(e.target.value)} />
        <div className="mt-1.5">
          {invalid ? <ErrorText>{t("tools.subnet.invalid")}</ErrorText> : <div className="text-xs text-muted">{t("tools.subnet.hint")}</div>}
        </div>
      </div>

      {subnet && (
        <div className="rounded-xl border border-line bg-surface px-3.5 py-2.5">
          <div className="mb-1.5 flex items-center justify-between text-[11px]">
            <span className="inline-flex items-center gap-1.5 font-medium text-accent">
              <span className="size-2 rounded-full bg-accent" />
              {t("tools.subnet.netBits", { bits: subnet.prefix })}
            </span>
            <span className="inline-flex items-center gap-1.5 font-medium text-emerald-600 dark:text-emerald-400">
              <span className="size-2 rounded-full bg-emerald-500" />
              {t("tools.subnet.hostBits", { bits: 32 - subnet.prefix })}
            </span>
          </div>
          <div className="flex h-1.5 overflow-hidden rounded-full bg-subtle">
            <div className="bg-accent transition-all" style={{ width: `${(subnet.prefix / 32) * 100}%` }} />
            <div className="flex-1 bg-emerald-500/80" />
          </div>
          <div className="selectable mt-2 overflow-x-auto font-mono text-[11.5px] tracking-wide whitespace-nowrap">
            {Array.from({ length: 32 }, (_, i) => (
              <span key={i}>
                {i > 0 && i % 8 === 0 && <span className="mx-1 text-muted/60">.</span>}
                <span className={i < subnet.prefix ? "font-semibold text-accent" : "font-semibold text-emerald-600 dark:text-emerald-400"}>
                  {bits[i]}
                </span>
              </span>
            ))}
          </div>
        </div>
      )}

      <div className="grid grid-cols-1 gap-2.5 @min-[480px]:grid-cols-3">
        <StatCard label="CIDR" value={subnet ? `${subnet.network}/${subnet.prefix}` : ""} tone="text-accent" />
        <StatCard
          label={t("tools.subnet.hosts")}
          value={subnet ? subnet.hosts.toLocaleString() : ""}
          copyValue={subnet ? String(subnet.hosts) : ""}
          tone="text-emerald-600 dark:text-emerald-400"
        />
        <StatCard label={t("tools.subnet.mask")} value={subnet?.mask ?? ""} />
      </div>

      <div className="overflow-hidden rounded-xl border border-line bg-surface">
        <div className="grid grid-cols-1 divide-y divide-line border-b border-line @min-[480px]:grid-cols-2 @min-[480px]:divide-x @min-[480px]:divide-y-0">
          <DetailCell label={t("tools.subnet.network")} value={subnet?.network ?? ""} />
          <DetailCell label={t("tools.subnet.broadcast")} value={subnet?.broadcast ?? ""} />
        </div>
        <div className="grid grid-cols-1 divide-y divide-line @min-[480px]:grid-cols-2 @min-[480px]:divide-x @min-[480px]:divide-y-0">
          <DetailCell label={t("tools.subnet.range")} value={subnet ? `${subnet.first} - ${subnet.last}` : ""} />
          <DetailCell label={t("tools.subnet.wildcard")} value={subnet?.wildcard ?? ""} />
        </div>
      </div>
    </div>
  );
}
