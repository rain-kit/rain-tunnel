import { ArrowDown, ArrowUp } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { formatBytes } from "../lib/format";
import { useSettings } from "../lib/store";
import type { DayUsage } from "../lib/usage";
import { COLORS, drawSeries, niceMax } from "./TrafficChart";

const PAD_TOP = 18;
const PAD_BOTTOM = 18;
/** 日期标签的最小间距，放不下时隔几天显示一个。 */
const LABEL_GAP = 44;

/** 每日流量折线图，上传、下载各一条，高度随容器变化。 */
export default function UsageChart({ days }: { days: DayUsage[] }) {
  const { dark } = useSettings();
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [hover, setHover] = useState<number | null>(null);

  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) =>
      setSize({ w: Math.floor(entry.contentRect.width), h: Math.floor(entry.contentRect.height) }),
    );
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const n = days.length;
  const slot = n > 0 ? size.w / n : 0;

  useEffect(() => {
    const canvas = canvasRef.current;
    const { w, h } = size;
    if (!canvas || w === 0 || h === 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);

    const plotH = Math.max(0, h - PAD_TOP - PAD_BOTTOM);
    const base = PAD_TOP + plotH;
    const max = niceMax(days.reduce((m, d) => Math.max(m, d.up, d.down), 0));
    const textColor = dark ? "#8b93a1" : "#6b7280";
    ctx.font = `11px ${getComputedStyle(document.body).fontFamily}`;

    ctx.strokeStyle = dark ? "#2a2f39" : "#e5e7ec";
    ctx.lineWidth = 1;
    ctx.setLineDash([3, 4]);
    for (let i = 0; i <= 3; i++) {
      const y = Math.round(PAD_TOP + (plotH * i) / 3) + 0.5;
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(w, y);
      ctx.stroke();
    }
    ctx.setLineDash([]);
    ctx.fillStyle = textColor;
    ctx.textBaseline = "bottom";
    ctx.fillText(formatBytes(max), 2, PAD_TOP - 4);

    const hOf = (v: number) => (Math.min(v, max) / max) * plotH;
    const xOf = (i: number) => (i + 0.5) * slot;
    if (hover !== null && hover < n) {
      ctx.strokeStyle = dark ? "#3a404c" : "#d4d8df";
      ctx.beginPath();
      ctx.moveTo(Math.round(xOf(hover)) + 0.5, PAD_TOP);
      ctx.lineTo(Math.round(xOf(hover)) + 0.5, base);
      ctx.stroke();
    }
    for (const key of ["up", "down"] as const) {
      drawSeries(ctx, days.map((d, i) => [xOf(i), base - hOf(d[key])] as const), COLORS[key], PAD_TOP, base);
      if (hover !== null && hover < n) {
        ctx.fillStyle = COLORS[key];
        ctx.beginPath();
        ctx.arc(xOf(hover), base - hOf(days[hover][key]), 3, 0, Math.PI * 2);
        ctx.fill();
      }
    }

    // 从今天往前每隔 step 天标一个日期。
    const step = Math.max(1, Math.ceil(LABEL_GAP / slot));
    ctx.fillStyle = textColor;
    ctx.textBaseline = "top";
    ctx.textAlign = "center";
    for (let i = n - 1; i >= 0; i -= step) {
      const label = days[i].date.slice(5);
      const half = ctx.measureText(label).width / 2;
      const x = Math.min(Math.max((i + 0.5) * slot, half), w - half);
      ctx.fillText(label, x, base + 4);
    }
    ctx.textAlign = "left";
  }, [days, size, hover, dark, n, slot]);

  const hovered = hover !== null && hover < n ? days[hover] : null;
  const hoverX = hover !== null ? (hover + 0.5) * slot : 0;

  return (
    <div
      ref={wrapRef}
      className="relative h-full w-full"
      onMouseMove={(e) => {
        if (slot <= 0) return;
        const x = e.clientX - e.currentTarget.getBoundingClientRect().left;
        setHover(Math.min(n - 1, Math.max(0, Math.floor(x / slot))));
      }}
      onMouseLeave={() => setHover(null)}
    >
      <canvas ref={canvasRef} className="absolute inset-0 block size-full" />
      {hovered && (
        <div
          className="pointer-events-none absolute top-0 z-10 rounded-lg border border-line bg-surface px-2.5 py-1.5 text-xs whitespace-nowrap shadow-lg tabular-nums"
          style={hoverX > size.w / 2 ? { right: size.w - hoverX + 12 } : { left: hoverX + 12 }}
        >
          <div className="mb-0.5 font-medium">{hovered.date}</div>
          <div className="flex items-center gap-1 text-muted">
            <ArrowUp size={11} className="text-amber-500" />
            {formatBytes(hovered.up)}
          </div>
          <div className="flex items-center gap-1 text-muted">
            <ArrowDown size={11} className="text-emerald-500" />
            {formatBytes(hovered.down)}
          </div>
        </div>
      )}
    </div>
  );
}
