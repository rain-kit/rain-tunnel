import { CircleAlert, CircleCheck, Info, X } from "lucide-react";
import { useI18n } from "../lib/i18n";
import { useToast } from "../lib/store";
import { cx } from "./ui";

const icons = {
  success: <CircleCheck size={16} className="text-emerald-500" />,
  error: <CircleAlert size={16} className="text-red-500" />,
  info: <Info size={16} className="text-accent" />,
};

export default function Toaster() {
  const { t } = useI18n();
  const { toasts, dismiss } = useToast();
  return (
    <div className="pointer-events-none fixed bottom-4 left-1/2 z-[60] flex -translate-x-1/2 flex-col items-center gap-2">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          className={cx(
            "pointer-events-auto flex max-w-lg animate-pop items-start gap-2 rounded-xl border border-line bg-surface px-3.5 py-2.5 text-[13px] shadow-lg",
          )}
        >
          <span className="mt-0.5">{icons[toast.kind]}</span>
          <span className="selectable flex-1 break-words">{toast.text}</span>
          <button className="mt-0.5 text-muted hover:text-ink" onClick={() => dismiss(toast.id)} aria-label={t("common.close")}>
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
