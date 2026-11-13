import { FolderOpen, Monitor, Moon, Sun } from "lucide-react";
import type { ReactNode } from "react";
import TitleBar from "../components/TitleBar";
import { Button, Card, Segmented, Select, SwitchRow } from "../components/ui";
import { settingsApi } from "../lib/api";
import { useI18n, type Language } from "../lib/i18n";
import { useSettings, useToast } from "../lib/store";
import type { LogLevel, SettingsInput, ThemeMode } from "../lib/types";

function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h2 className="mb-2 px-1 text-xs font-semibold tracking-wide text-muted">{title}</h2>
      <Card className="divide-y divide-line [&>*]:px-4 [&>*]:py-3.5">{children}</Card>
    </section>
  );
}

function Row({ title, desc, children }: { title: string; desc?: ReactNode; children: ReactNode }) {
  return (
    <div className="flex items-center gap-4">
      <div className="min-w-0 flex-1">
        <div className="text-sm">{title}</div>
        {desc && <div className="mt-0.5 truncate text-xs text-muted">{desc}</div>}
      </div>
      {children}
    </div>
  );
}

export default function Settings() {
  const { t } = useI18n();
  const { settings: s, save, setAutoLaunch } = useSettings();
  const toast = useToast();
  if (!s) return <TitleBar title={t("nav.settings")} />;

  const update = (patch: Partial<SettingsInput>) => save(patch).catch(toast.error);

  return (
    <>
      <TitleBar title={t("nav.settings")} />
      <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-8">
        <div className="mx-auto flex max-w-2xl flex-col gap-6">
          <Group title={t("settings.appearance")}>
            <Row title={t("settings.theme")}>
              <Segmented<ThemeMode>
                value={s.theme}
                onChange={(theme) => update({ theme })}
                options={[
                  { value: "system", label: <><Monitor size={14} />{t("settings.theme.system")}</> },
                  { value: "light", label: <><Sun size={14} />{t("settings.theme.light")}</> },
                  { value: "dark", label: <><Moon size={14} />{t("settings.theme.dark")}</> },
                ]}
              />
            </Row>
            <Row title={t("settings.language")}>
              <Select<Language>
                className="w-36"
                value={s.language}
                onChange={(language) => update({ language })}
                options={[
                  { value: "system", label: t("settings.language.system") },
                  { value: "zh-CN", label: "简体中文" },
                  { value: "zh-TW", label: "繁體中文" },
                  { value: "en", label: "English" },
                ]}
              />
            </Row>
          </Group>

          <Group title={t("settings.startup")}>
            <SwitchRow
              title={t("settings.autoLaunch")}
              desc={t("settings.autoLaunchDesc")}
              checked={s.autoLaunch}
              onChange={(v) => setAutoLaunch(v).catch(toast.error)}
            />
            <SwitchRow
              title={t("settings.silentStart")}
              desc={t("settings.silentStartDesc")}
              checked={s.silentStart}
              onChange={(silentStart) => update({ silentStart })}
            />
            <SwitchRow
              title={t("settings.closeToTray")}
              desc={t("settings.closeToTrayDesc")}
              checked={s.closeToTray}
              onChange={(closeToTray) => update({ closeToTray })}
            />
          </Group>

          <Group title={t("settings.logs")}>
            <Row title={t("settings.logLevel")} desc={t("settings.logLevelDesc")}>
              <Select<LogLevel>
                className="w-32"
                value={s.logLevel}
                onChange={(logLevel) => update({ logLevel })}
                options={[
                  { value: "debug", label: "Debug" },
                  { value: "info", label: "Info" },
                  { value: "warn", label: "Warn" },
                  { value: "error", label: "Error" },
                ]}
              />
            </Row>
            <Row title={t("settings.logDir")} desc={<span className="font-mono">{s.logDir}</span>}>
              <Button size="sm" icon={<FolderOpen size={13} />} onClick={() => settingsApi.openLogDir().catch(toast.error)}>
                {t("common.open")}
              </Button>
            </Row>
          </Group>

          <Group title={t("settings.data")}>
            <Row title={t("settings.dataDir")} desc={<span className="font-mono">{s.dataDir}</span>}>
              <Button size="sm" icon={<FolderOpen size={13} />} onClick={() => settingsApi.openDataDir().catch(toast.error)}>
                {t("common.open")}
              </Button>
            </Row>
            <div className="text-xs leading-relaxed text-muted">{t("settings.dataNote")}</div>
          </Group>

          <Group title={t("settings.about")}>
            <Row title="Rain Tunnel" desc={t("settings.aboutDesc")}>
              <span className="font-mono text-xs text-muted">v{s.version}</span>
            </Row>
          </Group>
        </div>
      </div>
    </>
  );
}
