import { useState } from "react";
import { Card, Input } from "../../components/ui";
import { useI18n } from "../../lib/i18n";
import { calcSubnet, type Subnet } from "../../lib/tools";
import { ErrorText, Pane, ResultRow } from "./common";

export default function SubnetTool() {
  const { t } = useI18n();
  const [value, setValue] = useState("192.168.1.10/24");

  let subnet: Subnet | null = null;
  try {
    if (value.trim()) subnet = calcSubnet(value);
  } catch {
    subnet = null;
  }
  const invalid = !!value.trim() && !subnet;

  return (
    <div className="flex flex-col gap-4">
      <Pane title={t("tools.subnet.input")}>
        <Input mono autoFocus value={value} invalid={invalid} placeholder="10.0.0.1/16" onChange={(e) => setValue(e.target.value)} />
        {invalid ? <ErrorText>{t("tools.subnet.invalid")}</ErrorText> : <div className="text-xs text-muted">{t("tools.subnet.hint")}</div>}
      </Pane>
      <Pane title={t("tools.result")}>
        <Card className="px-4">
          <ResultRow label="CIDR" value={subnet ? `${subnet.network}/${subnet.prefix}` : ""} />
          <ResultRow label={t("tools.subnet.network")} value={subnet?.network ?? ""} />
          <ResultRow label={t("tools.subnet.broadcast")} value={subnet?.broadcast ?? ""} />
          <ResultRow label={t("tools.subnet.mask")} value={subnet?.mask ?? ""} />
          <ResultRow label={t("tools.subnet.wildcard")} value={subnet?.wildcard ?? ""} />
          <ResultRow label={t("tools.subnet.range")} value={subnet ? `${subnet.first} - ${subnet.last}` : ""} />
          <ResultRow label={t("tools.subnet.hosts")} value={subnet ? subnet.hosts.toLocaleString() : ""} />
        </Card>
      </Pane>
    </div>
  );
}
