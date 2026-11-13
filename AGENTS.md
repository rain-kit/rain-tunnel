# AGENTS.md

Rain Tunnel：基于 Tauri 2 的 SSH 隧道管理工具。后端 Rust 在 `src-tauri`，前端 React + TypeScript 在 `src`。

## 更新日志（必须）

每次改动用户能感知到的功能、修复或行为，都要在 [ChangeLog.md](./ChangeLog.md) 记一条，和代码改动一起提交。

- 写进最上面那一节 `## vX.Y.Z`。如果这个版本已经发布（`git tag` 里有 `vX.Y.Z`），就在最上面新建下一个版本的一节，并同步修改版本号（见下文“发布版本”）。
- 标题只能是 `## vX.Y.Z`，不要用 `Unreleased` 之类的写法：发布脚本 `.github/scripts/release-notes.mjs` 按版本号取内容，找不到当前版本的一节时发布会失败。
- 这一节会原样显示在 Release 说明和应用内的更新页面里，写给用户看：
  - 中文，每条以 `- ` 开头，一句话说清楚变化；
  - 写用户能看到的效果，不写内部实现、文件名和重构细节；
  - 同一个功能在一个版本内多次修改时，更新原来那条，不要重复追加。
- 纯重构、测试、CI、文档等用户无感知的改动不用写。

示例：

```markdown
## v1.0.2

- 修复隧道断线重连后流量统计归零的问题。
- 托盘菜单新增“全部停止”。
```

## 发布版本

1. 三处版本号保持一致：`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml`（`Cargo.lock` 随编译更新）。
2. `ChangeLog.md` 中有对应的 `## vX.Y.Z` 一节。
3. 推送 `vX.Y.Z` 标签，标签与 `tauri.conf.json` 的版本不一致时发布会失败。

## 改动后的检查

- 前端：`npx tsc --noEmit`
- 后端：在 `src-tauri` 下运行 `cargo test`
- 界面文案改动要同时更新 `src/locales` 下的 `zh-CN.ts`、`zh-TW.ts`、`en.ts`。
