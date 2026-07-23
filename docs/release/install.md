# Install Jira Analytics

Local-only Tauri desktop app for Auto General AU Jira. Each machine keeps its own credentials (OS keychain / Credential Manager) and SQLite database.

| Field | Value |
|-------|-------|
| Product | Jira Analytics |
| Identifier | `com.aandganalytics.app` |
| Site | `https://autogeneral-au.atlassian.net` |

**Network note:** Auto General Jira uses an **IP allowlist**. Install and sync from an allowlisted network (office / company VPN). Home networks will get HTTP 403 even with a valid API token.

---

## macOS (prebuilt)

### Requirements

- macOS **11.0+** (Apple Silicon or Intel)
- No separate runtime install for the signed/notarized DMG path

### Install

1. Open `Jira Analytics_0.1.0_*.dmg`
2. Drag **Jira Analytics** into Applications
3. Launch → complete Setup (Atlassian email + Jira API token; Bedrock key optional)

### Build from source (Mac)

| Requirement | Notes |
|-------------|--------|
| Xcode Command Line Tools | `xcode-select --install` |
| Node.js LTS | For `ui` build |
| Rust | Workspace pins **1.88** (`rust-toolchain.toml`); `rustup` recommended |
| Tauri CLI 2 | `cargo install tauri-cli --version "^2"` |

```bash
npm --prefix ui install
cargo tauri build --features desktop --bundles dmg
```

Artifacts: `target/release/bundle/dmg/` and `target/release/bundle/macos/`.

Full signing / notarization checklist: [macos.md](./macos.md).

---

## Windows (build on Windows)

There is no prebuilt Windows installer in this repo yet. Build on a **Windows PC on the A&G network** (or VPN), then run the NSIS installer locally.

### Requirements

| Requirement | Notes |
|-------------|--------|
| Windows 10 / 11 | x64 recommended |
| [Node.js LTS](https://nodejs.org/) | For `ui` build |
| [Rust](https://rustup.rs/) | Stable via `rustup`; open a **new** PowerShell after install |
| [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) | Workload: **Desktop development with C++** |
| [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/) | Usually already present on Win10/11 |
| Git | To clone the repo |
| Tauri CLI 2 | `cargo install tauri-cli --version "^2"` |
| A&G network / VPN | Required for Jira API (IP allowlist) |

### Install steps (from source)

```powershell
git clone <repo-url> AandGAnalytics
cd AandGAnalytics
npm --prefix ui install
cargo install tauri-cli --version "^2"

# Dev (hot reload)
cargo tauri dev --features desktop

# Release installer
cargo tauri build --features desktop --bundles nsis
```

Installer output:

`src-tauri\target\release\bundle\nsis\` (or workspace `target\release\bundle\nsis\`)

Run the `.exe` installer, then launch **Jira Analytics** and complete Setup.

### First-run Setup (both platforms)

1. **Atlassian email** — account that owns the API token  
2. **Jira API token** — create with **Create API token** (not “with scopes”) at [id.atlassian.com/manage-profile/security/api-tokens](https://id.atlassian.com/manage-profile/security/api-tokens)  
3. **AWS Bedrock API key** — optional (Ask AI only)  
4. Use **Test connection** before continuing  

Credentials stay on the device. Sync data is local SQLite under the OS app data directory.

### Windows notes

- Unsigned builds may trigger SmartScreen → **More info → Run anyway**
- Secrets use Windows Credential Manager (`com.aandganalytics.desktop`)
- You cannot build a Windows NSIS package from macOS without a Windows toolchain / CI runner

Full Windows packaging notes: [windows.md](./windows.md).
