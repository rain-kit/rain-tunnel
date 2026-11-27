import {
  base64Decode,
  base64Encode,
  jsonEscape,
  jsonFormat,
  jsonMinify,
  jsonUnescape,
  unicodeEscape,
  unicodeUnescape,
} from "../../lib/tools";
import { Converter, type Mode } from "./common";

const jsonModes: Mode[] = [
  { value: "format", label: "tools.json.format", run: jsonFormat },
  { value: "minify", label: "tools.json.minify", run: jsonMinify },
  { value: "escape", label: "tools.json.escape", run: jsonEscape },
  { value: "unescape", label: "tools.json.unescape", run: jsonUnescape },
];

const base64Modes: Mode[] = [
  { value: "encode", label: "tools.encode", run: base64Encode },
  { value: "decode", label: "tools.decode", run: base64Decode, error: "tools.base64.invalid" },
];

const urlModes: Mode[] = [
  { value: "encode", label: "tools.encode", run: encodeURIComponent, error: "tools.url.invalidText" },
  { value: "decode", label: "tools.decode", run: decodeURIComponent, error: "tools.url.invalid" },
];

const unicodeModes: Mode[] = [
  { value: "escape", label: "tools.unicode.escape", run: unicodeEscape },
  { value: "unescape", label: "tools.unicode.unescape", run: unicodeUnescape },
];

export const JsonTool = () => <Converter modes={jsonModes} placeholder='{"name": "rain"}' />;
export const Base64Tool = () => <Converter modes={base64Modes} />;
export const UrlTool = () => <Converter modes={urlModes} placeholder="https://example.com/?q=..." />;
export const UnicodeTool = () => <Converter modes={unicodeModes} placeholder="\u4f60\u597d" />;
