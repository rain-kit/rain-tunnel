import { diffArrays, diffChars } from "diff";

export interface DiffOptions {
  /** 忽略行首尾空白，连续空白视为一个空格 */
  ignoreWhitespace: boolean;
  ignoreCase: boolean;
}

/** 行内片段，changed 表示这一段在另一侧不同。 */
export interface DiffPart {
  text: string;
  changed: boolean;
}

export interface DiffCell {
  /** 行号，从 1 开始 */
  no: number;
  parts: DiffPart[];
}

export type DiffKind = "same" | "change" | "remove" | "add";

export interface DiffRow {
  kind: DiffKind;
  left?: DiffCell;
  right?: DiffCell;
}

export interface DiffResult {
  rows: DiffRow[];
  same: boolean;
  changed: number;
  removed: number;
  added: number;
}

/** 末尾的换行只算行结束，不多出一个空行。 */
export function splitLines(text: string): string[] {
  if (!text) return [];
  const lines = text.split(/\r\n|\r|\n/);
  if (lines[lines.length - 1] === "") lines.pop();
  return lines;
}

function normalize(line: string, o: DiffOptions): string {
  let s = o.ignoreWhitespace ? line.trim().replace(/\s+/g, " ") : line;
  if (o.ignoreCase) s = s.toLowerCase();
  return s;
}

const plain = (text: string): DiffPart[] => [{ text, changed: false }];

/** 一对修改过的行，标出各自不同的字符。 */
function inline(a: string, b: string, o: DiffOptions): [DiffPart[], DiffPart[]] {
  // 字符级对比的耗时随长度平方增长，超长的行整行标出。
  if (a.length + b.length > 4000) return [[{ text: a, changed: true }], [{ text: b, changed: true }]];
  const left: DiffPart[] = [];
  const right: DiffPart[] = [];
  for (const c of diffChars(a, b, { ignoreCase: o.ignoreCase })) {
    if (!c.added) left.push({ text: c.value, changed: c.removed });
    if (!c.removed) right.push({ text: c.value, changed: c.added });
  }
  return [left, right];
}

/** 按行对比，左右相邻的删除和新增配成修改行。 */
export function compareText(a: string, b: string, o: DiffOptions): DiffResult {
  const left = splitLines(a);
  const right = splitLines(b);
  const changes = diffArrays(
    left.map((l) => normalize(l, o)),
    right.map((l) => normalize(l, o)),
  );
  const rows: DiffRow[] = [];
  let i = 0;
  let j = 0;
  let k = 0;
  while (k < changes.length) {
    const c = changes[k];
    if (!c.added && !c.removed) {
      for (let n = 0; n < c.count; n++, i++, j++) {
        rows.push({ kind: "same", left: { no: i + 1, parts: plain(left[i]) }, right: { no: j + 1, parts: plain(right[j]) } });
      }
      k++;
      continue;
    }
    const next = changes[k + 1];
    const removed = c.removed ? c.count : next?.removed ? next.count : 0;
    const added = c.added ? c.count : next?.added ? next.count : 0;
    k += removed && added ? 2 : 1;
    const pairs = Math.min(removed, added);
    for (let n = 0; n < pairs; n++, i++, j++) {
      const [l, r] = inline(left[i], right[j], o);
      rows.push({ kind: "change", left: { no: i + 1, parts: l }, right: { no: j + 1, parts: r } });
    }
    for (let n = pairs; n < removed; n++, i++) rows.push({ kind: "remove", left: { no: i + 1, parts: plain(left[i]) } });
    for (let n = pairs; n < added; n++, j++) rows.push({ kind: "add", right: { no: j + 1, parts: plain(right[j]) } });
  }
  const count = (kind: DiffKind) => rows.filter((r) => r.kind === kind).length;
  const changed = count("change");
  const removed = count("remove");
  const added = count("add");
  return { rows, same: changed + removed + added === 0, changed, removed, added };
}
