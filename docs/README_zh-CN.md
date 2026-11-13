![Rain Tunnel](../public/logo.svg)

# Rain Tunnel

基于 Tauri 的轻量级 SSH 隧道管理工具。

Languages: [English](../README.md) · 简体中文 · [繁體中文](./README_zh-TW.md)

[![Release](https://img.shields.io/github/v/release/rain-kit/rain-tunnel?style=flat-square)](https://github.com/rain-kit/rain-tunnel/releases)
[![Downloads](https://img.shields.io/github/downloads/rain-kit/rain-tunnel/total?style=flat-square)](https://github.com/rain-kit/rain-tunnel/releases)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue?style=flat-square)

## 安装

请到[发布页面](https://github.com/rain-kit/rain-tunnel/releases)下载对应平台的安装包。

| 平台 | 架构 | 文件 |
| :--- | :--- | :--- |
| Windows 10 / 11 | x64 | `Rain.Tunnel_x.y.z_x64-setup.exe` |
| Windows 11 | ARM64 | `Rain.Tunnel_x.y.z_arm64-setup.exe` |
| macOS | Apple Silicon（M 系列芯片） | `Rain.Tunnel_x.y.z_aarch64.dmg` |
| macOS | Intel | `Rain.Tunnel_x.y.z_x64.dmg` |
| Debian / Ubuntu | x64 | `Rain.Tunnel_x.y.z_amd64.deb` |
| Debian / Ubuntu | ARM64 | `Rain.Tunnel_x.y.z_arm64.deb` |
| RHEL / Fedora / CentOS | x64 | `Rain.Tunnel-x.y.z-1.x86_64.rpm` |
| RHEL / Fedora / CentOS | ARM64 | `Rain.Tunnel-x.y.z-1.aarch64.rpm` |

Linux 上用 `sudo apt install ./<文件名>.deb` 或 `sudo dnf install ./<文件名>.rpm` 安装。

> [!NOTE]
> 安装包目前还没有代码签名。
>
> - **Windows**：如果 SmartScreen 拦截了安装程序，点击"更多信息"，再点"仍要运行"。
> - **macOS**：如果提示"已损坏，无法打开"，在终端执行一次：
>
>   ```shell
>   xattr -cr "/Applications/Rain Tunnel.app"
>   ```

## 功能

- **三种转发模式**
  - 本地转发（`ssh -L`）：把服务器能访问到的端口映射到本机。
  - 远程转发（`ssh -R`）：把本机能访问到的端口暴露到服务器上。
  - 动态转发（`ssh -D`）：在本机开启 SOCKS5 代理，可在同一端口同时提供 HTTP 代理，支持用户名密码认证。
- **多种认证方式**：密码、私钥（支持私钥密码）、系统 SSH Agent。
- **默认安全**
  - 首次连接时校验服务器指纹并保存到 `known_hosts`，指纹变化时拒绝连接。
  - 密码保存在系统凭据库（Windows 凭据管理器 / macOS 钥匙串 / Linux Secret Service），不写入配置文件。
  - 代理监听在非本机地址时，必须设置认证。
- **稳定连接**：断线自动重连、心跳保活、空闲超时、单条隧道最大连接数限制。
- **实时监控**：实时流量图、每条隧道的流量统计、活动连接列表，可逐条断开。
- **日志**：内置日志查看器，支持按级别筛选。
- **后台常驻**：系统托盘、开机自启、静默启动、隧道随程序启动。
- **个性化**：浅色 / 深色 / 跟随系统主题；支持英文、简体中文、繁体中文。
- **轻量**：基于 Tauri 2 和 Rust 后端，安装包小，内存占用低。

## 数据

所有数据只保存在本机。数据目录：

- Windows：`%AppData%\ssh-tunnel`
- macOS：`~/Library/Application Support/ssh-tunnel`
- Linux：`~/.config/ssh-tunnel`

| 文件 | 内容 |
| :--- | :--- |
| `tunnels.json` | 隧道配置（不含密码） |
| `config.yaml` | 程序设置 |
| `known_hosts` | 已信任的服务器指纹 |
| `logs/` | 日志文件 |

设置环境变量 `SSH_TUNNEL_DATA_DIR` 可以使用其他数据目录，例如便携使用。

## 开发

准备环境：

- [Node.js](https://nodejs.org/) 20.19+ 或 22.12+
- [Rust](https://www.rust-lang.org/tools/install) 1.85+（stable）
- 对应平台的 [Tauri 依赖](https://tauri.app/start/prerequisites/)

```shell
# 安装依赖
npm install

# 以开发模式启动
npm run tauri dev

# 为当前平台构建安装包
npm run tauri build

# 运行后端测试
cd src-tauri
cargo test
```

### 发布

推送以 `v` 开头的 tag 会触发 [GitHub Actions](../.github/workflows/release.yml)，自动构建 Windows、macOS、Linux 的 x64 和 ARM64 安装包并发布到 Releases。推送到 `main` 时只编译，用于检查能否编译通过，并为下次发布预热缓存。

1. 修改 `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 中的版本号。
2. 提交后，创建并推送对应的 tag：

   ```shell
   git tag v1.0.0
   git push origin main --tags
   ```

如果 tag 与 `tauri.conf.json` 中的版本号不一致，工作流会直接失败。

## 贡献

欢迎提交 Issue 和 Pull Request！

## 致谢

Rain Tunnel 基于以下项目构建：

- [tauri-apps/tauri](https://github.com/tauri-apps/tauri)：使用 Web 前端构建更小、更快、更安全的桌面应用。
- [Eugeny/russh](https://github.com/Eugeny/russh)：Rust 实现的 SSH 客户端与服务端库。
- [tokio-rs/tokio](https://github.com/tokio-rs/tokio)：Rust 异步运行时。
- [facebook/react](https://github.com/facebook/react)：用于构建 Web 和原生界面的库。
- [vitejs/vite](https://github.com/vitejs/vite)：下一代前端构建工具。
- [tailwindlabs/tailwindcss](https://github.com/tailwindlabs/tailwindcss)：实用优先的 CSS 框架。
- [lucide-icons/lucide](https://github.com/lucide-icons/lucide)：美观一致的图标库。
