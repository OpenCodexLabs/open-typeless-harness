# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

OpenLess is a menu-bar/tray voice-input layer. Hold or toggle a global hotkey, speak, and the dictated text is polished and inserted at the current cursor in any app. Product principles, state machine, and module list live in `docs/openless-development.md` and `docs/openless-overall-logic.md` — read those before changing product behavior.

The active codebase lives at `openless-all/app/` and is **Tauri 2 + Rust backend + React/TS frontend**, targeting macOS 12+ and Windows. The legacy Swift implementation (Sources/, Tests/, Package.swift, appcast.xml, Sparkle pipeline) was removed in commit `34d2823`; do not resurrect it.

UI must match `openless-all/design_handoff_openless/*.jsx` pixel-for-pixel; the JSX is reference-only, never imported.

## Build, Run, Test

### Tauri (current — start here)

```bash
cd "openless-all/app"
npm ci

# Dev: vite at :1420 + tauri shell
npm run tauri dev

# Build .app (+ DMG) — use this script, not `tauri build` directly,
# because it threads Apple signing env vars and validates Info.plist.
./scripts/build-mac.sh           # build, sign, install to /Applications, reset TCC
INSTALL=0 ./scripts/build-mac.sh # build only

# Frontend-only TS check
npm run build   # = tsc && vite build

# Rust type-check without full compile
cargo check --manifest-path src-tauri/Cargo.toml
```

### Windows (cross-check only — no macOS runner in CI)

```powershell
# Preflight: verify toolchain
.\scripts\windows-preflight.ps1

# Build (requires Windows host or cross-compile target)
.\scripts\windows-build-gnu.ps1
```

Generated artifacts:
- `openless-all/app/src-tauri/target/release/bundle/macos/OpenLess.app`
- `openless-all/app/src-tauri/target/release/bundle/dmg/OpenLess_<version>_aarch64.dmg`

Logs: `~/Library/Logs/OpenLess/openless.log` (macOS) / `%LOCALAPPDATA%\OpenLess\Logs\openless.log` (Windows).

There is no test runner wired in for the frontend. `src/lib/providerSetup.test.ts` is a hand-rolled assertion script — run with `npx tsx src/lib/providerSetup.test.ts` if you need it. Rust side has no `cargo test` targets yet; behavior is verified by running the app.

## Architecture

`coordinator::Coordinator` is the **single owner of session state**. Hotkey edges drive a small phase enum (`Idle → Starting → Listening → Processing`); recorder, ASR, polish, insertion, and history are wired here and nowhere else. Library/module code never calls across modules — they each depend only on shared types.

```
Rust (openless-all/app/src-tauri/src)        Purpose
──────────────────────────────────────        ────────────────────────────────
types.rs                                      Pure value types: DictationSession, PolishMode, HotkeyBinding, errors
hotkey.rs                                     Global hotkey monitor (modifier-key edges)
recorder.rs                                   Mic → 16 kHz mono Int16 PCM, RMS callback
asr/{mod,frame,volcengine,whisper}.rs         ASR providers: Volcengine streaming WebSocket + Whisper HTTP
polish.rs                                     OpenAI-compatible chat completions (Ark / DeepSeek / etc.)
insertion.rs                                  AX focused-element write → clipboard + Cmd+V → copy-only fallback
persistence.rs                                History/preferences/vocab JSON + Keychain credentials
coordinator.rs + commands.rs + lib.rs         State machine, IPC surface, tray icon, window plumbing
permissions.rs                                TCC checks (Accessibility / Microphone)

Frontend (openless-all/app/src)
src/components/Capsule.tsx                    Capsule view + state enum
src/ (React)                                  Main window UI: Overview / History / Vocab / Style / Settings
src/i18n/                                     react-i18next init + zh-CN / en resources
src/pages/_atoms.tsx                          Recoil atoms — global frontend state
src/state/HotkeySettingsContext.tsx           HotkeySettings React context (capability + binding from backend)
```

### Dictation pipeline

```
hotkey edge (1st)  →  beginSession:  Recorder.start → ASR.openSession → BufferingAudioConsumer.attach
hotkey edge (2nd)  →  endSession:    Recorder.stop → ASR.sendLastFrame → awaitFinal → Polish → Insert → History.save
.cancelled         →  ASR.cancel, Recorder.stop, capsule .cancelled
```

Invariants:
- **Polish/ASR fallbacks are silent.** Missing Ark creds → insert raw transcript. Missing Volcengine creds → mock pipeline copies a placeholder. The contract is *"the user's words don't get lost"* — don't add hard errors here.
- **`BufferingAudioConsumer`** queues PCM until the WebSocket is ready, then drains. Recorder always pushes to it; ASR is attached after `openSession` resolves.
- **Hotkey is toggle-only**, not press-and-hold. The monitor yields one edge per modifier-key keydown; the coordinator interprets odd/even.

### Permissions, credentials, on-disk state

- **Bundle ID `com.openless.app`** is hard-coded in `openless-all/app/src-tauri/tauri.conf.json` and `CredentialsVault.serviceName`. Changing it breaks Keychain lookups *and* every existing TCC grant.
- **TCC**: Microphone + Accessibility + AppleEvents. `NSMicrophoneUsageDescription` / `NSAccessibilityUsageDescription` / `NSAppleEventsUsageDescription` live in `openless-all/app/src-tauri/Info.plist`. After a fresh build that resets TCC, the app must be **fully quit and relaunched** after granting Accessibility before the global hotkey tap installs.
- **Credentials** live in Keychain under accounts in `CredentialAccount` (`volcengine.app_key`, `volcengine.access_key`, `volcengine.resource_id`, `ark.api_key`, `ark.model_id`, `ark.endpoint`). The plaintext fallback at `~/.openless/credentials.json` is read on first launch so legacy users keep their creds without re-entering. Never hard-code keys.
- **Per-user data**:
  - macOS: `~/Library/Application Support/OpenLess/{history.json, preferences.json, dictionary.json}` — capped at 200 history entries. **Do not rename `dictionary.json` to `vocab.json`** (drops user data).
  - Windows: `%APPDATA%\OpenLess\`
  - Linux: `$XDG_DATA_HOME/OpenLess`

### Release pipeline

Push a `v*-tauri` tag → `.github/workflows/release-tauri.yml` builds macOS arm64 `.dmg` and Windows x64 `.msi`. macOS Developer ID signing + notarization runs only when `APPLE_CERTIFICATE` / `APPLE_CERTIFICATE_PASSWORD` / `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` secrets are set; otherwise it falls back to ad-hoc signing with a CI warning.

When bumping versions, update **both** `version` fields: `openless-all/app/package.json` and `openless-all/app/src-tauri/tauri.conf.json` (and `Cargo.toml`).

## Repo conventions

- **Comments, log messages, user-facing strings, and most docs are in Simplified Chinese.** UI strings additionally route through `react-i18next` (`src/i18n/{zh-CN,en}.ts`) so we ship English alongside; `zh-CN.ts` is source of truth.
- **macOS hotkey monitor must use native `CGEventTap`, never `rdev`.** `rdev` synchronously calls `TSMGetInputSourceProperty` from non-main threads, which macOS 14+ aborts via `dispatch_assert_queue_fail` → SIGTRAP. macOS uses CGEventTap; `rdev` is only used on Linux/Windows.
- **Don't `NSApp.activate` on the dictation path** — it steals focus and breaks insertion. Only call `set_activation_policy(Regular)` + `activateIgnoringOtherApps` from `show_main_window` / mic-permission prompts, never from `start_dictation`.
- Rust modules wrap shared mutable state with `Arc<Mutex<...>>` (parking_lot). Keep that locking discipline when adding fields.
- Rust modules depend only on `types.rs`. New cross-module wiring goes in `coordinator.rs`, not in the leaf modules.

### Adding a new module

1. Add a `<name>.rs` (or directory) under `openless-all/app/src-tauri/src/`, importing only from `types`.
2. Register it in `lib.rs` (`mod <name>;`).
3. Wire it into `coordinator.rs` and expose any frontend-callable surface via `commands.rs` + `invoke_handler!`.
4. Add the matching TS wrapper in `openless-all/app/src/lib/ipc.ts` (with a mock branch for browser dev).
