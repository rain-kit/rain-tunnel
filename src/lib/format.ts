import { t } from "./i18n";
import type { ShellType, TunnelState, TunnelType, TunnelView } from "./types";

export function formatBytes(n: number, fractionDigits = 1): string {
  if (!Number.isFinite(n) || n < 0) n = 0;
  const units = ["B", "KB", "MB", "GB", "TB"];
  let i = 0;
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024;
    i++;
  }
  return `${i === 0 ? n.toFixed(0) : n.toFixed(fractionDigits)} ${units[i]}`;
}

export const formatSpeed = (n: number) => `${formatBytes(n)}/s`;

export function formatDuration(fromIso: string, now = Date.now()): string {
  const sec = Math.max(0, Math.floor((now - new Date(fromIso).getTime()) / 1000));
  if (sec < 60) return t("duration.s", { s: sec });
  const min = Math.floor(sec / 60);
  if (min < 60) return t("duration.ms", { m: min, s: sec % 60 });
  const h = Math.floor(min / 60);
  if (h < 24) return t("duration.hm", { h, m: min % 60 });
  return t("duration.dh", { d: Math.floor(h / 24), h: h % 24 });
}

export function formatTime(iso: string, withMs = false): string {
  const d = new Date(iso);
  const p = (n: number, w = 2) => String(n).padStart(w, "0");
  const s = `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
  return withMs ? `${s}.${p(d.getMilliseconds(), 3)}` : s;
}

export const typeLabel = (type: TunnelType) => t(`type.${type}`);

export const typeShort: Record<TunnelType, string> = {
  local: "-L",
  remote: "-R",
  dynamic: "-D",
};

export const stateLabel = (state: TunnelState) => t(`state.${state}`);

export function hostPort(host: string | undefined, port: number | undefined): string {
  const h = host || "";
  return h.includes(":") ? `[${h}]:${port ?? ""}` : `${h}:${port ?? ""}`;
}

/** 隧道链路的三段：起点 → SSH 服务器 → 终点。 */
export function routeOf(tunnel: TunnelView) {
  const listen = tunnel.status?.listenAddr || hostPort(tunnel.listenHost, tunnel.listenPort);
  const ssh = hostPort(tunnel.sshHost, tunnel.sshPort);
  const target = hostPort(tunnel.targetHost, tunnel.targetPort);
  switch (tunnel.type) {
    case "local":
      return { from: t("route.local", { addr: listen }), via: ssh, to: target };
    case "remote":
      return {
        from: t("route.server", { addr: hostPort(tunnel.listenHost, tunnel.listenPort) }),
        via: ssh,
        to: t("route.reachable", { addr: target }),
      };
    default:
      return { from: t("route.local", { addr: listen }), via: ssh, to: t(tunnel.enableHttp ? "route.anySocksHttp" : "route.anySocks") };
  }
}

/** 动态转发的代理地址，监听 0.0.0.0 时给出本机地址。 */
export function proxyUrl(t: TunnelView, scheme: "socks5" | "http" = "socks5"): string {
  let addr = t.status?.listenAddr || hostPort(t.listenHost, t.listenPort);
  addr = addr.replace(/^0\.0\.0\.0:/, "127.0.0.1:").replace(/^\[::\]:/, "127.0.0.1:");
  return `${scheme}://${addr}`;
}

export const shellLabel: Record<ShellType, string> = {
  bash: "Bash / Zsh",
  fish: "Fish",
  nushell: "Nushell",
  powershell: "PowerShell",
  cmd: "CMD",
};

/** 当前系统可选的终端，与 src-tauri/src/config.rs 的 TERMINALS 一致。 */
export const shellOptions = (): ShellType[] =>
  isWindows ? ["powershell", "cmd", "bash", "fish", "nushell"] : ["bash", "fish", "nushell"];

/**
 * 把动态转发设为终端代理的命令，格式参考 Clash Verge Rev 的“复制环境变量”。
 * 开启 HTTP 代理时 http(s)_proxy 用 HTTP，all_proxy 始终用 SOCKS5。
 */
export function proxyEnvCommand(t: TunnelView, shell: ShellType): string {
  const socks = proxyUrl(t);
  const http = proxyUrl(t, "http");
  const vars: [string, string][] = t.enableHttp ? [["http_proxy", http], ["https_proxy", http]] : [];
  vars.push(["all_proxy", socks]);
  // IPv6 地址带 []，zsh 会当成通配符。
  const q = (v: string) => (/[[\]]/.test(v) ? `"${v}"` : v);
  switch (shell) {
    case "powershell":
      return vars.map(([k, v]) => `$env:${k.toUpperCase()}="${v}"`).join("; ");
    case "cmd":
      // 引号包住整个赋值，避免 & 前的空格进入变量值。
      return vars.map(([k, v]) => `set "${k.toUpperCase()}=${v}"`).join(" & ");
    case "fish":
      return vars.map(([k, v]) => `set -gx ${k} ${q(v)}`).join("; ");
    case "nushell":
      return `load-env {${vars.map(([k, v]) => `${k}: "${v}"`).join(", ")}}`;
    default:
      return `export ${vars.map(([k, v]) => `${k}=${q(v)}`).join(" ")}`;
  }
}

export const isActive = (s: TunnelState) => s !== "stopped";

export const isMac = navigator.userAgent.includes("Mac");
export const isWindows = navigator.userAgent.includes("Windows");
