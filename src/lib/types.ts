// 与 src-tauri/src/model.rs 对应。

import type { Language, Locale } from "./i18n";

/** 后端返回的国际化消息（src-tauri/src/i18n.rs 的 Msg），用 tm() 翻译。 */
export interface I18nMsg {
  i18n: string;
  params?: Record<string, string | I18nMsg>;
}

export type TunnelType = "local" | "remote" | "dynamic";
export type AuthType = "password" | "key" | "agent";
export type TunnelState = "stopped" | "connecting" | "connected" | "reconnecting" | "failed";

export interface Tunnel {
  id: string;
  name: string;
  type: TunnelType;
  sshHost: string;
  sshPort: number;
  sshUser: string;
  authType: AuthType;
  keyFile?: string;
  password?: string;
  keyPassphrase?: string;
  listenHost: string;
  listenPort: number;
  targetHost?: string;
  targetPort?: number;
  enableHttp?: boolean;
  proxyUser?: string;
  proxyPass?: string;
  autoStart: boolean;
  autoReconnect: boolean;
  keepaliveSeconds?: number;
  idleTimeoutSeconds?: number;
  maxConns?: number;
  createdAt?: string;
  updatedAt?: string;
}

export interface TunnelStatus {
  tunnelId: string;
  state: TunnelState;
  error?: I18nMsg;
  since: string;
  attempt: number;
  listenAddr: string;
}

export interface TunnelView extends Tunnel {
  hasPassword: boolean;
  hasKeyPassphrase: boolean;
  hasProxyPass: boolean;
  status: TunnelStatus;
  bytesUp: number;
  bytesDown: number;
  activeConns: number;
}

export interface SaveTunnelReq extends Tunnel {
  clearPassword?: boolean;
  clearKeyPassphrase?: boolean;
  clearProxyPass?: boolean;
}

export interface HostKeyPrompt {
  requestId: string;
  tunnelId: string;
  tunnelName: string;
  host: string;
  keyType: string;
  fingerprint: string;
}

export interface TunnelTraffic {
  up: number;
  down: number;
  totalUp: number;
  totalDown: number;
  active: number;
}

export interface Traffic {
  time?: string;
  up: number;
  down: number;
  totalUp: number;
  totalDown: number;
  active: number;
  memory: number;
  tunnels: Record<string, TunnelTraffic>;
}

export interface ConnectionView {
  id: string;
  tunnelId: string;
  tunnelName: string;
  connId: number;
  proto: string;
  client: string;
  target: string;
  start: string;
  bytesUp: number;
  bytesDown: number;
}

export type LogLevel = "debug" | "info" | "warn" | "error";

export interface LogEntry {
  seq: number;
  time: string;
  level: LogLevel;
  message: string;
  fields: string;
}

export type ThemeMode = "system" | "light" | "dark";

export interface Settings {
  logLevel: LogLevel;
  theme: ThemeMode;
  language: Language;
  /** 后端识别的系统语言，系统语言不受支持时为 en。 */
  systemLanguage: Locale;
  closeToTray: boolean;
  silentStart: boolean;
  autoLaunch: boolean;
  dataDir: string;
  logDir: string;
  version: string;
}

export type SettingsInput = Pick<Settings, "logLevel" | "theme" | "language" | "closeToTray" | "silentStart">;
