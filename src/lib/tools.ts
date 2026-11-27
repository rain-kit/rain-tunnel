// 工具页使用的纯函数，不依赖界面。

export const toHex = (bytes: Uint8Array) => Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");

// ---------- 编解码 ----------

export function base64Encode(text: string): string {
  let bin = "";
  for (const b of new TextEncoder().encode(text)) bin += String.fromCharCode(b);
  return btoa(bin);
}

/** 同时接受 URL 安全字符和省略的填充，内容必须是合法的 UTF-8。 */
export function base64Decode(text: string): string {
  let s = text.replace(/\s/g, "").replace(/-/g, "+").replace(/_/g, "/");
  s += "=".repeat((4 - (s.length % 4)) % 4);
  const bytes = Uint8Array.from(atob(s), (c) => c.charCodeAt(0));
  return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
}

export function unicodeEscape(text: string): string {
  let out = "";
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    out += c > 0x7e ? `\\u${c.toString(16).padStart(4, "0")}` : text[i];
  }
  return out;
}

export function unicodeUnescape(text: string): string {
  return text.replace(/\\u\{([0-9a-fA-F]{1,6})\}|\\u([0-9a-fA-F]{4})/g, (_, cp: string | undefined, unit: string | undefined) =>
    cp ? String.fromCodePoint(parseInt(cp, 16)) : String.fromCharCode(parseInt(unit!, 16)),
  );
}

export const jsonFormat = (text: string) => JSON.stringify(JSON.parse(text), null, 2);
export const jsonMinify = (text: string) => JSON.stringify(JSON.parse(text));
export const jsonEscape = (text: string) => JSON.stringify(text).slice(1, -1);

/** 输入可以带或不带两端的引号。 */
export function jsonUnescape(text: string): string {
  const s = text.trim();
  return JSON.parse(s.startsWith('"') && s.endsWith('"') && s.length > 1 ? s : `"${s}"`);
}

export interface Jwt {
  header: unknown;
  payload: Record<string, unknown>;
}

export function decodeJwt(token: string): Jwt {
  const parts = token.trim().replace(/^Bearer\s+/i, "").split(".");
  if (parts.length < 2) throw new Error("jwt");
  const payload = JSON.parse(base64Decode(parts[1]));
  if (typeof payload !== "object" || payload === null) throw new Error("jwt");
  return { header: JSON.parse(base64Decode(parts[0])), payload };
}

// ---------- 哈希 ----------

const S = [7, 12, 17, 22, 5, 9, 14, 20, 4, 11, 16, 23, 6, 10, 15, 21];
const K = Array.from({ length: 64 }, (_, i) => Math.floor(Math.abs(Math.sin(i + 1)) * 2 ** 32));

/** WebCrypto 不提供 MD5，这里按 RFC 1321 实现。 */
export function md5(data: Uint8Array): string {
  const len = data.length;
  const blocks = ((len + 8) >>> 6) + 1;
  const buf = new Uint8Array(blocks * 64);
  buf.set(data);
  buf[len] = 0x80;
  const view = new DataView(buf.buffer);
  view.setUint32(buf.length - 8, (len * 8) >>> 0, true);
  view.setUint32(buf.length - 4, Math.floor(len / 0x20000000), true);

  const h = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476];
  for (let off = 0; off < buf.length; off += 64) {
    let [a, b, c, d] = h;
    for (let i = 0; i < 64; i++) {
      let f: number, g: number;
      if (i < 16) [f, g] = [(b & c) | (~b & d), i];
      else if (i < 32) [f, g] = [(d & b) | (~d & c), (5 * i + 1) % 16];
      else if (i < 48) [f, g] = [b ^ c ^ d, (3 * i + 5) % 16];
      else [f, g] = [c ^ (b | ~d), (7 * i) % 16];
      const x = (a + f + K[i] + view.getUint32(off + g * 4, true)) | 0;
      const s = S[(i >> 4) * 4 + (i % 4)];
      [a, d, c] = [d, c, b];
      b = (b + ((x << s) | (x >>> (32 - s)))) | 0;
    }
    h[0] = (h[0] + a) | 0;
    h[1] = (h[1] + b) | 0;
    h[2] = (h[2] + c) | 0;
    h[3] = (h[3] + d) | 0;
  }
  const out = new DataView(new ArrayBuffer(16));
  h.forEach((v, i) => out.setUint32(i * 4, v, true));
  return toHex(new Uint8Array(out.buffer));
}

export const hashAlgos = ["MD5", "SHA-1", "SHA-256", "SHA-384", "SHA-512"] as const;
export type HashAlgo = (typeof hashAlgos)[number];

export async function digest(algo: HashAlgo, data: Uint8Array<ArrayBuffer>): Promise<string> {
  if (algo === "MD5") return md5(data);
  return toHex(new Uint8Array(await crypto.subtle.digest(algo, data)));
}

// ---------- 随机 ----------

/** 拒绝采样，保证 0..n-1 均匀分布。 */
function randomIndex(n: number): number {
  const limit = Math.floor(0x100000000 / n) * n;
  const buf = new Uint32Array(1);
  do crypto.getRandomValues(buf);
  while (buf[0] >= limit);
  return buf[0] % n;
}

export function uuidV4(): string {
  const b = crypto.getRandomValues(new Uint8Array(16));
  b[6] = (b[6] & 0x0f) | 0x40;
  b[8] = (b[8] & 0x3f) | 0x80;
  const h = toHex(b);
  return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
}

export const charsets = {
  upper: "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
  lower: "abcdefghijklmnopqrstuvwxyz",
  digits: "0123456789",
  symbols: "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~",
};
export type Charset = keyof typeof charsets;
const similar = /[0O1lI|]/g;

/** 去掉空白和重复字符，避免重复字符被选中的概率更高。 */
export const uniqueChars = (s: string) => [...new Set(s.replace(/\s/g, ""))].join("");

/** pools 为每一种选中的字符；生成的密码包含每一种，长度小于种类数时不保证。 */
export function randomPassword(length: number, pools: string[], excludeSimilar: boolean): string {
  pools = pools.map((p) => uniqueChars(excludeSimilar ? p.replace(similar, "") : p)).filter(Boolean);
  const all = uniqueChars(pools.join(""));
  if (!all || length < 1) return "";
  for (let attempt = 0; ; attempt++) {
    const pw = Array.from({ length }, () => all[randomIndex(all.length)]).join("");
    if (attempt >= 100 || length < pools.length || pools.every((p) => [...p].some((c) => pw.includes(c)))) return pw;
  }
}

// ---------- 时间 ----------

const pad = (n: number, w = 2) => String(n).padStart(w, "0");

export function formatDateTime(d: Date, ms = false): string {
  const date = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  const time = `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
  return `${date} ${time}${ms ? `.${pad(d.getMilliseconds(), 3)}` : ""}`;
}

/** 10 位左右按秒，13 位左右按毫秒。 */
export function parseTimestamp(text: string): { date: Date; unit: "s" | "ms" } | null {
  const s = text.trim();
  if (!/^-?\d+(\.\d+)?$/.test(s)) return null;
  const n = Number(s);
  const unit = Math.abs(n) >= 1e11 ? "ms" : "s";
  const date = new Date(unit === "ms" ? n : n * 1000);
  return Number.isNaN(date.getTime()) ? null : { date, unit };
}

/** 不带时区的 "YYYY-MM-DD HH:mm:ss" 按本地时间解析，其余交给 Date.parse（如 ISO 8601）。 */
export function parseDateTime(text: string): Date | null {
  const s = text.trim();
  const m = /^(\d{4})[-/](\d{1,2})[-/](\d{1,2})(?:[ T](\d{1,2}):(\d{1,2})(?::(\d{1,2})(?:\.(\d{1,3}))?)?)?$/.exec(s);
  const date = m
    ? new Date(+m[1], +m[2] - 1, +m[3], +(m[4] ?? 0), +(m[5] ?? 0), +(m[6] ?? 0), +(m[7] ?? "0").padEnd(3, "0"))
    : new Date(Date.parse(s));
  return Number.isNaN(date.getTime()) ? null : date;
}

// ---------- 进制 ----------

const prefixes: Record<number, RegExp> = { 2: /^0b/, 8: /^0o/, 16: /^0x/ };

export function parseBigInt(text: string, base: number): bigint {
  let s = text.trim().replace(/[\s_,]/g, "").toLowerCase();
  const neg = s.startsWith("-");
  if (neg) s = s.slice(1);
  s = s.replace(prefixes[base] ?? /^$/, "");
  if (!s) throw new Error("radix");
  const b = BigInt(base);
  let v = 0n;
  for (const c of s) {
    const d = parseInt(c, 36);
    if (Number.isNaN(d) || d >= base) throw new Error("radix");
    v = v * b + BigInt(d);
  }
  return neg ? -v : v;
}

// ---------- 子网 ----------

export interface Subnet {
  prefix: number;
  network: string;
  broadcast: string;
  mask: string;
  wildcard: string;
  first: string;
  last: string;
  hosts: number;
}

function ipToInt(ip: string): number {
  const parts = ip.split(".").map(Number);
  if (parts.length !== 4 || parts.some((p) => !Number.isInteger(p) || p > 255)) throw new Error("ip");
  return ((parts[0] << 24) | (parts[1] << 16) | (parts[2] << 8) | parts[3]) >>> 0;
}

const intToIp = (n: number) => [n >>> 24, (n >>> 16) & 255, (n >>> 8) & 255, n & 255].join(".");

/** 支持 "192.168.1.10/24"、"192.168.1.10 255.255.255.0" 和单个地址（按 /32）。 */
export function calcSubnet(text: string): Subnet {
  const m = /^\s*(\d{1,3}(?:\.\d{1,3}){3})\s*(?:\/\s*(\d{1,2})|\s+(\d{1,3}(?:\.\d{1,3}){3}))?\s*$/.exec(text);
  if (!m) throw new Error("subnet");
  const ip = ipToInt(m[1]);
  let prefix = 32;
  if (m[2]) {
    prefix = Number(m[2]);
    if (prefix > 32) throw new Error("subnet");
  } else if (m[3]) {
    const inv = ~ipToInt(m[3]) >>> 0;
    if ((inv & (inv + 1)) !== 0) throw new Error("subnet");
    prefix = Math.clz32(inv);
  }
  const mask = prefix === 0 ? 0 : (0xffffffff << (32 - prefix)) >>> 0;
  const network = (ip & mask) >>> 0;
  const broadcast = (network | ~mask) >>> 0;
  const size = 2 ** (32 - prefix);
  const small = prefix >= 31;
  return {
    prefix,
    network: intToIp(network),
    broadcast: intToIp(broadcast),
    mask: intToIp(mask),
    wildcard: intToIp(~mask >>> 0),
    first: intToIp(small ? network : network + 1),
    last: intToIp(small ? broadcast : broadcast - 1),
    hosts: small ? size : size - 2,
  };
}
