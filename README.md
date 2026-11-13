![Rain Tunnel](./public/logo.svg)

# Rain Tunnel

A lightweight SSH tunnel manager built with Tauri.

Languages: English · [简体中文](./docs/README_zh-CN.md) · [繁體中文](./docs/README_zh-TW.md)

[![Release](https://img.shields.io/github/v/release/rain-kit/rain-tunnel?style=flat-square)](https://github.com/rain-kit/rain-tunnel/releases)
[![Downloads](https://img.shields.io/github/downloads/rain-kit/rain-tunnel/total?style=flat-square)](https://github.com/rain-kit/rain-tunnel/releases)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-blue?style=flat-square)

## Install

Go to the [Release page](https://github.com/rain-kit/rain-tunnel/releases) to download the installer for your platform.

| Platform | Architecture | File |
| :------- | :----------- | :--- |
| Windows 10 / 11 | x64 | `Rain.Tunnel_x.y.z_x64-setup.exe` |
| Windows 11 | ARM64 | `Rain.Tunnel_x.y.z_arm64-setup.exe` |
| macOS | Apple Silicon (M series) | `Rain.Tunnel_x.y.z_aarch64.dmg` |
| macOS | Intel | `Rain.Tunnel_x.y.z_x64.dmg` |
| Debian / Ubuntu | x64 | `Rain.Tunnel_x.y.z_amd64.deb` |
| Debian / Ubuntu | ARM64 | `Rain.Tunnel_x.y.z_arm64.deb` |
| RHEL / Fedora / CentOS | x64 | `Rain.Tunnel-x.y.z-1.x86_64.rpm` |
| RHEL / Fedora / CentOS | ARM64 | `Rain.Tunnel-x.y.z-1.aarch64.rpm` |

On Linux, install with `sudo apt install ./<file>.deb` or `sudo dnf install ./<file>.rpm`.

> [!NOTE]
> The installers are not code-signed yet.
>
> - **Windows**: if SmartScreen blocks the installer, click "More info", then "Run anyway".
> - **macOS**: if macOS says the app "is damaged and can't be opened", run this once in Terminal:
>
>   ```shell
>   xattr -cr "/Applications/Rain Tunnel.app"
>   ```

## Features

- **Three forwarding modes**
  - Local forwarding (`ssh -L`): map a port reachable from the server to this machine.
  - Remote forwarding (`ssh -R`): expose a port reachable from this machine on the server.
  - Dynamic forwarding (`ssh -D`): run a local SOCKS5 proxy, optionally serving an HTTP proxy on the same port, with optional username / password authentication.
- **Flexible authentication**: password, private key (with passphrase), or the system SSH agent.
- **Safe by default**
  - Server fingerprints are verified on first connection and saved to `known_hosts`. Connections are refused if the fingerprint changes.
  - Passwords are stored in the system credential store (Windows Credential Manager / macOS Keychain / Linux Secret Service), never in config files.
  - A proxy listening on a non-loopback address must have authentication enabled.
- **Reliable connections**: automatic reconnect, keepalive, idle timeout and a per-tunnel connection limit.
- **Live monitoring**: real-time traffic chart, per-tunnel traffic statistics, and a list of active connections that you can close one by one.
- **Logs**: built-in log viewer with level filtering.
- **Runs in the background**: system tray, launch at login, silent start, and tunnels that start with the app.
- **Personalization**: light / dark / system theme; English, Simplified Chinese and Traditional Chinese.
- **Lightweight**: Rust backend on Tauri 2, a small installer and low memory usage.

## Data

All data stays on your own device. The data folder is:

- Windows: `%AppData%\ssh-tunnel`
- macOS: `~/Library/Application Support/ssh-tunnel`
- Linux: `~/.config/ssh-tunnel`

| File | Contents |
| :--- | :------- |
| `tunnels.json` | Tunnel definitions (no passwords) |
| `config.yaml` | App settings |
| `known_hosts` | Trusted server fingerprints |
| `logs/` | Log files |

Set the `SSH_TUNNEL_DATA_DIR` environment variable to use a different data folder, e.g. for portable use.

## Development

Prerequisites:

- [Node.js](https://nodejs.org/) 20.19+ or 22.12+
- [Rust](https://www.rust-lang.org/tools/install) 1.85+ (stable)
- The [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your platform

```shell
# Install dependencies
npm install

# Start the app in development mode
npm run tauri dev

# Build installers for the current platform
npm run tauri build

# Run backend tests
cd src-tauri
cargo test
```

### Release

Pushing a tag that starts with `v` triggers [GitHub Actions](./.github/workflows/release.yml), which builds installers for Windows, macOS and Linux (x64 and ARM64 each) and publishes them to Releases. Pushing to `main` only compiles, which checks the build and warms the cache for the next release.

1. Update the version in `package.json`, `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`.
2. Commit, then create and push a matching tag:

   ```shell
   git tag v1.0.0
   git push origin main --tags
   ```

The workflow fails if the tag does not match the version in `tauri.conf.json`.

## Contributions

Issues and pull requests are welcome!

## Acknowledgement

Rain Tunnel is built on these projects:

- [tauri-apps/tauri](https://github.com/tauri-apps/tauri): Build smaller, faster, and more secure desktop applications with a web frontend.
- [Eugeny/russh](https://github.com/Eugeny/russh): Rust SSH client and server library.
- [tokio-rs/tokio](https://github.com/tokio-rs/tokio): An asynchronous runtime for Rust.
- [facebook/react](https://github.com/facebook/react): The library for web and native user interfaces.
- [vitejs/vite](https://github.com/vitejs/vite): Next generation frontend tooling.
- [tailwindlabs/tailwindcss](https://github.com/tailwindlabs/tailwindcss): A utility-first CSS framework.
- [lucide-icons/lucide](https://github.com/lucide-icons/lucide): Beautiful and consistent icons.
