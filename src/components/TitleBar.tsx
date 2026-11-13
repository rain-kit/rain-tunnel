import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { isMac, isWindows } from "../lib/format";
import { useI18n } from "../lib/i18n";
import { cx } from "./ui";

function WindowControls() {
  const { t } = useI18n();
  const win = getCurrentWindow();
  const [maximized, setMaximized] = useState(false);
  useEffect(() => {
    win.isMaximized().then(setMaximized);
    const un = win.onResized(() => win.isMaximized().then(setMaximized));
    return () => {
      un.then((f) => f());
    };
  }, [win]);
  const btn = "flex h-full w-11.5 items-center justify-center text-muted transition-colors hover:text-ink";
  return (
    <div className="flex h-full">
      <button className={cx(btn, "hover:bg-subtle")} onClick={() => win.minimize()} title={t("window.minimize")}>
        <Minus size={16} strokeWidth={1.5} />
      </button>
      <button className={cx(btn, "hover:bg-subtle")} onClick={() => win.toggleMaximize()} title={t(maximized ? "window.restore" : "window.maximize")}>
        {maximized ? <Copy size={12} strokeWidth={1.5} className="-scale-x-100" /> : <Square size={12} strokeWidth={1.5} />}
      </button>
      <button className={cx(btn, "hover:bg-red-600 hover:text-white")} onClick={() => win.close()} title={t("common.close")}>
        <X size={17} strokeWidth={1.5} />
      </button>
    </div>
  );
}

/** 页面顶栏，同时作为窗口拖动区域；Windows 无边框窗口在右侧放窗口按钮。 */
export default function TitleBar({ title, actions }: { title: ReactNode; actions?: ReactNode }) {
  return (
    <div data-tauri-drag-region className="flex h-13 shrink-0 items-center gap-2 pl-6">
      <h1 data-tauri-drag-region className="flex-1 truncate text-lg font-semibold">
        {title}
      </h1>
      {actions && <div className="flex items-center gap-2">{actions}</div>}
      {isWindows && isTauri() ? (
        <div className="ml-2 h-9 self-start">
          <WindowControls />
        </div>
      ) : (
        <div className={isMac ? "w-4" : "w-6"} />
      )}
    </div>
  );
}
