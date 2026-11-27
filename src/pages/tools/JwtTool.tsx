import { useMemo, useState } from "react";
import { Card, TextArea } from "../../components/ui";
import { useI18n } from "../../lib/i18n";
import { decodeJwt, formatDateTime } from "../../lib/tools";
import { CopyButton, ErrorText, Pane, ResultRow } from "./common";

const timeClaims = ["exp", "iat", "nbf"] as const;

export default function JwtTool() {
  const { t } = useI18n();
  const [token, setToken] = useState("");
  const jwt = useMemo(() => {
    try {
      return token.trim() ? decodeJwt(token) : null;
    } catch {
      return null;
    }
  }, [token]);
  const header = jwt ? JSON.stringify(jwt.header, null, 2) : "";
  const payload = jwt ? JSON.stringify(jwt.payload, null, 2) : "";
  const claims = timeClaims.filter((c) => typeof jwt?.payload[c] === "number");
  const now = Date.now() / 1000;

  return (
    <div className="flex h-full flex-col gap-3">
      <Pane title="Token">
        <TextArea className="h-24" autoFocus value={token} invalid={!!token.trim() && !jwt} onChange={(e) => setToken(e.target.value)} />
        {token.trim() && !jwt ? (
          <ErrorText>{t("tools.jwt.invalid")}</ErrorText>
        ) : (
          <div className="text-xs text-muted">{t("tools.jwt.hint")}</div>
        )}
      </Pane>
      {claims.length > 0 && (
        <Card className="shrink-0 px-4">
          {claims.map((c) => {
            const sec = jwt!.payload[c] as number;
            const state = c === "exp" ? (sec < now ? "expired" : "valid") : c === "nbf" && sec > now ? "notYet" : null;
            return (
              <ResultRow
                key={c}
                label={`${c} · ${t(`tools.jwt.${c}`)}`}
                value={formatDateTime(new Date(sec * 1000))}
                extra={
                  state && (
                    <span className={state === "valid" ? "text-xs text-emerald-600 dark:text-emerald-400" : "text-xs text-red-600 dark:text-red-400"}>
                      {t(`tools.jwt.${state}`)}
                    </span>
                  )
                }
              />
            );
          })}
        </Card>
      )}
      <div className="grid min-h-48 flex-1 grid-cols-2 gap-3">
        <Pane title="Header" actions={<CopyButton text={header} />}>
          <TextArea className="flex-1" readOnly value={header} />
        </Pane>
        <Pane title="Payload" actions={<CopyButton text={payload} />}>
          <TextArea className="flex-1" readOnly value={payload} />
        </Pane>
      </div>
    </div>
  );
}
