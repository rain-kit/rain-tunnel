import { ArrowDownToLine, FolderOpen, Pause, Play, ScrollText, Search, Trash2 } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import TitleBar from "../components/TitleBar";
import { Button, Card, cx, Empty, IconButton, Input, Segmented } from "../components/ui";
import { events, logApi, settingsApi } from "../lib/api";
import { formatTime } from "../lib/format";
import { useI18n } from "../lib/i18n";
import { useToast } from "../lib/store";
import type { LogEntry, LogLevel } from "../lib/types";

const MAX = 2000;
const levelRank: Record<LogLevel, number> = { debug: 0, info: 1, warn: 2, error: 3 };
const levelStyle: Record<LogLevel, string> = {
  debug: "text-muted",
  info: "text-sky-600 dark:text-sky-400",
  warn: "text-amber-600 dark:text-amber-400",
  error: "text-red-600 dark:text-red-400",
};

export default function Logs() {
  const { t } = useI18n();
  const toast = useToast();
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [level, setLevel] = useState<LogLevel>("info");
  const [q, setQ] = useState("");
  const [paused, setPaused] = useState(false);
  const [follow, setFollow] = useState(true);
  const pausedRef = useRef(paused);
  const buffer = useRef<LogEntry[]>([]);
  const scroller = useRef<HTMLDivElement>(null);
  pausedRef.current = paused;

  useEffect(() => {
    logApi.list().then((l) => setLogs((cur) => dedupe([...l, ...cur]))).catch(toast.error);
    return events.onLog((e) => {
      if (pausedRef.current) buffer.current.push(e);
      else setLogs((cur) => [...cur.slice(-(MAX - 1)), e]);
    });
  }, []);

  useEffect(() => {
    if (!paused && buffer.current.length) {
      const add = buffer.current;
      buffer.current = [];
      setLogs((cur) => [...cur, ...add].slice(-MAX));
    }
  }, [paused]);

  const rows = useMemo(() => {
    const s = q.trim().toLowerCase();
    return logs.filter(
      (l) => levelRank[l.level] >= levelRank[level] && (!s || l.message.toLowerCase().includes(s) || l.fields.toLowerCase().includes(s)),
    );
  }, [logs, level, q]);

  useLayoutEffect(() => {
    if (follow && scroller.current) scroller.current.scrollTop = scroller.current.scrollHeight;
  }, [rows, follow]);

  const onScroll = () => {
    const el = scroller.current;
    if (el) setFollow(el.scrollHeight - el.scrollTop - el.clientHeight < 24);
  };

  const clear = async () => {
    try {
      await logApi.clear();
      setLogs([]);
    } catch (e) {
      toast.error(e);
    }
  };

  return (
    <>
      <TitleBar
        title={t("nav.logs")}
        actions={
          <>
            <Segmented
              value={level}
              onChange={setLevel}
              options={[
                { value: "debug", label: "All" },
                { value: "info", label: "Info" },
                { value: "warn", label: "Warn" },
                { value: "error", label: "Error" },
              ]}
            />
            <div className="w-48">
              <Input
                placeholder={t("common.search")}
                value={q}
                onChange={(e) => setQ(e.target.value)}
                suffix={<Search size={14} className="mr-1.5 text-muted" />}
              />
            </div>
            <Button icon={paused ? <Play size={13} /> : <Pause size={13} />} onClick={() => setPaused((v) => !v)}>
              {t(paused ? "common.resume" : "common.pause")}
            </Button>
            <IconButton label={t("logs.openDir")} onClick={() => settingsApi.openLogDir().catch(toast.error)}>
              <FolderOpen size={16} />
            </IconButton>
            <IconButton label={t("logs.clear")} tone="danger" onClick={clear}>
              <Trash2 size={16} />
            </IconButton>
          </>
        }
      />
      <div className="relative min-h-0 flex-1 px-6 pb-6">
        <Card className="h-full overflow-hidden">
          {rows.length === 0 ? (
            <Empty icon={<ScrollText size={22} />} title={t(logs.length ? "logs.noMatch" : "logs.empty")} desc={t("logs.emptyDesc")} />
          ) : (
            <div ref={scroller} onScroll={onScroll} className="selectable h-full overflow-y-auto py-1.5 font-mono text-xs leading-relaxed">
              {rows.map((l) => (
                <div key={l.seq} className="flex gap-3 px-4 py-0.5 hover:bg-subtle/60">
                  <span className="shrink-0 text-muted tabular-nums">{formatTime(l.time, true)}</span>
                  <span className={cx("w-10 shrink-0 font-semibold uppercase", levelStyle[l.level])}>{l.level}</span>
                  <span className="min-w-0 break-words">
                    {l.message}
                    {l.fields && <span className="ml-2 text-muted">{l.fields}</span>}
                  </span>
                </div>
              ))}
            </div>
          )}
        </Card>
        {!follow && rows.length > 0 && (
          <Button
            size="sm"
            className="absolute right-10 bottom-10 shadow-lg"
            icon={<ArrowDownToLine size={13} />}
            onClick={() => setFollow(true)}
          >
            {t("logs.toBottom")}
          </Button>
        )}
      </div>
    </>
  );
}

function dedupe(l: LogEntry[]) {
  const seen = new Set<number>();
  return l.filter((e) => !seen.has(e.seq) && seen.add(e.seq)).sort((a, b) => a.seq - b.seq).slice(-MAX);
}
