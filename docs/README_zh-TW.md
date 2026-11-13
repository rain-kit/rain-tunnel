![Rain Tunnel](../public/logo.svg)

# Rain Tunnel

基於 Tauri 的輕量級 SSH 隧道管理工具。

Languages: [English](../README.md) · [简体中文](./README_zh-CN.md) · 繁體中文

[![Release](https://img.shields.io/github/v/release/rain-kit/rain-tunnel?style=flat-square)](https://github.com/rain-kit/rain-tunnel/releases)
[![Downloads](https://img.shields.io/github/downloads/rain-kit/rain-tunnel/total?style=flat-square)](https://github.com/rain-kit/rain-tunnel/releases)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue?style=flat-square)

## 安裝

請到[發布頁面](https://github.com/rain-kit/rain-tunnel/releases)下載對應平台的安裝檔。

| 平台 | 架構 | 檔案 |
| :--- | :--- | :--- |
| Windows 10 / 11 | x64 | `Rain.Tunnel_x.y.z_x64-setup.exe` |
| Windows 11 | ARM64 | `Rain.Tunnel_x.y.z_arm64-setup.exe` |
| macOS | Apple Silicon（M 系列晶片） | `Rain.Tunnel_x.y.z_aarch64.dmg` |
| macOS | Intel | `Rain.Tunnel_x.y.z_x64.dmg` |
| Debian / Ubuntu | x64 | `Rain.Tunnel_x.y.z_amd64.deb` |
| Debian / Ubuntu | ARM64 | `Rain.Tunnel_x.y.z_arm64.deb` |
| RHEL / Fedora / CentOS | x64 | `Rain.Tunnel-x.y.z-1.x86_64.rpm` |
| RHEL / Fedora / CentOS | ARM64 | `Rain.Tunnel-x.y.z-1.aarch64.rpm` |

Linux 上使用 `sudo apt install ./<檔名>.deb` 或 `sudo dnf install ./<檔名>.rpm` 安裝。

> [!NOTE]
> 安裝檔目前尚未進行程式碼簽署。
>
> - **Windows**：如果 SmartScreen 攔截了安裝程式，點選「其他資訊」，再點「仍要執行」。
> - **macOS**：如果提示「已損毀，無法打開」，在終端機執行一次：
>
>   ```shell
>   xattr -cr "/Applications/Rain Tunnel.app"
>   ```

## 功能

- **三種轉送模式**
  - 本機轉送（`ssh -L`）：把伺服器能存取的連接埠對應到本機。
  - 遠端轉送（`ssh -R`）：把本機能存取的連接埠公開到伺服器上。
  - 動態轉送（`ssh -D`）：在本機開啟 SOCKS5 代理，可在同一連接埠同時提供 HTTP 代理，支援使用者名稱與密碼驗證。
- **多種驗證方式**：密碼、私鑰（支援私鑰密碼）、系統 SSH Agent。
- **預設安全**
  - 首次連線時驗證伺服器指紋並儲存到 `known_hosts`，指紋變更時拒絕連線。
  - 密碼儲存在系統認證資料庫（Windows 認證管理員 / macOS 鑰匙圈 / Linux Secret Service），不寫入設定檔。
  - 代理監聽在非本機位址時，必須設定驗證。
- **穩定連線**：斷線自動重新連線、心跳保活、閒置逾時、單條隧道最大連線數限制。
- **即時監控**：即時流量圖、每條隧道的流量統計、作用中連線清單，可逐條中斷。
- **日誌**：內建日誌檢視器，支援依等級篩選。
- **背景常駐**：系統匣、開機自動啟動、靜默啟動、隧道隨程式啟動。
- **個人化**：淺色 / 深色 / 跟隨系統主題；支援英文、簡體中文、繁體中文。
- **輕量**：基於 Tauri 2 與 Rust 後端，安裝檔小，記憶體占用低。

## 資料

所有資料只儲存在本機。資料資料夾：

- Windows：`%AppData%\ssh-tunnel`
- macOS：`~/Library/Application Support/ssh-tunnel`
- Linux：`~/.config/ssh-tunnel`

| 檔案 | 內容 |
| :--- | :--- |
| `tunnels.json` | 隧道設定（不含密碼） |
| `config.yaml` | 程式設定 |
| `known_hosts` | 已信任的伺服器指紋 |
| `logs/` | 日誌檔案 |

設定環境變數 `SSH_TUNNEL_DATA_DIR` 可以使用其他資料資料夾，例如可攜式使用。

## 開發

準備環境：

- [Node.js](https://nodejs.org/) 20.19+ 或 22.12+
- [Rust](https://www.rust-lang.org/tools/install) 1.85+（stable）
- 對應平台的 [Tauri 相依套件](https://tauri.app/start/prerequisites/)

```shell
# 安裝相依套件
npm install

# 以開發模式啟動
npm run tauri dev

# 為目前平台建置安裝檔
npm run tauri build

# 執行後端測試
cd src-tauri
cargo test
```

### 發布

推送以 `v` 開頭的 tag 會觸發 [GitHub Actions](../.github/workflows/release.yml)，自動建置 Windows、macOS、Linux 的 x64 與 ARM64 安裝檔並發布到 Releases。推送到 `main` 時只編譯，用來檢查能否編譯成功，並為下次發布預熱快取。

1. 修改 `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 中的版本號。
2. 提交後，建立並推送對應的 tag：

   ```shell
   git tag v1.0.0
   git push origin main --tags
   ```

如果 tag 與 `tauri.conf.json` 中的版本號不一致，工作流程會直接失敗。

## 貢獻

歡迎提交 Issue 與 Pull Request！

## 致謝

Rain Tunnel 基於以下專案建置：

- [tauri-apps/tauri](https://github.com/tauri-apps/tauri)：使用 Web 前端建置更小、更快、更安全的桌面應用程式。
- [Eugeny/russh](https://github.com/Eugeny/russh)：以 Rust 實作的 SSH 用戶端與伺服器函式庫。
- [tokio-rs/tokio](https://github.com/tokio-rs/tokio)：Rust 非同步執行環境。
- [facebook/react](https://github.com/facebook/react)：用於建置 Web 與原生介面的函式庫。
- [vitejs/vite](https://github.com/vitejs/vite)：新一代前端建置工具。
- [tailwindlabs/tailwindcss](https://github.com/tailwindlabs/tailwindcss)：實用優先的 CSS 框架。
- [lucide-icons/lucide](https://github.com/lucide-icons/lucide)：美觀一致的圖示庫。
