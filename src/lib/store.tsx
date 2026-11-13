import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { errorMessage, events, hostKeyApi, settingsApi, trafficApi, tunnelApi } from "./api";
import { setLanguage } from "./i18n";
import type { HostKeyPrompt, Settings, SettingsInput, Traffic, TunnelView } from "./types";

// ---------- 提示 ----------

export interface Toast {
  id: number;
  kind: "success" | "error" | "info";
  text: string;
}

interface ToastCtx {
  toasts: Toast[];
  success: (text: string) => void;
  error: (e: unknown) => void;
  info: (text: string) => void;
  dismiss: (id: number) => void;
}

const ToastContext = createContext<ToastCtx>(null!);
export const useToast = () => useContext(ToastContext);

function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const seq = useRef(0);
  const dismiss = useCallback((id: number) => setToasts((l) => l.filter((t) => t.id !== id)), []);
  const push = useCallback(
    (kind: Toast["kind"], text: string) => {
      const id = ++seq.current;
      setToasts((l) => [...l.slice(-3), { id, kind, text }]);
      setTimeout(() => dismiss(id), kind === "error" ? 6000 : 3000);
    },
    [dismiss],
  );
  const value = useMemo<ToastCtx>(
    () => ({
      toasts,
      dismiss,
      success: (t) => push("success", t),
      info: (t) => push("info", t),
      error: (e) => push("error", errorMessage(e)),
    }),
    [toasts, push, dismiss],
  );
  return <ToastContext.Provider value={value}>{children}</ToastContext.Provider>;
}

// ---------- 设置与主题 ----------

interface SettingsCtx {
  settings: Settings | null;
  dark: boolean;
  save: (patch: Partial<SettingsInput>) => Promise<void>;
  setAutoLaunch: (enable: boolean) => Promise<void>;
}

const SettingsContext = createContext<SettingsCtx>(null!);
export const useSettings = () => useContext(SettingsContext);

function useSystemDark() {
  const [dark, setDark] = useState(() => matchMedia("(prefers-color-scheme: dark)").matches);
  useEffect(() => {
    const mq = matchMedia("(prefers-color-scheme: dark)");
    const fn = () => setDark(mq.matches);
    mq.addEventListener("change", fn);
    return () => mq.removeEventListener("change", fn);
  }, []);
  return dark;
}

function SettingsProvider({ children }: { children: ReactNode }) {
  const toast = useToast();
  const [settings, setSettings] = useState<Settings | null>(null);
  const systemDark = useSystemDark();
  const theme = settings?.theme ?? "system";
  const dark = theme === "dark" || (theme === "system" && systemDark);

  useEffect(() => {
    settingsApi.get().then(setSettings).catch(toast.error);
  }, []);

  useEffect(() => {
    if (settings) setLanguage(settings.language, settings.systemLanguage);
  }, [settings?.language, settings?.systemLanguage]);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
    document.documentElement.style.colorScheme = dark ? "dark" : "light";
  }, [dark]);

  const save = useCallback(
    async (patch: Partial<SettingsInput>) => {
      if (!settings) return;
      const input: SettingsInput = {
        logLevel: settings.logLevel,
        theme: settings.theme,
        language: settings.language,
        closeToTray: settings.closeToTray,
        silentStart: settings.silentStart,
        ...patch,
      };
      setSettings({ ...settings, ...patch });
      try {
        setSettings(await settingsApi.save(input));
      } catch (e) {
        setSettings(settings);
        throw e;
      }
    },
    [settings],
  );

  const setAutoLaunch = useCallback(async (enable: boolean) => {
    setSettings(await settingsApi.setAutoLaunch(enable));
  }, []);

  const value = useMemo(() => ({ settings, dark, save, setAutoLaunch }), [settings, dark, save, setAutoLaunch]);
  return <SettingsContext.Provider value={value}>{children}</SettingsContext.Provider>;
}

// ---------- 隧道 ----------

interface TunnelsCtx {
  tunnels: TunnelView[];
  loaded: boolean;
  refresh: () => Promise<void>;
  /** 本地先调整顺序再提交，拖拽时界面不闪动。 */
  reorder: (ids: string[]) => Promise<void>;
}

const TunnelsContext = createContext<TunnelsCtx>(null!);
export const useTunnels = () => useContext(TunnelsContext);

function TunnelsProvider({ children }: { children: ReactNode }) {
  const toast = useToast();
  const [tunnels, setTunnels] = useState<TunnelView[]>([]);
  const [loaded, setLoaded] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setTunnels(await tunnelApi.list());
    } catch (e) {
      toast.error(e);
    } finally {
      setLoaded(true);
    }
  }, []);

  useEffect(() => {
    refresh();
    const offs = [
      events.onTunnelChanged(refresh),
      events.onTunnelStatus((s) =>
        setTunnels((list) => list.map((t) => (t.id === s.tunnelId ? { ...t, status: s } : t))),
      ),
      events.onTraffic((tr) =>
        setTunnels((list) =>
          list.map((t) => {
            const x = tr.tunnels[t.id];
            if (!x) return t.activeConns || t.bytesUp || t.bytesDown ? { ...t, activeConns: 0, bytesUp: 0, bytesDown: 0 } : t;
            return { ...t, bytesUp: x.totalUp, bytesDown: x.totalDown, activeConns: x.active };
          }),
        ),
      ),
    ];
    return () => offs.forEach((f) => f());
  }, [refresh]);

  const reorder = useCallback(
    async (ids: string[]) => {
      setTunnels((list) => ids.map((id) => list.find((t) => t.id === id)!).filter(Boolean));
      try {
        await tunnelApi.reorder(ids);
      } catch (e) {
        toast.error(e);
        refresh();
      }
    },
    [refresh],
  );

  const value = useMemo(() => ({ tunnels, loaded, refresh, reorder }), [tunnels, loaded, refresh, reorder]);
  return <TunnelsContext.Provider value={value}>{children}</TunnelsContext.Provider>;
}

// ---------- 流量 ----------

export const HISTORY_SIZE = 90;

export interface TrafficPoint {
  t: number;
  up: number;
  down: number;
}

interface TrafficCtx {
  current: Traffic | null;
  history: TrafficPoint[];
}

const TrafficContext = createContext<TrafficCtx>(null!);
export const useTraffic = () => useContext(TrafficContext);

function TrafficProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<TrafficCtx>({ current: null, history: [] });
  useEffect(() => {
    Promise.all([trafficApi.current(), trafficApi.history()])
      .then(([c, h]) =>
        setState((s) => {
          const last = h.length ? h[h.length - 1].t : 0;
          return { current: s.current ?? c, history: [...h, ...s.history.filter((p) => p.t > last)].slice(-HISTORY_SIZE) };
        }),
      )
      .catch(() => {});
    return events.onTraffic((tr) =>
      setState((s) => ({
        current: tr,
        history: [...s.history.slice(-(HISTORY_SIZE - 1)), { t: Date.now(), up: tr.up, down: tr.down }],
      })),
    );
  }, []);
  return <TrafficContext.Provider value={state}>{children}</TrafficContext.Provider>;
}

// ---------- 主机指纹确认 ----------

interface HostKeyCtx {
  prompts: HostKeyPrompt[];
  answer: (p: HostKeyPrompt, accept: boolean) => Promise<void>;
}

const HostKeyContext = createContext<HostKeyCtx>(null!);
export const useHostKeys = () => useContext(HostKeyContext);

function HostKeyProvider({ children }: { children: ReactNode }) {
  const toast = useToast();
  const [prompts, setPrompts] = useState<HostKeyPrompt[]>([]);
  useEffect(() => {
    hostKeyApi.pending().then((l) => setPrompts((cur) => merge(cur, l))).catch(() => {});
    return events.onHostKey((p) => setPrompts((cur) => merge(cur, [p])));
  }, []);
  const answer = useCallback(
    async (p: HostKeyPrompt, accept: boolean) => {
      setPrompts((cur) => cur.filter((x) => x.requestId !== p.requestId));
      try {
        await hostKeyApi.answer(p.requestId, accept);
      } catch (e) {
        toast.error(e);
      }
    },
    [],
  );
  const value = useMemo(() => ({ prompts, answer }), [prompts, answer]);
  return <HostKeyContext.Provider value={value}>{children}</HostKeyContext.Provider>;
}

function merge(cur: HostKeyPrompt[], add: HostKeyPrompt[]) {
  const ids = new Set(cur.map((p) => p.requestId));
  return [...cur, ...add.filter((p) => !ids.has(p.requestId))];
}

export function AppProvider({ children }: { children: ReactNode }) {
  return (
    <ToastProvider>
      <SettingsProvider>
        <TunnelsProvider>
          <TrafficProvider>
            <HostKeyProvider>{children}</HostKeyProvider>
          </TrafficProvider>
        </TunnelsProvider>
      </SettingsProvider>
    </ToastProvider>
  );
}
