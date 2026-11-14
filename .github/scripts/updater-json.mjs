// 所有平台构建完成后生成应用内更新用的 latest.json，上传到 Release，然后发布草稿。
// 各平台构建同时改写 latest.json 会互相覆盖，所以统一在这里生成一次。
// 用法：GITHUB_TOKEN=... node .github/scripts/updater-json.mjs <tag> <owner/repo>
const [tag, repo] = process.argv.slice(2);
const token = process.env.GITHUB_TOKEN;
if (!tag || !repo || !token) {
  console.error("usage: GITHUB_TOKEN=... updater-json.mjs <tag> <owner/repo>");
  process.exit(2);
}

/** 缺任何一个，用户在对应平台就只能去下载页安装。 */
const REQUIRED = ["windows-x86_64", "windows-aarch64", "darwin-aarch64", "darwin-x86_64", "linux-x86_64", "linux-aarch64"];

// 文件名规则见 release.yml 的 assetNamePattern：[name]_<os>_[version]_[arch][setup][ext]
const KINDS = [
  { ext: "-setup.exe", os: "windows", installer: "nsis" },
  { ext: ".app.tar.gz", os: "darwin", installer: "app" },
  { ext: ".deb", os: "linux", installer: "deb" },
  { ext: ".rpm", os: "linux", installer: "rpm" },
];
/** 没有安装方式后缀的 <os>-<arch> 用哪种包。 */
const DEFAULT_INSTALLER = { windows: "nsis", darwin: "app", linux: "deb" };
const ARCH = { x64: "x86_64", amd64: "x86_64", x86_64: "x86_64", arm64: "aarch64", aarch64: "aarch64" };

async function api(path, init = {}) {
  const res = await fetch(path.startsWith("https://") ? path : `https://api.github.com${path}`, {
    ...init,
    headers: { Authorization: `Bearer ${token}`, "X-GitHub-Api-Version": "2022-11-28", ...init.headers },
  });
  if (!res.ok) throw new Error(`${init.method ?? "GET"} ${path}: ${res.status} ${await res.text()}`);
  return res;
}

// 草稿查不到 /releases/tags/<tag>，只能从列表里找。
const releases = await (await api(`/repos/${repo}/releases?per_page=30`)).json();
const release = releases.find((r) => r.tag_name === tag);
if (!release) {
  console.error(`::error::找不到 ${tag} 的 Release`);
  process.exit(1);
}

const platforms = {};
for (const sig of release.assets.filter((a) => a.name.endsWith(".sig"))) {
  const name = sig.name.slice(0, -".sig".length);
  const kind = KINDS.find((k) => name.endsWith(k.ext));
  const m = kind && name.slice(0, -kind.ext.length).match(/_(windows|macos|linux)_[^_]+_(.+)$/);
  const arch = m && ARCH[m[2]];
  if (!arch) {
    console.log(`跳过无法识别的签名文件 ${sig.name}`);
    continue;
  }
  const signature = await (await api(sig.url, { headers: { Accept: "application/octet-stream" } })).text();
  const entry = { signature: signature.trim(), url: `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(name)}` };
  platforms[`${kind.os}-${arch}-${kind.installer}`] = entry;
  if (DEFAULT_INSTALLER[kind.os] === kind.installer) platforms[`${kind.os}-${arch}`] = entry;
}

const missing = REQUIRED.filter((p) => !platforms[p]);
if (missing.length) {
  console.error(`::error::缺少这些平台的更新包或签名：${missing.join(", ")}`);
  process.exit(1);
}

const json = JSON.stringify(
  { version: tag.replace(/^v/, ""), notes: release.body ?? "", pub_date: new Date().toISOString(), platforms },
  null,
  2,
);
for (const old of release.assets.filter((a) => a.name === "latest.json")) {
  await api(`/repos/${repo}/releases/assets/${old.id}`, { method: "DELETE" });
}
await api(release.upload_url.replace(/\{.*\}$/, "?name=latest.json"), {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: json,
});
console.log(`已上传 latest.json：${Object.keys(platforms).sort().join(", ")}`);

if (release.draft) {
  await api(`/repos/${repo}/releases/${release.id}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ draft: false }),
  });
  console.log(`已发布 ${tag}`);
}
