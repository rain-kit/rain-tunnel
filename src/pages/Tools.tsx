import { Asterisk, Binary, Braces, Calculator, Clock, Fingerprint, Hash, KeyRound, Link, Network, Type, type LucideIcon } from "lucide-react";
import { useState, type ComponentType } from "react";
import TitleBar from "../components/TitleBar";
import { Card, cx, Segmented } from "../components/ui";
import { useI18n } from "../lib/i18n";
import { Base64Tool, JsonTool, UnicodeTool, UrlTool } from "./tools/codecs";
import { PasswordTool, UuidTool } from "./tools/generators";
import HashTool from "./tools/HashTool";
import JwtTool from "./tools/JwtTool";
import RadixTool from "./tools/RadixTool";
import SubnetTool from "./tools/SubnetTool";
import TimestampTool from "./tools/TimestampTool";

type Category = "common" | "codec" | "crypto" | "convert" | "network";
const categories: Category[] = ["common", "codec", "crypto", "convert", "network"];

const tools = [
  { id: "json", category: "codec", icon: Braces, component: JsonTool },
  { id: "base64", category: "codec", icon: Binary, component: Base64Tool },
  { id: "url", category: "codec", icon: Link, component: UrlTool },
  { id: "unicode", category: "codec", icon: Type, component: UnicodeTool },
  { id: "jwt", category: "codec", icon: KeyRound, component: JwtTool },
  { id: "hash", category: "crypto", icon: Hash, component: HashTool },
  { id: "uuid", category: "crypto", icon: Fingerprint, component: UuidTool },
  { id: "password", category: "crypto", icon: Asterisk, component: PasswordTool },
  { id: "timestamp", category: "convert", icon: Clock, component: TimestampTool },
  { id: "radix", category: "convert", icon: Calculator, component: RadixTool },
  { id: "subnet", category: "network", icon: Network, component: SubnetTool },
] as const satisfies { id: string; category: Exclude<Category, "common">; icon: LucideIcon; component: ComponentType }[];

type ToolId = (typeof tools)[number]["id"];
/** 常用分类里的工具同时也在各自的分类中。 */
const common: ToolId[] = ["json", "uuid", "password", "timestamp"];
const toolsIn = (c: Category) => (c === "common" ? common.map((id) => tools.find((x) => x.id === id)!) : tools.filter((x) => x.category === c));

const STORAGE_KEY = "tool";
const CATEGORY_KEY = "toolCategory";
/** 每个分类上次打开的工具，切换分类时回到它。 */
const lastInCategory: Partial<Record<Category, ToolId>> = {};

export default function Tools() {
  const { t } = useI18n();
  const [current, setCurrent] = useState<ToolId>(() => {
    const saved = localStorage.getItem(STORAGE_KEY);
    return tools.find((x) => x.id === saved)?.id ?? "json";
  });
  const [category, setCategory] = useState<Category>(() => {
    const saved = localStorage.getItem(CATEGORY_KEY) as Category | null;
    if (saved && categories.includes(saved) && toolsIn(saved).some((x) => x.id === current)) return saved;
    return common.includes(current) ? "common" : tools.find((x) => x.id === current)!.category;
  });
  const select = (id: ToolId, c = category) => {
    setCurrent(id);
    setCategory(c);
    lastInCategory[c] = id;
    localStorage.setItem(STORAGE_KEY, id);
    localStorage.setItem(CATEGORY_KEY, c);
  };
  const selectCategory = (c: Category) => select(lastInCategory[c] ?? toolsIn(c)[0].id, c);
  const Tool = tools.find((x) => x.id === current)!.component;

  return (
    <>
      <TitleBar title={t("nav.tools")} />
      <div className="shrink-0 px-6 pb-3">
        <Segmented
          value={category}
          onChange={selectCategory}
          options={categories.map((c) => ({ value: c, label: t(`tools.category.${c}`) }))}
        />
      </div>
      <div className="flex min-h-0 flex-1 gap-4 px-6 pb-6">
        <Card className="w-44 shrink-0 overflow-y-auto p-1.5">
          {toolsIn(category).map(({ id, icon: Icon }) => (
            <button
              key={id}
              onClick={() => select(id)}
              className={cx(
                "flex h-9 w-full items-center gap-2.5 rounded-lg px-2.5 text-left text-[13px] transition-colors",
                current === id ? "bg-accent/15 font-medium dark:bg-accent/35" : "hover:bg-subtle",
              )}
            >
              <Icon size={15} className="shrink-0 text-muted" />
              <span className="truncate">{t(`tools.name.${id}`)}</span>
            </button>
          ))}
        </Card>
        <Card className="min-w-0 flex-1 overflow-y-auto p-4">
          <Tool key={current} />
        </Card>
      </div>
    </>
  );
}
