import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { tm } from "./i18n";
import type {
  ConnectionView,
  HostKeyPrompt,
  I18nMsg,
  LogEntry,
  SaveTunnelReq,
  Settings,
  SettingsInput,
  Traffic,
  TunnelStatus,
  TunnelView,
  UpdateInfo,
} from "./types";

/** 后端命令失败时返回 { code, i18n, params }，见 src-tauri/src/error.rs。 */
export interface ApiError extends I18nMsg {
  code: number;
}

export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "i18n" in e) return tm(e as ApiError);
  if (e instanceof Error) return e.message;
  return String(e);
}

export const tunnelApi = {
  list: () => invoke<TunnelView[]>("tunnel_list"),
  get: (id: string) => invoke<TunnelView>("tunnel_get", { id }),
  save: (req: SaveTunnelReq) => invoke<TunnelView>("tunnel_save", { req }),
  duplicate: (id: string) => invoke<TunnelView>("tunnel_duplicate", { id }),
  remove: (id: string) => invoke<void>("tunnel_delete", { id }),
  reorder: (ids: string[]) => invoke<void>("tunnel_reorder", { ids }),
  start: (id: string) => invoke<void>("tunnel_start", { id }),
  stop: (id: string) => invoke<void>("tunnel_stop", { id }),
  /** 返回启动失败的隧道名 → 原因。 */
  startAll: () => invoke<Record<string, I18nMsg>>("tunnel_start_all"),
  stopAll: () => invoke<void>("tunnel_stop_all"),
};

export const hostKeyApi = {
  pending: () => invoke<HostKeyPrompt[]>("hostkey_pending"),
  answer: (requestId: string, accept: boolean) => invoke<void>("hostkey_answer", { requestId, accept }),
};

export const connApi = {
  list: () => invoke<ConnectionView[]>("conn_list"),
  close: (tunnelId: string, connId: number) => invoke<void>("conn_close", { tunnelId, connId }),
  closeAll: () => invoke<number>("conn_close_all"),
};

export const trafficApi = {
  current: () => invoke<Traffic>("traffic_current"),
  history: () => invoke<{ t: number; up: number; down: number }[]>("traffic_history"),
};

export const settingsApi = {
  get: () => invoke<Settings>("settings_get"),
  save: (input: SettingsInput) => invoke<Settings>("settings_save", { input }),
  setAutoLaunch: (enable: boolean) => invoke<Settings>("settings_set_autolaunch", { enable }),
  openDataDir: () => invoke<void>("open_data_dir"),
  openLogDir: () => invoke<void>("open_log_dir"),
  pickKeyFile: () => invoke<string>("pick_key_file"),
};

export const logApi = {
  list: () => invoke<LogEntry[]>("logs_list"),
  clear: () => invoke<void>("logs_clear"),
};

export const updateApi = {
  status: () => invoke<UpdateInfo>("update_status"),
  /** via 为动态转发隧道 ID 时经它访问 GitHub，为空时直接访问。 */
  check: (via?: string) => invoke<UpdateInfo>("update_check", { via: via ?? null }),
  /** 托盘是否请求打开更新页面，取出后清除。 */
  takeOpenRequest: () => invoke<boolean>("update_take_open_request"),
  /** 下载并安装，成功后程序会重启，不会正常返回。 */
  install: (via?: string) => invoke<void>("update_install", { via: via ?? null }),
  openReleasePage: () => invoke<void>("open_release_page"),
};

/** 后端推送的事件，见 src-tauri/src/events.rs。 */
export const events = {
  onTunnelStatus: (fn: (s: TunnelStatus) => void) => sub<TunnelStatus>("tunnel:status", fn),
  onTunnelChanged: (fn: () => void) => sub<null>("tunnel:changed", () => fn()),
  onTraffic: (fn: (t: Traffic) => void) => sub<Traffic>("traffic", fn),
  onLog: (fn: (e: LogEntry) => void) => sub<LogEntry>("log", fn),
  onHostKey: (fn: (p: HostKeyPrompt) => void) => sub<HostKeyPrompt>("hostkey:prompt", fn),
  onUpdate: (fn: (u: UpdateInfo) => void) => sub<UpdateInfo>("update:status", fn),
  onUpdateProgress: (fn: (p: { downloaded: number; total?: number }) => void) => sub("update:progress", fn),
  onUpdateOpen: (fn: () => void) => sub<null>("update:open", () => fn()),
};

/** 订阅事件，返回同步的取消函数，方便在 useEffect 中直接使用。 */
function sub<T>(name: string, fn: (payload: T) => void): () => void {
  if (!isTauri()) return () => {};
  let un: UnlistenFn | undefined;
  let cancelled = false;
  listen<T>(name, (e) => fn(e.payload)).then((u) => {
    if (cancelled) u();
    else un = u;
  });
  return () => {
    cancelled = true;
    un?.();
  };
}
