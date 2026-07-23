# macOS release checklist (Jira Analytics)

Local-only Tauri desktop app. Shared install requirements (macOS + Windows): [install.md](./install.md).

Bundle identity:

| Field | Value |
|-------|-------|
| `productName` | Jira Analytics |
| `identifier` | `com.aandganalytics.app` |
| Artifact | Signed + notarized `.dmg` (`.app` also produced) |

## Toolchain

### Rust / library tests vs desktop build

- **Library / CI path** (`cargo test --workspace`, no `desktop` feature): keep working on the pinned workspace Rust (see `rust-toolchain.toml`). Command logic lives in `*_inner` helpers so CI does not need the Tauri webview stack.
- **Desktop / DMG path** (`cargo tauri build` with `--features desktop`): use the pinned toolchain (currently **1.88**). Install or upgrade with `rustup` if the build rejects the toolchain.
- Do not raise the workspace MSRV solely for packaging if it breaks the library-testable CI path; use a newer toolchain only on release machines when needed.

### Other prerequisites

- Xcode Command Line Tools (and full Xcode for notarization tooling)
- Node.js for `ui` build (`beforeBuildCommand`)
- Apple Developer Program membership for signing + notarization
- `cargo install tauri-cli` (or `npm`/`pnpm` Tauri CLI) matching Tauri 2

## Icons

```bash
# Place source PNG (1024×1024), then:
cargo tauri icon path/to/app-icon.png
```

## Unsigned local smoke (no Apple certs)

```bash
cd src-tauri
cargo tauri build --features desktop --bundles dmg
```

Artifacts land under `target/release/bundle/` (or `src-tauri/target/release/bundle/`). Without signing identity this produces an unsigned DMG suitable only for local smoke tests.

## Signing

1. Create a **Developer ID Application** certificate in Apple Developer → Certificates.
2. Import it into the login keychain on the release Mac.
3. Either set in `tauri.conf.json` → `bundle.macOS.signingIdentity`, or export:

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
```

4. Hardened Runtime is already enabled (`bundle.macOS.hardenedRuntime: true`).

## Notarization

Provide Apple ID credentials for `notarytool` (app-specific password recommended):

```bash
export APPLE_ID="you@example.com"
export APPLE_PASSWORD="app-specific-password"
export APPLE_TEAM_ID="TEAMID"
```

Then:

```bash
cargo tauri build --features desktop --bundles dmg
```

Tauri signs, notarizes, and staples when these variables (and identity) are present. To skip stapling on a first pass, append `--skip-stapling` to the Tauri CLI invocation.

Verify:

```bash
spctl --assess --type open --verbose "target/release/bundle/macos/Jira Analytics.app"
xcrun stapler validate "target/release/bundle/dmg/"*.dmg
```

## Manual QA after install

1. Fresh install from DMG → Setup → save Jira + optional Bedrock credentials (keychain).
2. Full sync on allowlisted network (office/VPN) → dashboards populate.
3. Quit mid-sync → relaunch → resume / incremental.
4. Offline: disconnect network → Home/Flow still load from SQLite; SyncBanner shows offline copy.
5. Bad token / IP allowlist: force 401/403 → banner + Settings prompt.
6. Maintenance: **Rebuild derived** (raw issues retained) vs **Full re-sync** (checkpoints cleared, credentials kept).
7. Ask AI once with context pack preview visible (if Bedrock key set).

## Related

- Windows build / NSIS: [windows.md](./windows.md)
- Combined install page: [install.md](./install.md)

## Deferred

- Prebuilt Windows/Linux download artifacts in CI
- Auto-update channel
- Shared DB snapshots
