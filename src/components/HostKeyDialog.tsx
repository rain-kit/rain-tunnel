import { ShieldQuestion } from "lucide-react";
import { useI18n } from "../lib/i18n";
import { useHostKeys } from "../lib/store";
import { Button, Dialog } from "./ui";

/** 首次连接服务器时确认主机指纹；多个请求依次处理。 */
export default function HostKeyDialog() {
  const { t, tx } = useI18n();
  const { prompts, answer } = useHostKeys();
  const p = prompts[0];
  if (!p) return null;
  return (
    <Dialog
      open
      title={
        <span className="flex items-center gap-2">
          <ShieldQuestion size={18} className="text-amber-500" />
          {t("hostKey.title")}
        </span>
      }
      footer={
        <>
          <Button onClick={() => answer(p, false)}>{t("hostKey.reject")}</Button>
          <Button variant="primary" onClick={() => answer(p, true)}>
            {t("hostKey.trust")}
          </Button>
        </>
      }
    >
      <p className="text-[13px] leading-relaxed text-muted">
        {tx("hostKey.message", {
          name: <span className="font-medium text-ink">{p.tunnelName}</span>,
          host: <span className="font-mono text-ink">{p.host}</span>,
        })}
      </p>
      <div className="mt-3 rounded-lg bg-subtle p-3">
        <div className="text-xs text-muted">{t("hostKey.fingerprint", { type: p.keyType })}</div>
        <div className="selectable mt-1 font-mono text-[13px] break-all">{p.fingerprint}</div>
      </div>
      <p className="mt-3 text-xs text-muted">{t("hostKey.note")}</p>
      {prompts.length > 1 && <p className="mt-2 text-xs text-muted">{t("hostKey.pending", { count: prompts.length - 1 })}</p>}
    </Dialog>
  );
}
