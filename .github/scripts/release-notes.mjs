// 生成 Release 说明：把 ChangeLog.md 中当前版本的内容和下载链接填进 .github/release-notes.md。
// 用法：node .github/scripts/release-notes.mjs <tag> <owner/repo>，结果输出到 stdout。
import { readFileSync } from "node:fs";

const [tag, repo] = process.argv.slice(2);
if (!tag || !repo) {
  console.error("usage: release-notes.mjs <tag> <owner/repo>");
  process.exit(2);
}
const version = tag.replace(/^v/, "");

// 取 "## v1.2.3" 到下一个二级标题之间的内容。
function changesOf(changelog, version) {
  const lines = changelog.split(/\r?\n/);
  const start = lines.findIndex((l) => new RegExp(`^##\\s+v?${version.replace(/\./g, "\\.")}(\\s|$)`).test(l));
  if (start < 0) return "";
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((l) => /^##\s/.test(l));
  return (end < 0 ? rest : rest.slice(0, end)).join("\n").trim();
}

const changes = changesOf(readFileSync("ChangeLog.md", "utf8"), version);
if (!changes) {
  console.error(`::error::ChangeLog.md 缺少 v${version} 的更新内容，请先补充 "## v${version}" 一节`);
  process.exit(1);
}

const template = readFileSync(".github/release-notes.md", "utf8");
process.stdout.write(
  template
    .replaceAll("{{changes}}", changes)
    .replaceAll("{{version}}", version)
    .replaceAll("{{tag}}", tag)
    .replaceAll("{{repo}}", repo),
);
