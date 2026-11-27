import { useState } from "react";
import { Card, Field, Input, Select } from "../../components/ui";
import { useI18n } from "../../lib/i18n";
import { parseBigInt } from "../../lib/tools";
import { ErrorText, Pane, ResultRow } from "./common";

const bases = [2, 8, 10, 16, 32, 36];

export default function RadixTool() {
  const { t } = useI18n();
  const [value, setValue] = useState("255");
  const [base, setBase] = useState("10");

  let parsed: bigint | null = null;
  try {
    if (value.trim()) parsed = parseBigInt(value, Number(base));
  } catch {
    parsed = null;
  }
  const invalid = !!value.trim() && parsed === null;

  return (
    <div className="flex flex-col gap-4">
      <div className="flex gap-3">
        <Field label={t("tools.radix.value")} className="flex-1">
          <Input mono autoFocus value={value} invalid={invalid} onChange={(e) => setValue(e.target.value)} />
        </Field>
        <Field label={t("tools.radix.from")} className="w-40">
          <Select
            value={base}
            onChange={setBase}
            options={bases.map((b) => ({ value: String(b), label: t("tools.radix.base", { base: b }) }))}
          />
        </Field>
      </div>
      {invalid && <ErrorText>{t("tools.radix.invalid", { base })}</ErrorText>}
      <Pane title={t("tools.result")}>
        <Card className="px-4">
          {bases.map((b) => (
            <ResultRow key={b} label={t("tools.radix.base", { base: b })} value={parsed === null ? "" : parsed.toString(b)} />
          ))}
        </Card>
      </Pane>
    </div>
  );
}
