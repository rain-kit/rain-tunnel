import { Fragment, type ReactNode } from "react";

/** 更新说明用的 Markdown 子集：标题、列表、段落、行内代码和粗体；链接只显示文字，避免在应用窗口里跳转。 */
export default function Markdown({ text }: { text: string }) {
  const blocks: ReactNode[] = [];
  let list: { ordered: boolean; items: ReactNode[] } | null = null;
  let para: string[] = [];

  const flushPara = () => {
    if (para.length) blocks.push(<p key={blocks.length}>{inline(para.join(" "))}</p>);
    para = [];
  };
  const flushList = () => {
    if (list) {
      const List = list.ordered ? "ol" : "ul";
      blocks.push(
        <List key={blocks.length} className={list.ordered ? "list-decimal space-y-1 pl-5" : "list-disc space-y-1 pl-5"}>
          {list.items}
        </List>,
      );
    }
    list = null;
  };

  for (const raw of text.split("\n")) {
    const line = raw.trimEnd();
    let m: RegExpExecArray | null;
    if (!line.trim()) {
      flushPara();
      flushList();
    } else if ((m = /^#{1,6}\s+(.*)$/.exec(line))) {
      flushPara();
      flushList();
      blocks.push(
        <div key={blocks.length} className="pt-1 font-semibold">
          {inline(m[1])}
        </div>,
      );
    } else if ((m = /^(\s*)([-*+]|\d+[.)])\s+(.*)$/.exec(line))) {
      flushPara();
      const ordered = /\d/.test(m[2]);
      if (!list || list.ordered !== ordered) {
        flushList();
        list = { ordered, items: [] };
      }
      list.items.push(
        <li key={list.items.length} className={m[1].length >= 2 ? "ml-4" : undefined}>
          {inline(m[3])}
        </li>,
      );
    } else {
      flushList();
      para.push(line.trim());
    }
  }
  flushPara();
  flushList();
  return <div className="selectable space-y-2 text-sm leading-relaxed">{blocks}</div>;
}

function inline(s: string): ReactNode[] {
  return s.split(/(`[^`]+`|\*\*[^*]+\*\*|\[[^\]]+\]\([^)]+\))/g).map((part, i) => {
    if (part.length > 2 && part.startsWith("`") && part.endsWith("`")) {
      return (
        <code key={i} className="rounded bg-subtle px-1 font-mono text-[0.85em]">
          {part.slice(1, -1)}
        </code>
      );
    }
    if (part.length > 4 && part.startsWith("**") && part.endsWith("**")) return <strong key={i}>{part.slice(2, -2)}</strong>;
    const link = /^\[([^\]]+)\]\([^)]+\)$/.exec(part);
    return <Fragment key={i}>{link ? link[1] : part}</Fragment>;
  });
}
