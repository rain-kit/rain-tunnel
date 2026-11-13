import { ArrowRight, Download, ExternalLink, LoaderCircle } from "lucide-react";
import { updateApi } from "../lib/api";
import { useI18n } from "../lib/i18n";
import { useToast, useUpdate } from "../lib/store";
import Markdown from "./Markdown";
import UpdateRouteMenu from "./UpdateRouteMenu";
import { Button, Dialog } from "./ui";

/** 更新页面：新版本的更新内容，可直接更新或前往下载页。 */
export default function UpdateDialog() {
  const { t } = useI18n();
  const toast = useToast();
  const { info, installing, progress, install, dialogOpen, setDialogOpen } = useUpdate();
  if (!info?.hasUpdate) return null;
  const percent = installing && progress !== null && progress < 100 ? progress : null;

  return (
    <Dialog
      open={dialogOpen}
      onClose={installing ? undefined : () => setDialogOpen(false)}
      width="max-w-lg"
      title={t("update.title")}
      footer={
        <>
          <Button
            variant={info.canInstall ? undefined : "primary"}
            icon={<ExternalLink size={14} />}
            disabled={installing}
            onClick={() => updateApi.openReleasePage().catch(toast.error)}
          >
            {t("update.download")}
          </Button>
          {info.canInstall && (
            <UpdateRouteMenu onPick={install}>
              {(open) => (
                <Button
                  variant="primary"
                  icon={installing ? <LoaderCircle size={14} className="animate-spin" /> : <Download size={14} />}
                  disabled={installing}
                  onClick={open}
                >
                  {installing ? (percent !== null ? t("update.downloading", { percent }) : t("update.installing")) : t("update.install")}
                </Button>
              )}
            </UpdateRouteMenu>
          )}
        </>
      }
    >
      <div className="flex items-center gap-2 text-sm">
        <span className="font-mono text-muted">v{info.current}</span>
        <ArrowRight size={14} className="text-muted" />
        <span className="font-mono font-semibold text-accent">v{info.latest}</span>
        {info.publishedAt && (
          <span className="ml-auto text-xs text-muted">{t("update.publishedAt", { date: new Date(info.publishedAt).toLocaleDateString() })}</span>
        )}
      </div>

      <div className="mt-3 rounded-xl border border-line bg-subtle/40 px-4 py-3">
        <div className="mb-2 text-xs font-semibold text-muted">{t("update.notes")}</div>
        <div className="max-h-72 overflow-y-auto">
          {info.notes ? <Markdown text={info.notes} /> : <div className="text-sm text-muted">{t("update.noNotes")}</div>}
        </div>
      </div>

      {percent !== null && (
        <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-subtle">
          <div className="h-full rounded-full bg-accent transition-[width]" style={{ width: `${percent}%` }} />
        </div>
      )}
      <p className="mt-3 mb-1 text-xs text-muted">{t(info.canInstall ? "update.installHint" : "update.installUnavailable")}</p>
    </Dialog>
  );
}
