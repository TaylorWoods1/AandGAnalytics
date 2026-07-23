# Windows release checklist (Jira Analytics)

Local-only Tauri desktop app. See also the shared install page: [install.md](./install.md).

| Field | Value |
|-------|-------|
| `productName` | Jira Analytics |
| `identifier` | `com.aandganalytics.app` |
| Artifact | NSIS installer (`.exe`) |

## Prerequisites

Install on the **Windows** machine that will build (preferably on the A&G network so you can smoke-test Jira after packaging):

1. **Node.js LTS** — https://nodejs.org/
2. **Rust** — https://rustup.rs/ (stable). Confirm with `rustc --version` in a new PowerShell.
3. **Visual Studio Build Tools** — https://visualstudio.microsoft.com/visual-cpp-build-tools/  
   Select workload **Desktop development with C++**.
4. **WebView2 Runtime** — https://developer.microsoft.com/microsoft-edge/webview2/  
   (Often already installed with Edge / Windows 10/11.)
5. **Git**
6. **Tauri CLI 2:**

```powershell
cargo install tauri-cli --version "^2"
```

## Build

```powershell
cd AandGAnalytics
npm --prefix ui install
cargo tauri build --features desktop --bundles nsis
```

Find the installer under:

- `target\release\bundle\nsis\` (workspace target), or
- `src-tauri\target\release\bundle\nsis\` (depending on Cargo layout)

## Dev loop

```powershell
cargo tauri dev --features desktop
```

## Manual QA after install

1. Fresh install → Setup → save Jira credentials (Credential Manager) + optional Bedrock key.
2. Confirm you are on **A&G network / VPN** (IP allowlist); otherwise Test connection returns 403.
3. Full sync → dashboards populate.
4. Quit mid-sync → relaunch → resume / incremental.
5. Offline: dashboards still load from SQLite.
6. Settings → Test connection / Clear credentials & local data.
7. Ask AI once if Bedrock key is configured.

## Signing (optional)

Unsigned NSIS builds work for internal use but may show SmartScreen. For wider distribution, sign the installer with an Authenticode certificate (`signtool`) after `tauri build`.

## Network / allowlist

Auto General Jira blocks non-allowlisted IPs. Building or running off-network will fail Setup/sync with an IP allowlist 403 even when the API token is valid.
