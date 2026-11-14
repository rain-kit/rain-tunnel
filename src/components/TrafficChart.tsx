import { useEffect, useRef, useState } from "react";
import { formatSpeed } from "../lib/format";
import { useI18n } from "../lib/i18n";
import { HISTORY_SIZE, useSettings, type TrafficPoint } from "../lib/store";

// 取一个整齐的纵轴上限，避免刻度跳来跳去。
export function niceMax(v: number): number {
  const min = 1024;
  if (v <= min) return min;
  const exp = Math.pow(2, Math.ceil(Math.log2(v)));
  return exp * 0.75 >= v ? exp * 0.75 : exp;
}

export const COLORS = { up: "#f59e0b", down: "#10b981" };

export function withAlpha(hex: string, a: number) {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${a})`;
}

export default function TrafficChart({ history, height, compact }: { history: TrafficPoint[]; height: number; compact?: boolean }) {
  const { dark } = useSettings();
  const { t, locale } = useI18n();
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [width, setWidth] = useState(0);

  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => setWidth(Math.floor(entry.contentRect.width)));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || width === 0) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);

    const padTop = compact ? 4 : 20;
    const padBottom = compact ? 2 : 6;
    const plotH = height - padTop - padBottom;
    const peak = history.reduce((m, p) => Math.max(m, p.up, p.down), 0);
    const max = niceMax(peak);
    const step = width / (HISTORY_SIZE - 1);
    const offset = HISTORY_SIZE - history.length;
    const xOf = (i: number) => (i + offset) * step;
    const yOf = (v: number) => padTop + plotH - (Math.min(v, max) / max) * plotH;

    if (!compact) {
      ctx.strokeStyle = dark ? "#2a2f39" : "#e5e7ec";
      ctx.lineWidth = 1;
      ctx.setLineDash([3, 4]);
      for (let i = 0; i <= 3; i++) {
        const y = Math.round(padTop + (plotH * i) / 3) + 0.5;
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(width, y);
        ctx.stroke();
      }
      ctx.setLineDash([]);
      ctx.fillStyle = dark ? "#8b93a1" : "#6b7280";
      ctx.font = `11px ${getComputedStyle(document.body).fontFamily}`;
      ctx.textBaseline = "bottom";
      ctx.fillText(formatSpeed(max), 2, padTop - 4);
      ctx.textAlign = "right";
      ctx.fillText(t("chart.seconds", { count: HISTORY_SIZE }), width - 2, padTop - 4);
      ctx.textAlign = "left";
    }

    for (const key of ["up", "down"] as const) {
      if (history.length < 2) continue;
      const color = COLORS[key];
      const pts = history.map((p, i) => [xOf(i), yOf(p[key])] as const);
      const path = new Path2D();
      path.moveTo(pts[0][0], pts[0][1]);
      for (let i = 1; i < pts.length; i++) {
        const [x0, y0] = pts[i - 1];
        const [x1, y1] = pts[i];
        const mx = (x0 + x1) / 2;
        path.bezierCurveTo(mx, y0, mx, y1, x1, y1);
      }
      const area = new Path2D(path);
      area.lineTo(pts[pts.length - 1][0], padTop + plotH);
      area.lineTo(pts[0][0], padTop + plotH);
      area.closePath();
      const grad = ctx.createLinearGradient(0, padTop, 0, padTop + plotH);
      grad.addColorStop(0, withAlpha(color, compact ? 0.28 : 0.22));
      grad.addColorStop(1, withAlpha(color, 0));
      ctx.fillStyle = grad;
      ctx.fill(area);
      ctx.strokeStyle = color;
      ctx.lineWidth = compact ? 1.5 : 2;
      ctx.lineJoin = "round";
      ctx.stroke(path);
    }
  }, [history, width, height, compact, dark, locale]);

  return (
    <div ref={wrapRef} style={{ width: "100%", height }}>
      <canvas ref={canvasRef} style={{ width: "100%", height, display: "block" }} />
    </div>
  );
}
