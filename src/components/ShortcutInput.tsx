import { useEffect, useState } from "react";
import { isMac } from "../lib/format";
import { useI18n } from "../lib/i18n";
import { cx } from "./ui";

const modifierCodes = /^(Control|Shift|Alt|Meta|OS)(Left|Right)?$/;
const macSymbols: Record<string, string> = { Cmd: "⌘", Super: "⌘", Alt: "⌥", Ctrl: "⌃", Shift: "⇧" };

/** "Ctrl+Shift+KeyT" 显示为 "Ctrl + Shift + T"，macOS 上修饰键显示为符号。 */
export function formatShortcut(accel: string): string {
  return accel
    .split("+")
    .map((k) => {
      const key = k.replace(/^Key(?=[A-Z]$)/, "").replace(/^Digit(?=\d$)/, "");
      if (isMac) return macSymbols[key] ?? key;
      return key === "Super" || key === "Cmd" ? "Win" : key;
    })
    .join(isMac ? " " : " + ");
}

/** 把按键事件转成后端可解析的快捷键，按键名同 KeyboardEvent.code；没有修饰键时只接受功能键。 */
function toAccelerator(e: KeyboardEvent): string | null {
  if (modifierCodes.test(e.code)) return null;
  const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && (isMac ? "Cmd" : "Super")].filter(Boolean);
  if (!mods.length && !/^F\d{1,2}$/.test(e.code)) return null;
  return [...mods, e.code].join("+");
}

/** 点击后录制快捷键，Esc 或失去焦点取消。onRecording 用来在录制期间暂停全局快捷键。 */
export default function ShortcutInput({
  value,
  disabled,
  onChange,
  onRecording,
}: {
  value: string;
  disabled?: boolean;
  onChange: (accel: string) => void;
  onRecording?: (recording: boolean) => void;
}) {
  const { t } = useI18n();
  const [recording, setRecording] = useState(false);

  const stop = () => {
    setRecording(false);
    onRecording?.(false);
  };

  useEffect(() => {
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.code === "Escape" && !e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey) return stop();
      const accel = toAccelerator(e);
      if (!accel) return;
      setRecording(false);
      onChange(accel);
    };
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", stop);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("blur", stop);
    };
  }, [recording, onChange]);

  return (
    <button
      type="button"
      disabled={disabled}
      onClick={() => {
        if (recording) return stop();
        setRecording(true);
        onRecording?.(true);
      }}
      onBlur={() => recording && stop()}
      title={t("settings.toolsShortcut.change")}
      className={cx(
        "h-8.5 min-w-36 rounded-lg border bg-surface px-3 text-[13px] transition-colors disabled:opacity-50",
        recording ? "border-accent text-accent ring-3 ring-accent/15" : "border-line font-mono hover:bg-subtle",
      )}
    >
      {recording ? t("settings.toolsShortcut.recording") : formatShortcut(value)}
    </button>
  );
}
