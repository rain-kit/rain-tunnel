import { useEffect, useState } from "react";
import { Card, Input } from "../../components/ui";
import { useI18n } from "../../lib/i18n";
import { formatDateTime, parseDateTime, parseTimestamp } from "../../lib/tools";
import { ErrorText, Pane, ResultRow } from "./common";

export default function TimestampTool() {
  const { t } = useI18n();
  const [now, setNow] = useState(Date.now);
  const [ts, setTs] = useState(() => String(Math.floor(Date.now() / 1000)));
  const [date, setDate] = useState(() => formatDateTime(new Date()));

  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);

  const parsedTs = parseTimestamp(ts);
  const parsedDate = parseDateTime(date);

  return (
    <div className="flex flex-col gap-4">
      <Pane title={t("tools.time.now")}>
        <Card className="px-4">
          <ResultRow label={t("tools.time.seconds")} value={String(Math.floor(now / 1000))} />
          <ResultRow label={t("tools.time.millis")} value={String(now)} />
          <ResultRow label={t("tools.time.local")} value={formatDateTime(new Date(now))} />
        </Card>
      </Pane>

      <Pane title={t("tools.time.tsToDate")}>
        <Input mono value={ts} invalid={!!ts.trim() && !parsedTs} placeholder="1700000000" onChange={(e) => setTs(e.target.value)} />
        {ts.trim() && !parsedTs && <ErrorText>{t("tools.time.invalidTs")}</ErrorText>}
        <Card className="px-4">
          <ResultRow
            label={t("tools.time.local")}
            value={parsedTs ? formatDateTime(parsedTs.date, parsedTs.unit === "ms") : ""}
            extra={parsedTs && <span className="text-xs text-muted">{t(parsedTs.unit === "ms" ? "tools.time.asMillis" : "tools.time.asSeconds")}</span>}
          />
          <ResultRow label="UTC (ISO 8601)" value={parsedTs?.date.toISOString() ?? ""} />
        </Card>
      </Pane>

      <Pane title={t("tools.time.dateToTs")}>
        <Input mono value={date} invalid={!!date.trim() && !parsedDate} placeholder="2024-01-01 08:00:00" onChange={(e) => setDate(e.target.value)} />
        {date.trim() && !parsedDate && <ErrorText>{t("tools.time.invalidDate")}</ErrorText>}
        <Card className="px-4">
          <ResultRow label={t("tools.time.seconds")} value={parsedDate ? String(Math.floor(parsedDate.getTime() / 1000)) : ""} />
          <ResultRow label={t("tools.time.millis")} value={parsedDate ? String(parsedDate.getTime()) : ""} />
        </Card>
      </Pane>
    </div>
  );
}
