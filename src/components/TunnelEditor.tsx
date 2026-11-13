import { ArrowRight, ChevronDown, FolderOpen } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { errorMessage, settingsApi, tunnelApi } from "../lib/api";
import { hostPort, typeLabel, typeShort } from "../lib/format";
import { t, useI18n } from "../lib/i18n";
import { useToast } from "../lib/store";
import type { AuthType, SaveTunnelReq, TunnelType, TunnelView } from "../lib/types";
import { typeStyle } from "./badges";
import { Button, cx, Dialog, Field, IconButton, Input, PasswordInput, Segmented, SwitchRow } from "./ui";

interface Form {
  name: string;
  type: TunnelType;
  sshHost: string;
  sshPort: string;
  sshUser: string;
  authType: AuthType;
  keyFile: string;
  password: string;
  keyPassphrase: string;
  listenHost: string;
  listenPort: string;
  targetHost: string;
  targetPort: string;
  enableHttp: boolean;
  proxyUser: string;
  proxyPass: string;
  autoStart: boolean;
  autoReconnect: boolean;
  keepaliveSeconds: string;
  idleTimeoutSeconds: string;
  maxConns: string;
}

type Errors = Partial<Record<keyof Form, string>>;

const DYNAMIC_PORT = "1080";

function toForm(t?: TunnelView | null): Form {
  return {
    name: t?.name ?? "",
    type: t?.type ?? "dynamic",
    sshHost: t?.sshHost ?? "",
    sshPort: String(t?.sshPort ?? 22),
    sshUser: t?.sshUser ?? "",
    authType: t?.authType ?? "password",
    keyFile: t?.keyFile ?? "",
    password: "",
    keyPassphrase: "",
    listenHost: t?.listenHost ?? "127.0.0.1",
    listenPort: t ? String(t.listenPort) : DYNAMIC_PORT,
    targetHost: t?.targetHost ?? "",
    targetPort: t?.targetPort ? String(t.targetPort) : "",
    enableHttp: t ? !!t.enableHttp : true,
    proxyUser: t?.proxyUser ?? "",
    proxyPass: "",
    autoStart: t?.autoStart ?? false,
    autoReconnect: t?.autoReconnect ?? true,
    keepaliveSeconds: String(t?.keepaliveSeconds ?? 15),
    idleTimeoutSeconds: String(t?.idleTimeoutSeconds ?? 300),
    maxConns: String(t?.maxConns ?? 512),
  };
}

const isLoopback = (h: string) => h === "localhost" || h === "::1" || h.startsWith("127.");

function portError(v: string, allowZero: boolean): string | undefined {
  if (v.trim() === "") return t("editor.required");
  const n = Number(v);
  if (!Number.isInteger(n) || n < (allowZero ? 0 : 1) || n > 65535) return allowZero ? "0-65535" : "1-65535";
}

function numError(v: string, max: number): string | undefined {
  const n = Number(v || 0);
  if (!Number.isInteger(n) || n < -1 || n > max) return t("editor.range", { min: -1, max });
}

function validate(f: Form, existing?: TunnelView | null): Errors {
  const e: Errors = {};
  if (!f.sshHost.trim()) e.sshHost = t("editor.err.sshHost");
  if (!f.sshUser.trim()) e.sshUser = t("editor.err.sshUser");
  e.sshPort = portError(f.sshPort, false);
  if (f.authType === "key" && !f.keyFile.trim()) e.keyFile = t("editor.err.keyFile");
  e.listenPort = portError(f.listenPort, f.type !== "remote");
  if (f.type !== "dynamic") {
    if (!f.targetHost.trim()) e.targetHost = t("editor.err.targetHost");
    e.targetPort = portError(f.targetPort, false);
  } else {
    const hasPass = !!f.proxyPass || (!!existing?.hasProxyPass && !!f.proxyUser);
    if (f.proxyUser && !hasPass) e.proxyPass = t("editor.err.proxyPass");
    if (!f.proxyUser && f.proxyPass) e.proxyUser = t("editor.err.proxyUser");
    if (!isLoopback(f.listenHost.trim() || "127.0.0.1") && !f.proxyUser) {
      e.proxyUser = t("editor.err.proxyAuthRequired");
    }
  }
  e.keepaliveSeconds = numError(f.keepaliveSeconds, 3600);
  e.idleTimeoutSeconds = numError(f.idleTimeoutSeconds, 86400 * 7);
  e.maxConns = numError(f.maxConns, 100000);
  (Object.keys(e) as (keyof Form)[]).forEach((k) => e[k] === undefined && delete e[k]);
  return e;
}

function toReq(f: Form, existing?: TunnelView | null): SaveTunnelReq {
  const num = (s: string) => Number(s) || 0;
  const dynamic = f.type === "dynamic";
  return {
    id: existing?.id ?? "",
    name: f.name.trim(),
    type: f.type,
    sshHost: f.sshHost.trim(),
    sshPort: num(f.sshPort),
    sshUser: f.sshUser.trim(),
    authType: f.authType,
    keyFile: f.authType === "key" ? f.keyFile.trim() : "",
    password: f.authType === "password" ? f.password : "",
    keyPassphrase: f.authType === "key" ? f.keyPassphrase : "",
    listenHost: f.listenHost.trim(),
    listenPort: num(f.listenPort),
    targetHost: dynamic ? "" : f.targetHost.trim(),
    targetPort: dynamic ? 0 : num(f.targetPort),
    enableHttp: dynamic && f.enableHttp,
    proxyUser: dynamic ? f.proxyUser.trim() : "",
    proxyPass: dynamic ? f.proxyPass : "",
    clearProxyPass: !dynamic || !f.proxyUser.trim(),
    autoStart: f.autoStart,
    autoReconnect: f.autoReconnect,
    keepaliveSeconds: num(f.keepaliveSeconds),
    idleTimeoutSeconds: num(f.idleTimeoutSeconds),
    maxConns: num(f.maxConns),
  };
}

function TypeOption({ type, selected, onClick }: { type: TunnelType; selected: boolean; onClick: () => void }) {
  const { t } = useI18n();
  return (
    <button
      type="button"
      onClick={onClick}
      className={cx(
        "flex-1 rounded-xl border-[1.5px] p-3 text-left transition-colors",
        selected ? "border-accent bg-accent/5" : "border-line hover:border-accent/40",
      )}
    >
      <div className="flex items-center gap-1.5 text-sm font-semibold">
        <span className={cx("rounded px-1 font-mono text-[11px]", typeStyle[type])}>{typeShort[type]}</span>
        <span className={selected ? "text-accent" : ""}>{typeLabel(type)}</span>
      </div>
      <div className="mt-1 text-xs leading-snug text-muted">{t(`editor.typeDesc.${type}`)}</div>
    </button>
  );
}

function Section({ title, extra, children }: { title: string; extra?: ReactNode; children: ReactNode }) {
  return (
    <section className="mt-5">
      <div className="mb-2.5 flex items-center">
        <h3 className="flex-1 text-xs font-semibold tracking-wide text-muted">{title}</h3>
        {extra}
      </div>
      <div className="flex flex-col gap-3">{children}</div>
    </section>
  );
}

const Row = ({ children }: { children: ReactNode }) => <div className="flex gap-3 [&>*]:flex-1">{children}</div>;
const portCls = "!flex-none w-28";

export default function TunnelEditor({ open, tunnel, onClose }: { open: boolean; tunnel: TunnelView | null; onClose: () => void }) {
  const { t } = useI18n();
  const toast = useToast();
  const [form, setForm] = useState<Form>(() => toForm(tunnel));
  const [errors, setErrors] = useState<Errors>({});
  const [advanced, setAdvanced] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open) {
      setForm(toForm(tunnel));
      setErrors({});
      setAdvanced(false);
    }
  }, [open, tunnel]);

  const set = <K extends keyof Form>(k: K, v: Form[K]) => {
    setForm((f) => ({ ...f, [k]: v }));
    if (errors[k]) setErrors((e) => ({ ...e, [k]: undefined }));
  };

  const setType = (type: TunnelType) =>
    setForm((f) => {
      let listenPort = f.listenPort;
      if (!tunnel) {
        if (type === "dynamic" && !listenPort) listenPort = DYNAMIC_PORT;
        if (type !== "dynamic" && listenPort === DYNAMIC_PORT) listenPort = "";
      }
      return { ...f, type, listenPort };
    });

  const pickKey = async () => {
    try {
      const path = await settingsApi.pickKeyFile();
      if (path) set("keyFile", path);
    } catch (e) {
      toast.error(e);
    }
  };

  const submit = async (start: boolean) => {
    const errs = validate(form, tunnel);
    setErrors(errs);
    if (Object.keys(errs).length > 0) {
      if (errs.keepaliveSeconds || errs.idleTimeoutSeconds || errs.maxConns) setAdvanced(true);
      return;
    }
    setSaving(true);
    try {
      const saved = await tunnelApi.save(toReq(form, tunnel));
      if (start) {
        try {
          await tunnelApi.start(saved.id);
        } catch (e) {
          toast.error(t("editor.startFailed", { error: errorMessage(e) }));
          onClose();
          return;
        }
      }
      toast.success(t(tunnel ? "common.saved" : start ? "editor.createdStarted" : "editor.created"));
      onClose();
    } catch (e) {
      toast.error(e);
    } finally {
      setSaving(false);
    }
  };

  const f = form;
  const secretHint = (has: boolean | undefined) => (has ? t("editor.secretSaved") : undefined);
  const listen = hostPort(f.listenHost || "127.0.0.1", Number(f.listenPort) || 0);
  const target = hostPort(f.targetHost || t("editor.preview.target"), Number(f.targetPort) || 0);
  const ssh = f.sshHost || t("editor.preview.ssh");
  const preview = {
    local: [t("route.local", { addr: listen }), ssh, target],
    remote: [t("route.server", { addr: listen }), ssh, t("editor.preview.localTarget", { addr: target })],
    dynamic: [`${f.enableHttp ? "SOCKS5/HTTP" : "SOCKS5"} ${listen}`, ssh, t("editor.preview.any")],
  }[f.type];

  return (
    <Dialog
      open={open}
      onClose={saving ? undefined : onClose}
      title={t(tunnel ? "editor.titleEdit" : "editor.titleNew")}
      width="max-w-xl"
      footer={
        <>
          <Button onClick={onClose} disabled={saving}>
            {t("common.cancel")}
          </Button>
          {!tunnel && (
            <Button onClick={() => submit(true)} disabled={saving}>
              {t("editor.saveAndStart")}
            </Button>
          )}
          <Button variant="primary" onClick={() => submit(false)} disabled={saving}>
            {t("common.save")}
          </Button>
        </>
      }
    >
      <div className="flex gap-2.5">
        {(["local", "remote", "dynamic"] as TunnelType[]).map((t) => (
          <TypeOption key={t} type={t} selected={f.type === t} onClick={() => setType(t)} />
        ))}
      </div>

      <div className="mt-3 flex items-center gap-2 overflow-hidden rounded-lg bg-subtle px-3 py-2 font-mono text-xs whitespace-nowrap text-muted [&>span]:truncate">
        <span>{preview[0]}</span>
        <ArrowRight size={13} className="shrink-0" />
        <span>{preview[1]}</span>
        <ArrowRight size={13} className="shrink-0" />
        <span>{preview[2]}</span>
      </div>

      <div className="mt-4">
        <Field label={t("editor.name")}>
          <Input placeholder={t("editor.namePlaceholder")} value={f.name} onChange={(e) => set("name", e.target.value)} />
        </Field>
      </div>

      <Section title={t("editor.section.ssh")}>
        <Row>
          <Field label={t("editor.sshHost")} error={errors.sshHost}>
            <Input placeholder="example.com" value={f.sshHost} invalid={!!errors.sshHost} onChange={(e) => set("sshHost", e.target.value)} />
          </Field>
          <Field label={t("editor.port")} error={errors.sshPort} className={portCls}>
            <Input value={f.sshPort} invalid={!!errors.sshPort} onChange={(e) => set("sshPort", e.target.value)} />
          </Field>
        </Row>
        <Row>
          <Field label={t("editor.username")} error={errors.sshUser}>
            <Input value={f.sshUser} invalid={!!errors.sshUser} onChange={(e) => set("sshUser", e.target.value)} />
          </Field>
          <Field label={t("editor.authType")} className="!flex-none">
            <Segmented
              value={f.authType}
              onChange={(v) => set("authType", v)}
              options={[
                { value: "password", label: t("editor.auth.password") },
                { value: "key", label: t("editor.auth.key") },
                { value: "agent", label: "SSH Agent" },
              ]}
            />
          </Field>
        </Row>
        {f.authType === "password" && (
          <Field label={t("editor.password")} hint={secretHint(tunnel?.hasPassword) ?? t("editor.passwordHint")}>
            <PasswordInput
              value={f.password}
              placeholder={tunnel?.hasPassword ? "••••••••" : undefined}
              onChange={(e) => set("password", e.target.value)}
            />
          </Field>
        )}
        {f.authType === "key" && (
          <>
            <Field label={t("editor.keyFile")} error={errors.keyFile}>
              <Input
                mono
                placeholder="~/.ssh/id_ed25519"
                value={f.keyFile}
                invalid={!!errors.keyFile}
                onChange={(e) => set("keyFile", e.target.value)}
                suffix={
                  <IconButton label={t("editor.pickFile")} size="sm" type="button" onClick={pickKey}>
                    <FolderOpen size={15} />
                  </IconButton>
                }
              />
            </Field>
            <Field label={t("editor.keyPassphrase")} hint={secretHint(tunnel?.hasKeyPassphrase)}>
              <PasswordInput
                value={f.keyPassphrase}
                placeholder={tunnel?.hasKeyPassphrase ? "••••••••" : undefined}
                onChange={(e) => set("keyPassphrase", e.target.value)}
              />
            </Field>
          </>
        )}
        {f.authType === "agent" && (
          <p className="text-xs text-muted">{t("editor.agentHint")}</p>
        )}
      </Section>

      <Section title={t("editor.section.forward")}>
        <Row>
          <Field
            label={t(f.type === "remote" ? "editor.listenHost.remote" : "editor.listenHost.local")}
            hint={t(f.type === "remote" ? "editor.listenHint.remote" : "editor.listenHint.local")}
          >
            <Input value={f.listenHost} onChange={(e) => set("listenHost", e.target.value)} />
          </Field>
          <Field label={t("editor.listenPort")} error={errors.listenPort} className={portCls}>
            <Input value={f.listenPort} invalid={!!errors.listenPort} onChange={(e) => set("listenPort", e.target.value)} />
          </Field>
        </Row>
        {f.type !== "dynamic" && (
          <Row>
            <Field label={t(f.type === "local" ? "editor.targetHost.local" : "editor.targetHost.remote")} error={errors.targetHost}>
              <Input
                placeholder={f.type === "local" ? t("editor.targetPlaceholder.local") : "127.0.0.1"}
                value={f.targetHost}
                invalid={!!errors.targetHost}
                onChange={(e) => set("targetHost", e.target.value)}
              />
            </Field>
            <Field label={t("editor.targetPort")} error={errors.targetPort} className={portCls}>
              <Input value={f.targetPort} invalid={!!errors.targetPort} onChange={(e) => set("targetPort", e.target.value)} />
            </Field>
          </Row>
        )}
        {f.type === "dynamic" && (
          <>
            <SwitchRow
              title={t("editor.enableHttp")}
              desc={t("editor.enableHttpDesc")}
              checked={f.enableHttp}
              onChange={(v) => set("enableHttp", v)}
            />
            <Row>
              <Field label={t("editor.proxyUser")} error={errors.proxyUser}>
                <Input value={f.proxyUser} invalid={!!errors.proxyUser} onChange={(e) => set("proxyUser", e.target.value)} />
              </Field>
              <Field label={t("editor.proxyPass")} error={errors.proxyPass} hint={secretHint(tunnel?.hasProxyPass && !!f.proxyUser)}>
                <PasswordInput
                  value={f.proxyPass}
                  invalid={!!errors.proxyPass}
                  placeholder={tunnel?.hasProxyPass ? "••••••••" : undefined}
                  onChange={(e) => set("proxyPass", e.target.value)}
                />
              </Field>
            </Row>
          </>
        )}
      </Section>

      <Section
        title={t("editor.section.options")}
        extra={
          <button type="button" className="flex items-center gap-0.5 text-xs text-muted hover:text-ink" onClick={() => setAdvanced((v) => !v)}>
            {t("editor.advanced")}
            <ChevronDown size={14} className={cx("transition-transform", advanced && "rotate-180")} />
          </button>
        }
      >
        <SwitchRow title={t("common.autoStart")} checked={f.autoStart} onChange={(v) => set("autoStart", v)} />
        <SwitchRow
          title={t("editor.autoReconnect")}
          desc={t("editor.autoReconnectDesc")}
          checked={f.autoReconnect}
          onChange={(v) => set("autoReconnect", v)}
        />
        {advanced && (
          <Row>
            <Field label={t("editor.keepalive")} error={errors.keepaliveSeconds} hint={t("editor.keepaliveHint")}>
              <Input value={f.keepaliveSeconds} invalid={!!errors.keepaliveSeconds} onChange={(e) => set("keepaliveSeconds", e.target.value)} />
            </Field>
            <Field label={t("editor.idleTimeout")} error={errors.idleTimeoutSeconds} hint={t("editor.unlimitedHint")}>
              <Input value={f.idleTimeoutSeconds} invalid={!!errors.idleTimeoutSeconds} onChange={(e) => set("idleTimeoutSeconds", e.target.value)} />
            </Field>
            <Field label={t("editor.maxConns")} error={errors.maxConns} hint={t("editor.unlimitedHint")}>
              <Input value={f.maxConns} invalid={!!errors.maxConns} onChange={(e) => set("maxConns", e.target.value)} />
            </Field>
          </Row>
        )}
      </Section>
      <div className="h-2" />
    </Dialog>
  );
}
