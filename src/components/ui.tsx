import { Check, ChevronDown, Eye, EyeOff, X } from "lucide-react";
import {
  forwardRef,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { useI18n } from "../lib/i18n";

export const cx = (...c: (string | false | null | undefined)[]) => c.filter(Boolean).join(" ");

// ---------- 按钮 ----------

type Variant = "primary" | "secondary" | "ghost" | "danger";

const variants: Record<Variant, string> = {
  primary: "bg-accent text-white hover:bg-accent-hover shadow-sm",
  secondary: "bg-surface border border-line hover:bg-subtle text-ink",
  ghost: "hover:bg-subtle text-ink",
  danger: "bg-red-600 text-white hover:bg-red-700 shadow-sm",
};

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: "sm" | "md";
  icon?: ReactNode;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "secondary", size = "md", icon, className, children, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      className={cx(
        "inline-flex items-center justify-center gap-1.5 rounded-lg font-medium whitespace-nowrap transition-colors",
        "disabled:opacity-50 disabled:pointer-events-none focus-visible:outline-2 focus-visible:outline-accent/60",
        size === "sm" ? "h-7 px-2.5 text-xs" : "h-8.5 px-3.5 text-sm",
        variants[variant],
        className,
      )}
      {...rest}
    >
      {icon}
      {children}
    </button>
  );
});

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  size?: "sm" | "md";
  tone?: "default" | "danger";
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { label, size = "md", tone = "default", className, children, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      title={label}
      aria-label={label}
      className={cx(
        "inline-flex items-center justify-center rounded-lg text-muted transition-colors disabled:opacity-40 disabled:pointer-events-none",
        size === "sm" ? "size-7" : "size-8",
        tone === "danger" ? "hover:bg-red-500/10 hover:text-red-600" : "hover:bg-subtle hover:text-ink",
        className,
      )}
      {...rest}
    >
      {children}
    </button>
  );
});

// ---------- 表单 ----------

export function Field({
  label,
  hint,
  error,
  className,
  children,
}: {
  label?: string;
  hint?: ReactNode;
  error?: string;
  className?: string;
  children: ReactNode;
}) {
  return (
    <label className={cx("flex flex-col gap-1 min-w-0", className)}>
      {label && <span className="text-xs font-medium text-muted">{label}</span>}
      {children}
      {error ? (
        <span className="text-xs text-red-600 dark:text-red-400">{error}</span>
      ) : hint ? (
        <span className="text-xs text-muted">{hint}</span>
      ) : null}
    </label>
  );
}

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  invalid?: boolean;
  mono?: boolean;
  suffix?: ReactNode;
}

const inputBase =
  "h-8.5 w-full rounded-lg border bg-surface px-2.5 text-sm text-ink outline-none transition-colors placeholder:text-muted/60 focus:border-accent focus:ring-3 focus:ring-accent/15 disabled:opacity-60";

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input(
  { invalid, mono, suffix, className, ...rest },
  ref,
) {
  const input = (
    <input
      ref={ref}
      spellCheck={false}
      autoComplete="off"
      className={cx(
        inputBase,
        invalid ? "border-red-500 focus:border-red-500 focus:ring-red-500/15" : "border-line",
        mono && "font-mono text-[13px]",
        suffix ? "pr-9" : "",
        className,
      )}
      {...rest}
    />
  );
  if (!suffix) return input;
  return (
    <div className="relative">
      {input}
      <div className="absolute inset-y-0 right-1 flex items-center">{suffix}</div>
    </div>
  );
});

export function PasswordInput(props: Omit<InputProps, "type" | "suffix">) {
  const { t } = useI18n();
  const [show, setShow] = useState(false);
  return (
    <Input
      {...props}
      type={show ? "text" : "password"}
      suffix={
        <IconButton label={t(show ? "common.hide" : "common.show")} size="sm" type="button" onClick={() => setShow((v) => !v)}>
          {show ? <EyeOff size={15} /> : <Eye size={15} />}
        </IconButton>
      }
    />
  );
}

export function Select<T extends string>({
  value,
  options,
  onChange,
  className,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (v: T) => void;
  className?: string;
}) {
  return (
    <div className={cx("relative", className)}>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value as T)}
        className={cx(inputBase, "border-line appearance-none pr-8")}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <ChevronDown size={15} className="pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 text-muted" />
    </div>
  );
}

export function Switch({
  checked,
  onChange,
  disabled,
  label,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
  label?: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cx(
        "relative inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-colors disabled:opacity-50",
        checked ? "bg-accent" : "bg-zinc-300 dark:bg-zinc-600",
      )}
    >
      <span
        className={cx(
          "inline-block size-4 rounded-full bg-white shadow transition-transform",
          checked ? "translate-x-4.5" : "translate-x-0.5",
        )}
      />
    </button>
  );
}

/** 带标题和说明的开关行。 */
export function SwitchRow({
  title,
  desc,
  checked,
  onChange,
  disabled,
}: {
  title: string;
  desc?: ReactNode;
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <div className="flex items-center gap-4">
      <div className="min-w-0 flex-1">
        <div className="text-sm">{title}</div>
        {desc && <div className="mt-0.5 text-xs text-muted">{desc}</div>}
      </div>
      <Switch checked={checked} onChange={onChange} disabled={disabled} label={title} />
    </div>
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  className,
}: {
  value: T;
  options: { value: T; label: ReactNode }[];
  onChange: (v: T) => void;
  className?: string;
}) {
  return (
    <div className={cx("inline-flex h-8.5 rounded-lg bg-subtle p-0.5", className)}>
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          onClick={() => onChange(o.value)}
          className={cx(
            "flex items-center gap-1.5 rounded-md px-3 text-[13px] transition-colors",
            value === o.value ? "bg-surface text-ink shadow-sm font-medium" : "text-muted hover:text-ink",
          )}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

// ---------- 容器 ----------

export function Card({ className, children }: { className?: string; children: ReactNode }) {
  return <div className={cx("rounded-xl border border-line bg-surface", className)}>{children}</div>;
}

export function Empty({ icon, title, desc, action }: { icon: ReactNode; title: string; desc?: string; action?: ReactNode }) {
  return (
    <div className="flex flex-col items-center justify-center py-16 text-center">
      <div className="mb-3 flex size-12 items-center justify-center rounded-2xl bg-subtle text-muted">{icon}</div>
      <div className="font-medium">{title}</div>
      {desc && <div className="mt-1 max-w-sm text-xs text-muted">{desc}</div>}
      {action && <div className="mt-4">{action}</div>}
    </div>
  );
}

// ---------- 对话框 ----------

export function Dialog({
  open,
  onClose,
  title,
  width = "max-w-md",
  children,
  footer,
}: {
  open: boolean;
  onClose?: () => void;
  title: ReactNode;
  width?: string;
  children: ReactNode;
  footer?: ReactNode;
}) {
  const { t } = useI18n();
  useEffect(() => {
    if (!open || !onClose) return;
    const fn = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", fn);
    return () => window.removeEventListener("keydown", fn);
  }, [open, onClose]);
  if (!open) return null;
  return createPortal(
    <div className="fixed inset-0 z-50 flex items-center justify-center p-6">
      <div className="absolute inset-0 animate-fade bg-black/40 dark:bg-black/60" onMouseDown={onClose} />
      <div
        className={cx(
          "relative flex max-h-full w-full animate-pop flex-col rounded-2xl border border-line bg-surface shadow-2xl",
          width,
        )}
      >
        <div className="flex items-center gap-2 px-5 pt-4 pb-2">
          <div className="flex-1 text-base font-semibold">{title}</div>
          {onClose && (
            <IconButton label={t("common.close")} size="sm" onClick={onClose}>
              <X size={16} />
            </IconButton>
          )}
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-2">{children}</div>
        {footer && <div className="flex justify-end gap-2 px-5 pt-2 pb-4">{footer}</div>}
      </div>
    </div>,
    document.body,
  );
}

export function Confirm({
  open,
  title,
  message,
  confirmText,
  danger,
  onConfirm,
  onClose,
}: {
  open: boolean;
  title: string;
  message: ReactNode;
  confirmText?: string;
  danger?: boolean;
  onConfirm: () => void;
  onClose: () => void;
}) {
  const { t } = useI18n();
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={title}
      width="max-w-sm"
      footer={
        <>
          <Button onClick={onClose}>{t("common.cancel")}</Button>
          <Button
            variant={danger ? "danger" : "primary"}
            onClick={() => {
              onConfirm();
              onClose();
            }}
          >
            {confirmText ?? t("common.confirm")}
          </Button>
        </>
      }
    >
      <div className="text-sm text-muted">{message}</div>
    </Dialog>
  );
}

// ---------- 下拉菜单 ----------

export interface MenuItem {
  label: string;
  icon?: ReactNode;
  danger?: boolean;
  checked?: boolean;
  disabled?: boolean;
  onClick: () => void;
}

export function Menu({ trigger, items }: { trigger: (open: () => void) => ReactNode; items: MenuItem[] }) {
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLSpanElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ top: 0, left: 0 });

  useLayoutEffect(() => {
    if (!open || !anchor.current || !menu.current) return;
    const a = anchor.current.getBoundingClientRect();
    const m = menu.current.getBoundingClientRect();
    let top = a.bottom + 4;
    if (top + m.height > window.innerHeight - 8) top = a.top - m.height - 4;
    const left = Math.max(8, Math.min(a.right - m.width, window.innerWidth - m.width - 8));
    setPos({ top, left });
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const close = (e: Event) => {
      if (e instanceof KeyboardEvent && e.key !== "Escape") return;
      if (e.type === "mousedown" && menu.current?.contains(e.target as Node)) return;
      setOpen(false);
    };
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", close);
    window.addEventListener("blur", close);
    window.addEventListener("resize", close);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", close);
      window.removeEventListener("blur", close);
      window.removeEventListener("resize", close);
    };
  }, [open]);

  return (
    <>
      <span ref={anchor} className="inline-flex">
        {trigger(() => setOpen((v) => !v))}
      </span>
      {open &&
        createPortal(
          <div
            ref={menu}
            style={pos}
            className="fixed z-50 min-w-40 animate-pop rounded-xl border border-line bg-surface p-1 shadow-xl"
          >
            {items.map((it) => (
              <button
                key={it.label}
                disabled={it.disabled}
                onClick={() => {
                  setOpen(false);
                  it.onClick();
                }}
                className={cx(
                  "flex w-full items-center gap-2 rounded-lg px-2.5 py-1.5 text-left text-[13px] disabled:opacity-40",
                  it.danger ? "text-red-600 hover:bg-red-500/10 dark:text-red-400" : "hover:bg-subtle",
                )}
              >
                <span className="flex w-4 justify-center text-muted">
                  {it.checked !== undefined ? it.checked && <Check size={14} /> : it.icon}
                </span>
                {it.label}
              </button>
            ))}
          </div>,
          document.body,
        )}
    </>
  );
}
