# Windows Temporary TSF IME Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Windows-only TSF input-method backend that temporarily activates OpenLess during a voice session, commits the final dictated text through TSF, and restores the user's previous input method.

**Architecture:** Keep the existing Tauri/Rust app as the only owner of hotkeys, recording, ASR, polish, UI, history, and fallback insertion. Add a small Windows TSF COM DLL that registers as an input processor and accepts one active `SubmitText` request from the app over a local named pipe. Add a Rust Windows IME controller that records/restores the active input profile and falls back to the current `WM_PASTE` path whenever TSF activation or commit cannot complete.

**Tech Stack:** Rust 2021, Tauri 2, `windows` crate Win32/TSF bindings, Tokio named pipes on Windows, C++17 Windows SDK COM/TSF DLL, PowerShell registration scripts, React/TypeScript settings surface.

---

## File Structure

- Create: `openless-all/app/src-tauri/src/windows_ime_protocol.rs`
  - Shared Rust message types for app-side JSONL IPC.
  - Pure tests for serialization and stale session rejection.
- Create: `openless-all/app/src-tauri/src/windows_ime_profile.rs`
  - Windows-only TSF profile snapshot, OpenLess profile activation, and restoration.
  - Non-Windows stub so cross-platform builds keep compiling.
- Create: `openless-all/app/src-tauri/src/windows_ime_ipc.rs`
  - Windows-only named-pipe server that tracks the most recent OpenLess IME client and submits text with a timeout.
  - Non-Windows stub returning `Unavailable`.
- Create: `openless-all/app/src-tauri/src/windows_ime_session.rs`
  - Session guard that combines profile switching, IPC submit, fallback routing, and restoration.
- Modify: `openless-all/app/src-tauri/src/insertion.rs`
  - Keep existing clipboard/`WM_PASTE` insertion as the Windows fallback and expose a clearly named fallback method.
- Modify: `openless-all/app/src-tauri/src/coordinator.rs`
  - Prepare Windows IME session on voice-session start.
  - Submit through TSF first on Windows, then fallback.
  - Restore input profile on success, failure, and cancellation.
- Modify: `openless-all/app/src-tauri/src/lib.rs`
  - Register the new Rust modules and Tauri commands.
- Modify: `openless-all/app/src-tauri/src/commands.rs`
  - Expose Windows IME install/status commands.
- Modify: `openless-all/app/src-tauri/src/types.rs`
  - Add Windows IME status value types for IPC to the frontend.
- Modify: `openless-all/app/src-tauri/Cargo.toml`
  - Add Windows API feature gates required for COM, TSF, named-pipe helpers, registry, and process/thread lookup.
- Create: `openless-all/app/windows-ime/OpenLessIme.sln`
- Create: `openless-all/app/windows-ime/OpenLessIme.vcxproj`
- Create: `openless-all/app/windows-ime/src/guids.h`
- Create: `openless-all/app/windows-ime/src/dllmain.cpp`
- Create: `openless-all/app/windows-ime/src/class_factory.h`
- Create: `openless-all/app/windows-ime/src/class_factory.cpp`
- Create: `openless-all/app/windows-ime/src/text_service.h`
- Create: `openless-all/app/windows-ime/src/text_service.cpp`
- Create: `openless-all/app/windows-ime/src/edit_session.h`
- Create: `openless-all/app/windows-ime/src/edit_session.cpp`
- Create: `openless-all/app/windows-ime/src/ipc_client.h`
- Create: `openless-all/app/windows-ime/src/ipc_client.cpp`
- Create: `openless-all/app/windows-ime/src/registry.h`
- Create: `openless-all/app/windows-ime/src/registry.cpp`
- Create: `openless-all/app/windows-ime/src/resource.rc`
  - Minimal C++ TSF text service DLL.
- Create: `openless-all/app/scripts/windows-ime-register.ps1`
- Create: `openless-all/app/scripts/windows-ime-unregister.ps1`
- Create: `openless-all/app/scripts/windows-ime-build.ps1`
  - Build, register, and unregister scripts for the TSF DLL.
- Modify: `openless-all/app/scripts/windows-preflight.ps1`
  - Check MSBuild and Windows SDK when TSF IME work is requested.
- Modify: `openless-all/app/src/lib/types.ts`
- Modify: `openless-all/app/src/lib/ipc.ts`
- Modify: `openless-all/app/src/i18n/zh-CN.ts`
- Modify: `openless-all/app/src/i18n/en.ts`
- Modify: `openless-all/app/src/pages/Settings.tsx`
  - Windows-only TSF IME status and actions.

Use these fixed identifiers in every Rust, C++, and script location:

```text
OpenLess TSF text service CLSID: {6B9F3F4F-5EE7-42D6-9C61-9F80B03A5D7D}
OpenLess TSF profile GUID:       {9B5F5E04-23F6-47DA-9A26-D221F6C3F02E}
OpenLess TSF category GUID:      GUID_TFCAT_TIP_KEYBOARD
OpenLess TSF language id:        0x0804
OpenLess named pipe:             \\.\pipe\OpenLessImeSubmit
OpenLess protocol version:       1
```

---

### Task 1: Shared IME IPC Protocol

**Files:**
- Create: `openless-all/app/src-tauri/src/windows_ime_protocol.rs`
- Modify: `openless-all/app/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing protocol serialization tests**

Add this test module to the new file before adding production types:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_text_roundtrips_as_camel_case_json() {
        let message = ImePipeMessage::SubmitText {
            protocol_version: OPENLESS_IME_PROTOCOL_VERSION,
            session_id: "session-1".to_string(),
            text: "你好 OpenLess".to_string(),
            created_at: "2026-05-01T12:00:00Z".to_string(),
        };

        let json = encode_message(&message).expect("encode");
        assert!(json.contains("\"submitText\""));
        assert!(json.ends_with('\n'));

        let decoded = decode_message(json.trim_end()).expect("decode");
        assert_eq!(decoded, message);
    }

    #[test]
    fn stale_submit_result_is_rejected() {
        let result = ImePipeMessage::SubmitResult {
            protocol_version: OPENLESS_IME_PROTOCOL_VERSION,
            session_id: "old-session".to_string(),
            status: ImeSubmitStatus::Committed,
            error_code: None,
        };

        assert!(is_result_for_pending_session(&result, "current-session").is_err());
        assert!(is_result_for_pending_session(&result, "old-session").is_ok());
    }
}
```

- [ ] **Step 2: Run the test and verify it fails because types are missing**

Run:

```powershell
cargo test --manifest-path openless-all/app/src-tauri/Cargo.toml windows_ime_protocol --lib
```

Expected: compile fails with missing `ImePipeMessage`, `OPENLESS_IME_PROTOCOL_VERSION`, `encode_message`, `decode_message`, `ImeSubmitStatus`, and `is_result_for_pending_session`.

- [ ] **Step 3: Add the protocol implementation**

Put this implementation above the test module:

```rust
use serde::{Deserialize, Serialize};

pub const OPENLESS_IME_PROTOCOL_VERSION: u32 = 1;
pub const OPENLESS_IME_PIPE_NAME: &str = r"\\.\pipe\OpenLessImeSubmit";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ImePipeMessage {
    ClientReady {
        protocol_version: u32,
        client_id: String,
        process_id: u32,
        thread_id: u32,
    },
    SubmitText {
        protocol_version: u32,
        session_id: String,
        text: String,
        created_at: String,
    },
    SubmitResult {
        protocol_version: u32,
        session_id: String,
        status: ImeSubmitStatus,
        error_code: Option<String>,
    },
    CancelSession {
        protocol_version: u32,
        session_id: String,
    },
    Ping {
        protocol_version: u32,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ImeSubmitStatus {
    Committed,
    Rejected,
    Failed,
}

pub fn encode_message(message: &ImePipeMessage) -> Result<String, serde_json::Error> {
    let mut line = serde_json::to_string(message)?;
    line.push('\n');
    Ok(line)
}

pub fn decode_message(line: &str) -> Result<ImePipeMessage, serde_json::Error> {
    serde_json::from_str(line)
}

pub fn is_result_for_pending_session(
    message: &ImePipeMessage,
    pending_session_id: &str,
) -> Result<(), &'static str> {
    match message {
        ImePipeMessage::SubmitResult { session_id, .. } if session_id == pending_session_id => Ok(()),
        ImePipeMessage::SubmitResult { .. } => Err("submit result belongs to a different session"),
        _ => Err("message is not a submit result"),
    }
}
```

- [ ] **Step 4: Register the module**

Add this to `openless-all/app/src-tauri/src/lib.rs` beside the other `mod` declarations:

```rust
mod windows_ime_protocol;
```

- [ ] **Step 5: Run the protocol tests**

Run:

```powershell
cargo test --manifest-path openless-all/app/src-tauri/Cargo.toml windows_ime_protocol --lib
```

Expected: both protocol tests pass.

- [ ] **Step 6: Commit**

```powershell
git add -- openless-all/app/src-tauri/src/windows_ime_protocol.rs openless-all/app/src-tauri/src/lib.rs
git commit -m "feat: add Windows IME IPC protocol"
```

---

### Task 2: Profile Snapshot State Machine

**Files:**
- Create: `openless-all/app/src-tauri/src/windows_ime_profile.rs`
- Modify: `openless-all/app/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing pure state tests**

Create `openless-all/app/src-tauri/src/windows_ime_profile.rs` with these tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn text_service_snapshot() -> ImeProfileSnapshot {
        ImeProfileSnapshot {
            kind: ImeProfileKind::TextService,
            lang_id: 0x0804,
            clsid: Some("{11111111-1111-1111-1111-111111111111}".to_string()),
            profile_guid: Some("{22222222-2222-2222-2222-222222222222}".to_string()),
            hkl: None,
        }
    }

    #[test]
    fn restore_is_required_when_openless_is_active_and_snapshot_exists() {
        assert_eq!(
            restore_decision(Some(&text_service_snapshot()), true),
            ProfileRestoreDecision::RestoreSavedProfile
        );
    }

    #[test]
    fn restore_is_skipped_when_snapshot_is_missing() {
        assert_eq!(
            restore_decision(None, true),
            ProfileRestoreDecision::KeepCurrentProfile
        );
    }

    #[test]
    fn restore_is_skipped_when_user_already_changed_away_from_openless() {
        assert_eq!(
            restore_decision(Some(&text_service_snapshot()), false),
            ProfileRestoreDecision::KeepCurrentProfile
        );
    }
}
```

- [ ] **Step 2: Run the test and verify it fails because profile types are missing**

Run:

```powershell
cargo test --manifest-path openless-all/app/src-tauri/Cargo.toml windows_ime_profile --lib
```

Expected: compile fails with missing snapshot and decision types.

- [ ] **Step 3: Add platform-neutral profile types**

Add this implementation above the test module:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeProfileKind {
    KeyboardLayout,
    TextService,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImeProfileSnapshot {
    pub kind: ImeProfileKind,
    pub lang_id: u16,
    pub clsid: Option<String>,
    pub profile_guid: Option<String>,
    pub hkl: Option<isize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileRestoreDecision {
    RestoreSavedProfile,
    KeepCurrentProfile,
}

pub fn restore_decision(
    saved: Option<&ImeProfileSnapshot>,
    openless_profile_is_current: bool,
) -> ProfileRestoreDecision {
    if saved.is_some() && openless_profile_is_current {
        ProfileRestoreDecision::RestoreSavedProfile
    } else {
        ProfileRestoreDecision::KeepCurrentProfile
    }
}
```

- [ ] **Step 4: Add public manager API with non-Windows stub**

Add this API below the pure types:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowsImeProfileError {
    Unavailable(String),
    WindowsApi(String),
}

impl std::fmt::Display for WindowsImeProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(message) | Self::WindowsApi(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for WindowsImeProfileError {}

pub type WindowsImeProfileResult<T> = Result<T, WindowsImeProfileError>;

#[cfg(not(target_os = "windows"))]
pub struct WindowsImeProfileManager;

#[cfg(not(target_os = "windows"))]
impl WindowsImeProfileManager {
    pub fn new() -> Self {
        Self
    }

    pub fn capture_active_profile(&self) -> WindowsImeProfileResult<ImeProfileSnapshot> {
        Err(WindowsImeProfileError::Unavailable(
            "Windows TSF profiles are only available on Windows".to_string(),
        ))
    }

    pub fn activate_openless_profile(&self) -> WindowsImeProfileResult<()> {
        Err(WindowsImeProfileError::Unavailable(
            "Windows TSF profiles are only available on Windows".to_string(),
        ))
    }

    pub fn restore_profile(&self, _snapshot: &ImeProfileSnapshot) -> WindowsImeProfileResult<()> {
        Err(WindowsImeProfileError::Unavailable(
            "Windows TSF profiles are only available on Windows".to_string(),
        ))
    }

    pub fn is_openless_profile_active(&self) -> WindowsImeProfileResult<bool> {
        Ok(false)
    }
}
```

- [ ] **Step 5: Register the module**

Add this to `openless-all/app/src-tauri/src/lib.rs`:

```rust
mod windows_ime_profile;
```

- [ ] **Step 6: Run the profile tests**

Run:

```powershell
cargo test --manifest-path openless-all/app/src-tauri/Cargo.toml windows_ime_profile --lib
```

Expected: all profile state tests pass.

- [ ] **Step 7: Commit**

```powershell
git add -- openless-all/app/src-tauri/src/windows_ime_profile.rs openless-all/app/src-tauri/src/lib.rs
git commit -m "feat: add Windows IME profile state"
```

---

### Task 3: Windows TSF Profile Manager

**Files:**
- Modify: `openless-all/app/src-tauri/Cargo.toml`
- Modify: `openless-all/app/src-tauri/src/windows_ime_profile.rs`

- [ ] **Step 1: Write failing Windows-only compile test for fixed identifiers**

Add these tests inside `windows_ime_profile.rs`:

```rust
#[cfg(all(test, target_os = "windows"))]
mod windows_tests {
    use super::*;

    #[test]
    fn openless_profile_identifiers_are_fixed() {
        assert_eq!(OPENLESS_TSF_LANG_ID, 0x0804);
        assert_eq!(
            OPENLESS_TEXT_SERVICE_CLSID_BRACED,
            "{6B9F3F4F-5EE7-42D6-9C61-9F80B03A5D7D}"
        );
        assert_eq!(
            OPENLESS_PROFILE_GUID_BRACED,
            "{9B5F5E04-23F6-47DA-9A26-D221F6C3F02E}"
        );
    }
}
```

- [ ] **Step 2: Run the Windows-only test and verify it fails**

Run on Windows:

```powershell
cargo test --manifest-path openless-all/app/src-tauri/Cargo.toml openless_profile_identifiers_are_fixed --lib
```

Expected: compile fails because the constants are not defined.

- [ ] **Step 3: Extend Windows API features**

In `openless-all/app/src-tauri/Cargo.toml`, extend the Windows dependency features to include:

```toml
  "Win32_Globalization",
  "Win32_System_Com",
  "Win32_System_Ole",
  "Win32_System_Registry",
  "Win32_UI_TextServices",
```

The resulting Windows dependency block keeps the existing features and includes the new ones:

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
  "Win32_Foundation",
  "Win32_Globalization",
  "Win32_System_Com",
  "Win32_System_Ole",
  "Win32_System_Registry",
  "Win32_System_Threading",
  "Win32_UI_Input_KeyboardAndMouse",
  "Win32_UI_Shell",
  "Win32_UI_TextServices",
  "Win32_UI_WindowsAndMessaging",
] }
winreg = "0.52"
```

- [ ] **Step 4: Add Windows constants and GUID parsing helpers**

Add these items near the top of `windows_ime_profile.rs`:

```rust
pub const OPENLESS_TSF_LANG_ID: u16 = 0x0804;
pub const OPENLESS_TEXT_SERVICE_CLSID_BRACED: &str =
    "{6B9F3F4F-5EE7-42D6-9C61-9F80B03A5D7D}";
pub const OPENLESS_PROFILE_GUID_BRACED: &str =
    "{9B5F5E04-23F6-47DA-9A26-D221F6C3F02E}";

#[cfg(target_os = "windows")]
fn parse_guid(value: &str) -> WindowsImeProfileResult<windows::core::GUID> {
    windows::core::GUID::from(value).map_err(|err| {
        WindowsImeProfileError::WindowsApi(format!("invalid GUID {value}: {err}"))
    })
}
```

- [ ] **Step 5: Add Windows profile manager skeleton**

Replace the non-Windows-only manager coverage with a Windows implementation guarded by `#[cfg(target_os = "windows")]`:

```rust
#[cfg(target_os = "windows")]
pub struct WindowsImeProfileManager;

#[cfg(target_os = "windows")]
impl WindowsImeProfileManager {
    pub fn new() -> Self {
        Self
    }

    pub fn capture_active_profile(&self) -> WindowsImeProfileResult<ImeProfileSnapshot> {
        windows_impl::capture_active_profile()
    }

    pub fn activate_openless_profile(&self) -> WindowsImeProfileResult<()> {
        windows_impl::activate_openless_profile()
    }

    pub fn restore_profile(&self, snapshot: &ImeProfileSnapshot) -> WindowsImeProfileResult<()> {
        windows_impl::restore_profile(snapshot)
    }

    pub fn is_openless_profile_active(&self) -> WindowsImeProfileResult<bool> {
        windows_impl::is_openless_profile_active()
    }
}
```

- [ ] **Step 6: Implement the Windows TSF calls**

Add this module in `windows_ime_profile.rs`. Keep all COM calls inside this module:

```rust
#[cfg(target_os = "windows")]
mod windows_impl {
    use super::*;
    use windows::core::{Interface, GUID};
    use windows::Win32::Foundation::HKL;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
    use windows::Win32::UI::TextServices::{
        ITfInputProcessorProfileMgr, CLSID_TF_InputProcessorProfiles,
        TF_PROFILETYPE_INPUTPROCESSOR, TF_PROFILETYPE_KEYBOARDLAYOUT,
        TF_IPPMF_FORPROCESS,
    };

    struct ComApartment;

    impl ComApartment {
        fn init() -> WindowsImeProfileResult<Self> {
            unsafe {
                CoInitializeEx(None, COINIT_APARTMENTTHREADED).map_err(|err| {
                    WindowsImeProfileError::WindowsApi(format!("CoInitializeEx failed: {err}"))
                })?;
            }
            Ok(Self)
        }
    }

    impl Drop for ComApartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }

    fn profile_mgr() -> WindowsImeProfileResult<ITfInputProcessorProfileMgr> {
        unsafe {
            CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)
                .map_err(|err| {
                    WindowsImeProfileError::WindowsApi(format!(
                        "CoCreateInstance(CLSID_TF_InputProcessorProfiles) failed: {err}"
                    ))
                })
        }
    }

    pub fn capture_active_profile() -> WindowsImeProfileResult<ImeProfileSnapshot> {
        let _com = ComApartment::init()?;
        let mgr = profile_mgr()?;
        unsafe {
            let profile = mgr.GetActiveProfile(GUID::zeroed()).map_err(|err| {
                WindowsImeProfileError::WindowsApi(format!("GetActiveProfile failed: {err}"))
            })?;
            if profile.dwProfileType == TF_PROFILETYPE_INPUTPROCESSOR {
                return Ok(ImeProfileSnapshot {
                    kind: ImeProfileKind::TextService,
                    lang_id: profile.langid as u16,
                    clsid: Some(format!("{:?}", profile.clsid)),
                    profile_guid: Some(format!("{:?}", profile.guidProfile)),
                    hkl: None,
                });
            }
            let hkl = GetKeyboardLayout(0);
            Ok(ImeProfileSnapshot {
                kind: ImeProfileKind::KeyboardLayout,
                lang_id: profile.langid as u16,
                clsid: None,
                profile_guid: None,
                hkl: Some(hkl.0),
            })
        }
    }

    pub fn activate_openless_profile() -> WindowsImeProfileResult<()> {
        let _com = ComApartment::init()?;
        let mgr = profile_mgr()?;
        let clsid = parse_guid(OPENLESS_TEXT_SERVICE_CLSID_BRACED)?;
        let profile_guid = parse_guid(OPENLESS_PROFILE_GUID_BRACED)?;
        unsafe {
            mgr.ActivateProfile(
                TF_PROFILETYPE_INPUTPROCESSOR,
                OPENLESS_TSF_LANG_ID,
                &clsid,
                &profile_guid,
                windows::Win32::Foundation::HKL(0),
                TF_IPPMF_FORPROCESS,
            )
            .map_err(|err| {
                WindowsImeProfileError::WindowsApi(format!(
                    "ActivateProfile(OpenLess) failed: {err}"
                ))
            })
        }
    }

    pub fn restore_profile(snapshot: &ImeProfileSnapshot) -> WindowsImeProfileResult<()> {
        let _com = ComApartment::init()?;
        let mgr = profile_mgr()?;
        unsafe {
            match snapshot.kind {
                ImeProfileKind::TextService => {
                    let clsid = parse_guid(snapshot.clsid.as_deref().ok_or_else(|| {
                        WindowsImeProfileError::WindowsApi(
                            "saved text service profile has no CLSID".to_string(),
                        )
                    })?)?;
                    let profile_guid = parse_guid(snapshot.profile_guid.as_deref().ok_or_else(|| {
                        WindowsImeProfileError::WindowsApi(
                            "saved text service profile has no profile GUID".to_string(),
                        )
                    })?)?;
                    mgr.ActivateProfile(
                        TF_PROFILETYPE_INPUTPROCESSOR,
                        snapshot.lang_id,
                        &clsid,
                        &profile_guid,
                        HKL(0),
                        TF_IPPMF_FORPROCESS,
                    )
                }
                ImeProfileKind::KeyboardLayout => {
                    mgr.ActivateProfile(
                        TF_PROFILETYPE_KEYBOARDLAYOUT,
                        snapshot.lang_id,
                        &GUID::zeroed(),
                        &GUID::zeroed(),
                        HKL(snapshot.hkl.unwrap_or_default()),
                        TF_IPPMF_FORPROCESS,
                    )
                }
            }
            .map_err(|err| {
                WindowsImeProfileError::WindowsApi(format!("restore profile failed: {err}"))
            })
        }
    }

    pub fn is_openless_profile_active() -> WindowsImeProfileResult<bool> {
        let active = capture_active_profile()?;
        Ok(active.kind == ImeProfileKind::TextService
            && active.clsid.as_deref() == Some(OPENLESS_TEXT_SERVICE_CLSID_BRACED)
            && active.profile_guid.as_deref() == Some(OPENLESS_PROFILE_GUID_BRACED))
    }
}
```

If `windows` crate signatures differ, adjust only the type adapters around `GetActiveProfile` and `ActivateProfile`; keep the public API and behavior unchanged.

- [ ] **Step 7: Run Windows type check**

Run:

```powershell
cargo check --manifest-path openless-all/app/src-tauri/Cargo.toml
```

Expected: backend type-checks on Windows.

- [ ] **Step 8: Run profile tests**

Run:

```powershell
cargo test --manifest-path openless-all/app/src-tauri/Cargo.toml windows_ime_profile --lib
```

Expected: pure profile tests pass; Windows identifier test passes.

- [ ] **Step 9: Commit**

```powershell
git add -- openless-all/app/src-tauri/Cargo.toml openless-all/app/src-tauri/src/windows_ime_profile.rs
git commit -m "feat: manage Windows TSF input profiles"
```

---

### Task 4: Rust Named-Pipe IME Server

**Files:**
- Create: `openless-all/app/src-tauri/src/windows_ime_ipc.rs`
- Modify: `openless-all/app/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing pending-submit tests**

Create `windows_ime_ipc.rs` with this test-first state logic:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows_ime_protocol::ImeSubmitStatus;

    #[test]
    fn pending_submit_accepts_only_matching_session() {
        let mut pending = PendingImeSubmit::new("session-1".to_string());
        assert!(pending.accept_result("session-2", ImeSubmitStatus::Committed).is_err());
        assert_eq!(
            pending.accept_result("session-1", ImeSubmitStatus::Committed),
            Ok(ImeSubmitStatus::Committed)
        );
    }

    #[test]
    fn pending_submit_rejects_second_result_after_completion() {
        let mut pending = PendingImeSubmit::new("session-1".to_string());
        assert_eq!(
            pending.accept_result("session-1", ImeSubmitStatus::Committed),
            Ok(ImeSubmitStatus::Committed)
        );
        assert!(pending.accept_result("session-1", ImeSubmitStatus::Committed).is_err());
    }
}
```

- [ ] **Step 2: Run the test and verify it fails because `PendingImeSubmit` is missing**

Run:

```powershell
cargo test --manifest-path openless-all/app/src-tauri/Cargo.toml windows_ime_ipc --lib
```

Expected: compile fails with missing `PendingImeSubmit`.

- [ ] **Step 3: Add pending-submit state**

Add this implementation above the tests:

```rust
use std::time::Duration;

use crate::windows_ime_protocol::ImeSubmitStatus;

pub const IME_CLIENT_WAIT_TIMEOUT: Duration = Duration::from_millis(700);
pub const IME_SUBMIT_TIMEOUT: Duration = Duration::from_millis(900);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowsImeIpcError {
    Unavailable(String),
    NoReadyClient,
    Timeout,
    Protocol(String),
    Io(String),
}

impl std::fmt::Display for WindowsImeIpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unavailable(message)
            | Self::Protocol(message)
            | Self::Io(message) => write!(f, "{message}"),
            Self::NoReadyClient => write!(f, "no OpenLess IME client is ready"),
            Self::Timeout => write!(f, "OpenLess IME IPC timed out"),
        }
    }
}

impl std::error::Error for WindowsImeIpcError {}

pub type WindowsImeIpcResult<T> = Result<T, WindowsImeIpcError>;

#[derive(Debug)]
pub struct PendingImeSubmit {
    session_id: String,
    completed: bool,
}

impl PendingImeSubmit {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            completed: false,
        }
    }

    pub fn accept_result(
        &mut self,
        session_id: &str,
        status: ImeSubmitStatus,
    ) -> WindowsImeIpcResult<ImeSubmitStatus> {
        if self.completed {
            return Err(WindowsImeIpcError::Protocol(
                "submit result arrived after completion".to_string(),
            ));
        }
        if self.session_id != session_id {
            return Err(WindowsImeIpcError::Protocol(
                "submit result belongs to a different session".to_string(),
            ));
        }
        self.completed = true;
        Ok(status)
    }
}
```

- [ ] **Step 4: Add public server API stubs**

Add this API below `PendingImeSubmit`:

```rust
#[derive(Debug, Clone)]
pub struct ImeSubmitRequest {
    pub session_id: String,
    pub text: String,
    pub created_at: String,
}

#[derive(Clone)]
pub struct WindowsImeIpcServer {
    inner: std::sync::Arc<parking_lot::Mutex<WindowsImeIpcState>>,
}

#[derive(Debug, Default)]
struct WindowsImeIpcState {
    ready_client_id: Option<String>,
}

impl WindowsImeIpcServer {
    pub fn new() -> Self {
        Self {
            inner: std::sync::Arc::new(parking_lot::Mutex::new(WindowsImeIpcState::default())),
        }
    }

    pub fn mark_client_ready_for_test(&self, client_id: String) {
        self.inner.lock().ready_client_id = Some(client_id);
    }

    pub fn has_ready_client(&self) -> bool {
        self.inner.lock().ready_client_id.is_some()
    }
}
```

- [ ] **Step 5: Add Windows async submit implementation**

Add a Windows-only `submit_text` implementation. Keep the non-Windows implementation as an immediate `Unavailable` error:

```rust
#[cfg(not(target_os = "windows"))]
impl WindowsImeIpcServer {
    pub async fn submit_text(
        &self,
        _request: ImeSubmitRequest,
    ) -> WindowsImeIpcResult<ImeSubmitStatus> {
        Err(WindowsImeIpcError::Unavailable(
            "Windows IME IPC is only available on Windows".to_string(),
        ))
    }
}

#[cfg(target_os = "windows")]
impl WindowsImeIpcServer {
    pub async fn submit_text(
        &self,
        request: ImeSubmitRequest,
    ) -> WindowsImeIpcResult<ImeSubmitStatus> {
        if !self.has_ready_client() {
            return Err(WindowsImeIpcError::NoReadyClient);
        }

        windows_pipe::submit_text_over_pipe(request).await
    }
}

#[cfg(target_os = "windows")]
mod windows_pipe {
    use super::*;
    use crate::windows_ime_protocol::{
        decode_message, encode_message, ImePipeMessage, OPENLESS_IME_PIPE_NAME,
        OPENLESS_IME_PROTOCOL_VERSION,
    };
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::windows::named_pipe::ClientOptions;

    pub async fn submit_text_over_pipe(
        request: ImeSubmitRequest,
    ) -> WindowsImeIpcResult<ImeSubmitStatus> {
        let client = ClientOptions::new()
            .open(OPENLESS_IME_PIPE_NAME)
            .map_err(|err| WindowsImeIpcError::Io(format!("open IME pipe failed: {err}")))?;
        let (reader, mut writer) = tokio::io::split(client);
        let mut reader = BufReader::new(reader);
        let submit = ImePipeMessage::SubmitText {
            protocol_version: OPENLESS_IME_PROTOCOL_VERSION,
            session_id: request.session_id.clone(),
            text: request.text,
            created_at: request.created_at,
        };
        let line = encode_message(&submit)
            .map_err(|err| WindowsImeIpcError::Protocol(format!("encode submit failed: {err}")))?;
        writer
            .write_all(line.as_bytes())
            .await
            .map_err(|err| WindowsImeIpcError::Io(format!("write submit failed: {err}")))?;
        writer
            .flush()
            .await
            .map_err(|err| WindowsImeIpcError::Io(format!("flush submit failed: {err}")))?;

        let mut response = String::new();
        let read = tokio::time::timeout(IME_SUBMIT_TIMEOUT, reader.read_line(&mut response))
            .await
            .map_err(|_| WindowsImeIpcError::Timeout)?
            .map_err(|err| WindowsImeIpcError::Io(format!("read submit result failed: {err}")))?;
        if read == 0 {
            return Err(WindowsImeIpcError::Io("IME pipe closed before result".to_string()));
        }

        match decode_message(response.trim_end())
            .map_err(|err| WindowsImeIpcError::Protocol(format!("decode result failed: {err}")))?
        {
            ImePipeMessage::SubmitResult {
                session_id,
                status,
                ..
            } if session_id == request.session_id => Ok(status),
            _ => Err(WindowsImeIpcError::Protocol(
                "unexpected IME submit result".to_string(),
            )),
        }
    }
}
```

This MVP opens the named pipe for each submit. The C++ IME DLL owns the pipe server because it is the active TSF instance inside the focused process.

- [ ] **Step 6: Register the module**

Add this to `lib.rs`:

```rust
mod windows_ime_ipc;
```

- [ ] **Step 7: Run tests**

Run:

```powershell
cargo test --manifest-path openless-all/app/src-tauri/Cargo.toml windows_ime_ipc --lib
```

Expected: pending-submit tests pass.

- [ ] **Step 8: Run type check**

Run:

```powershell
cargo check --manifest-path openless-all/app/src-tauri/Cargo.toml
```

Expected: backend type-checks.

- [ ] **Step 9: Commit**

```powershell
git add -- openless-all/app/src-tauri/src/windows_ime_ipc.rs openless-all/app/src-tauri/src/lib.rs
git commit -m "feat: add Windows IME IPC client"
```

---

### Task 5: Windows IME Session Guard and Fallback Routing

**Files:**
- Create: `openless-all/app/src-tauri/src/windows_ime_session.rs`
- Modify: `openless-all/app/src-tauri/src/insertion.rs`
- Modify: `openless-all/app/src-tauri/src/coordinator.rs`
- Modify: `openless-all/app/src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing routing tests**

Create `windows_ime_session.rs` with these tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::InsertStatus;
    use crate::windows_ime_protocol::ImeSubmitStatus;

    #[test]
    fn committed_ime_result_maps_to_inserted() {
        assert_eq!(
            map_ime_status_to_insert_status(ImeSubmitStatus::Committed),
            InsertStatus::Inserted
        );
    }

    #[test]
    fn rejected_ime_result_requests_fallback() {
        assert!(should_fallback_after_ime_result(ImeSubmitStatus::Rejected));
        assert!(should_fallback_after_ime_result(ImeSubmitStatus::Failed));
        assert!(!should_fallback_after_ime_result(ImeSubmitStatus::Committed));
    }
}
```

- [ ] **Step 2: Run the test and verify it fails because mapping functions are missing**

Run:

```powershell
cargo test --manifest-path openless-all/app/src-tauri/Cargo.toml windows_ime_session --lib
```

Expected: compile fails with missing mapping functions.

- [ ] **Step 3: Add mapping functions and session result types**

Add this implementation above the tests:

```rust
use crate::types::InsertStatus;
use crate::windows_ime_ipc::{ImeSubmitRequest, WindowsImeIpcServer};
use crate::windows_ime_profile::{ImeProfileSnapshot, WindowsImeProfileManager};
use crate::windows_ime_protocol::ImeSubmitStatus;

#[derive(Debug)]
pub enum WindowsImeSessionError {
    Profile(String),
    Ipc(String),
}

impl std::fmt::Display for WindowsImeSessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Profile(message) | Self::Ipc(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for WindowsImeSessionError {}

pub fn map_ime_status_to_insert_status(status: ImeSubmitStatus) -> InsertStatus {
    match status {
        ImeSubmitStatus::Committed => InsertStatus::Inserted,
        ImeSubmitStatus::Rejected | ImeSubmitStatus::Failed => InsertStatus::CopiedFallback,
    }
}

pub fn should_fallback_after_ime_result(status: ImeSubmitStatus) -> bool {
    !matches!(status, ImeSubmitStatus::Committed)
}

#[derive(Debug)]
pub struct PreparedWindowsImeSession {
    saved_profile: Option<ImeProfileSnapshot>,
    openless_activated: bool,
}

impl PreparedWindowsImeSession {
    pub fn unavailable() -> Self {
        Self {
            saved_profile: None,
            openless_activated: false,
        }
    }

    pub fn is_ready_for_tsf_submit(&self) -> bool {
        self.saved_profile.is_some() && self.openless_activated
    }
}
```

- [ ] **Step 4: Add Windows session controller**

Add this controller below `PreparedWindowsImeSession`:

```rust
pub struct WindowsImeSessionController {
    profile_manager: WindowsImeProfileManager,
    ipc: WindowsImeIpcServer,
}

impl WindowsImeSessionController {
    pub fn new() -> Self {
        Self {
            profile_manager: WindowsImeProfileManager::new(),
            ipc: WindowsImeIpcServer::new(),
        }
    }

    pub fn prepare_session(&self) -> PreparedWindowsImeSession {
        #[cfg(not(target_os = "windows"))]
        {
            PreparedWindowsImeSession::unavailable()
        }

        #[cfg(target_os = "windows")]
        {
            let saved_profile = match self.profile_manager.capture_active_profile() {
                Ok(snapshot) => Some(snapshot),
                Err(err) => {
                    log::warn!("[windows-ime] capture active profile failed: {err}");
                    None
                }
            };
            if saved_profile.is_none() {
                return PreparedWindowsImeSession::unavailable();
            }
            match self.profile_manager.activate_openless_profile() {
                Ok(()) => PreparedWindowsImeSession {
                    saved_profile,
                    openless_activated: true,
                },
                Err(err) => {
                    log::warn!("[windows-ime] activate OpenLess profile failed: {err}");
                    PreparedWindowsImeSession::unavailable()
                }
            }
        }
    }

    pub async fn submit_prepared(
        &self,
        prepared: &PreparedWindowsImeSession,
        request: ImeSubmitRequest,
    ) -> Result<InsertStatus, WindowsImeSessionError> {
        if !prepared.is_ready_for_tsf_submit() {
            return Err(WindowsImeSessionError::Ipc(
                "OpenLess IME session is not active".to_string(),
            ));
        }
        let status = self
            .ipc
            .submit_text(request)
            .await
            .map_err(|err| WindowsImeSessionError::Ipc(err.to_string()))?;
        Ok(map_ime_status_to_insert_status(status))
    }

    pub fn restore_session(&self, prepared: PreparedWindowsImeSession) {
        let Some(saved_profile) = prepared.saved_profile else {
            return;
        };
        match self.profile_manager.is_openless_profile_active() {
            Ok(true) => {
                if let Err(err) = self.profile_manager.restore_profile(&saved_profile) {
                    log::warn!("[windows-ime] restore previous profile failed: {err}");
                }
            }
            Ok(false) => {}
            Err(err) => log::warn!("[windows-ime] profile active check failed: {err}"),
        }
    }
}
```

- [ ] **Step 5: Register the module**

Add this to `lib.rs`:

```rust
mod windows_ime_session;
```

- [ ] **Step 6: Expose a fallback-only insertion method**

In `insertion.rs`, keep current behavior but rename the Windows/Linux helper intent by adding this method to `impl TextInserter` under `#[cfg(not(target_os = "macos"))]`:

```rust
#[cfg(not(target_os = "macos"))]
pub fn insert_via_clipboard_fallback(
    &self,
    text: &str,
    restore_clipboard_after_paste: bool,
) -> InsertStatus {
    self.insert(text, restore_clipboard_after_paste)
}
```

- [ ] **Step 7: Wire the controller into coordinator state**

In `coordinator.rs`, add the controller and prepared session field near the existing `inserter` field:

```rust
#[cfg(target_os = "windows")]
use crate::windows_ime_session::{PreparedWindowsImeSession, WindowsImeSessionController};
```

Add fields to the coordinator inner state:

```rust
#[cfg(target_os = "windows")]
windows_ime: WindowsImeSessionController,
#[cfg(target_os = "windows")]
prepared_windows_ime_session: Arc<Mutex<Option<PreparedWindowsImeSession>>>,
```

Initialize them where `TextInserter::new()` is initialized:

```rust
#[cfg(target_os = "windows")]
windows_ime: WindowsImeSessionController::new(),
#[cfg(target_os = "windows")]
prepared_windows_ime_session: Arc::new(Mutex::new(None)),
```

- [ ] **Step 8: Prepare TSF session when recording starts**

In the recording-start path, immediately after the coordinator accepts the hotkey edge and before recorder start, add:

```rust
#[cfg(target_os = "windows")]
{
    let prepared = inner.windows_ime.prepare_session();
    *inner.prepared_windows_ime_session.lock() = Some(prepared);
}
```

This code belongs in the same start-session branch that changes phase from `Idle` to `Starting`.

- [ ] **Step 9: Submit through TSF first in `end_session`**

Replace the direct insertion call:

```rust
let status = inner.inserter.insert(&polished, restore_clipboard);
```

with Windows-first routing:

```rust
#[cfg(target_os = "windows")]
let status = {
    let prepared = inner.prepared_windows_ime_session.lock().take();
    if let Some(prepared) = prepared {
        let request = crate::windows_ime_ipc::ImeSubmitRequest {
            session_id: Uuid::new_v4().to_string(),
            text: polished.clone(),
            created_at: Utc::now().to_rfc3339(),
        };
        let tsf_status = inner.windows_ime.submit_prepared(&prepared, request).await;
        inner.windows_ime.restore_session(prepared);
        match tsf_status {
            Ok(InsertStatus::Inserted) => InsertStatus::Inserted,
            Ok(_) | Err(_) => inner
                .inserter
                .insert_via_clipboard_fallback(&polished, restore_clipboard),
        }
    } else {
        inner
            .inserter
            .insert_via_clipboard_fallback(&polished, restore_clipboard)
    }
};

#[cfg(not(target_os = "windows"))]
let status = inner.inserter.insert(&polished, restore_clipboard);
```

- [ ] **Step 10: Restore on cancellation**

In the cancellation path that handles active `Starting`, `Listening`, or `Processing` sessions, add:

```rust
#[cfg(target_os = "windows")]
if let Some(prepared) = inner.prepared_windows_ime_session.lock().take() {
    inner.windows_ime.restore_session(prepared);
}
```

Place it before returning the session to `Idle`.

- [ ] **Step 11: Run focused tests**

Run:

```powershell
cargo test --manifest-path openless-all/app/src-tauri/Cargo.toml windows_ime_session --lib
```

Expected: routing tests pass.

- [ ] **Step 12: Run backend type check**

Run:

```powershell
cargo check --manifest-path openless-all/app/src-tauri/Cargo.toml
```

Expected: backend type-checks.

- [ ] **Step 13: Commit**

```powershell
git add -- openless-all/app/src-tauri/src/windows_ime_session.rs openless-all/app/src-tauri/src/insertion.rs openless-all/app/src-tauri/src/coordinator.rs openless-all/app/src-tauri/src/lib.rs
git commit -m "feat: route Windows insertion through temporary TSF IME"
```

---

### Task 6: C++ TSF DLL Project Skeleton

**Files:**
- Create: `openless-all/app/windows-ime/OpenLessIme.sln`
- Create: `openless-all/app/windows-ime/OpenLessIme.vcxproj`
- Create: `openless-all/app/windows-ime/src/guids.h`
- Create: `openless-all/app/windows-ime/src/dllmain.cpp`
- Create: `openless-all/app/windows-ime/src/class_factory.h`
- Create: `openless-all/app/windows-ime/src/class_factory.cpp`
- Create: `openless-all/app/windows-ime/src/text_service.h`
- Create: `openless-all/app/windows-ime/src/text_service.cpp`
- Create: `openless-all/app/windows-ime/src/registry.h`
- Create: `openless-all/app/windows-ime/src/registry.cpp`
- Create: `openless-all/app/windows-ime/src/resource.rc`

- [ ] **Step 1: Create the C++ project files**

Create a Visual Studio DLL project that builds `OpenLessIme.dll` for x64 with C++17 and the Windows SDK. The `.vcxproj` must include:

```xml
<ConfigurationType>DynamicLibrary</ConfigurationType>
<CharacterSet>Unicode</CharacterSet>
<LanguageStandard>stdcpp17</LanguageStandard>
<AdditionalDependencies>msctf.lib;ole32.lib;uuid.lib;advapi32.lib;%(AdditionalDependencies)</AdditionalDependencies>
```

Include every `src/*.cpp`, `src/*.h`, and `src/resource.rc` file listed in this task.

- [ ] **Step 2: Add fixed GUID constants**

Create `src/guids.h`:

```cpp
#pragma once

#include <guiddef.h>

// {6B9F3F4F-5EE7-42D6-9C61-9F80B03A5D7D}
inline constexpr GUID CLSID_OpenLessTextService = {
    0x6b9f3f4f,
    0x5ee7,
    0x42d6,
    {0x9c, 0x61, 0x9f, 0x80, 0xb0, 0x3a, 0x5d, 0x7d}};

// {9B5F5E04-23F6-47DA-9A26-D221F6C3F02E}
inline constexpr GUID GUID_OpenLessProfile = {
    0x9b5f5e04,
    0x23f6,
    0x47da,
    {0x9a, 0x26, 0xd2, 0x21, 0xf6, 0xc3, 0xf0, 0x2e}};

inline constexpr wchar_t kOpenLessImeName[] = L"OpenLess Voice Input";
inline constexpr LANGID kOpenLessLangId = 0x0804;
```

- [ ] **Step 3: Add DLL exports and module lifetime**

Create `src/dllmain.cpp` with exports:

```cpp
#include <windows.h>
#include "class_factory.h"
#include "registry.h"
#include "guids.h"

HINSTANCE g_module = nullptr;
long g_lock_count = 0;
long g_object_count = 0;

BOOL APIENTRY DllMain(HINSTANCE module, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        g_module = module;
        DisableThreadLibraryCalls(module);
    }
    return TRUE;
}

STDAPI DllCanUnloadNow() {
    return (g_lock_count == 0 && g_object_count == 0) ? S_OK : S_FALSE;
}

STDAPI DllGetClassObject(REFCLSID clsid, REFIID iid, void** result) {
    if (!result) {
        return E_POINTER;
    }
    *result = nullptr;
    if (clsid != CLSID_OpenLessTextService) {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    auto* factory = new (std::nothrow) OpenLessClassFactory();
    if (!factory) {
        return E_OUTOFMEMORY;
    }
    const HRESULT hr = factory->QueryInterface(iid, result);
    factory->Release();
    return hr;
}

STDAPI DllRegisterServer() {
    return RegisterOpenLessTextService(g_module);
}

STDAPI DllUnregisterServer() {
    return UnregisterOpenLessTextService();
}
```

- [ ] **Step 4: Add class factory**

Create `class_factory.h/.cpp` implementing `IClassFactory`. It must:

- Support `IUnknown` and `IClassFactory`.
- Increment `g_object_count` on construction and decrement it on destruction.
- `CreateInstance` returns a new `OpenLessTextService`.
- `LockServer` increments/decrements `g_lock_count`.

Use this `CreateInstance` body:

```cpp
HRESULT OpenLessClassFactory::CreateInstance(IUnknown* outer, REFIID iid, void** result) {
    if (!result) {
        return E_POINTER;
    }
    *result = nullptr;
    if (outer) {
        return CLASS_E_NOAGGREGATION;
    }
    auto* service = new (std::nothrow) OpenLessTextService();
    if (!service) {
        return E_OUTOFMEMORY;
    }
    const HRESULT hr = service->QueryInterface(iid, result);
    service->Release();
    return hr;
}
```

- [ ] **Step 5: Add minimal text service class**

Create `text_service.h/.cpp` implementing `ITfTextInputProcessorEx`. It must:

- Support `IUnknown`, `ITfTextInputProcessor`, and `ITfTextInputProcessorEx`.
- Store `ITfThreadMgr* thread_mgr_` and `TfClientId client_id_`.
- `ActivateEx` stores the thread manager and client id, starts the IPC server thread, and returns `S_OK`.
- `Deactivate` stops the IPC server thread, releases the thread manager, clears client id, and returns `S_OK`.

Use this method shape:

```cpp
HRESULT OpenLessTextService::ActivateEx(ITfThreadMgr* thread_mgr, TfClientId client_id, DWORD) {
    if (!thread_mgr) {
        return E_INVALIDARG;
    }
    thread_mgr_ = thread_mgr;
    thread_mgr_->AddRef();
    client_id_ = client_id;
    ipc_client_.Start(this);
    return S_OK;
}

HRESULT OpenLessTextService::Deactivate() {
    ipc_client_.Stop();
    if (thread_mgr_) {
        thread_mgr_->Release();
        thread_mgr_ = nullptr;
    }
    client_id_ = TF_CLIENTID_NULL;
    return S_OK;
}
```

Add a method used by the IPC client:

```cpp
HRESULT OpenLessTextService::SubmitTextFromPipe(const std::wstring& session_id,
                                                const std::wstring& text);
```

For this task, return `E_NOTIMPL` from `SubmitTextFromPipe`; Task 7 replaces it with real edit-session submission.

- [ ] **Step 6: Add COM and TSF registration code**

Create `registry.h/.cpp` with:

```cpp
HRESULT RegisterOpenLessTextService(HINSTANCE module);
HRESULT UnregisterOpenLessTextService();
```

`RegisterOpenLessTextService` must:

- Write HKCU COM registration under `Software\Classes\CLSID\{6B9F3F4F-5EE7-42D6-9C61-9F80B03A5D7D}`.
- Set `InprocServer32` default value to the DLL path.
- Set `ThreadingModel` to `Apartment`.
- Create `ITfInputProcessorProfiles`.
- Call `Register(CLSID_OpenLessTextService)`.
- Call `AddLanguageProfile(CLSID_OpenLessTextService, 0x0804, GUID_OpenLessProfile, L"OpenLess Voice Input", ...)`.
- Call `EnableLanguageProfile(CLSID_OpenLessTextService, 0x0804, GUID_OpenLessProfile, TRUE)`.

`UnregisterOpenLessTextService` must call `Unregister(CLSID_OpenLessTextService)` and remove the HKCU COM registration key.

- [ ] **Step 7: Build the DLL**

Run from a Developer PowerShell:

```powershell
MSBuild openless-all/app/windows-ime/OpenLessIme.sln /p:Configuration=Release /p:Platform=x64
```

Expected: `openless-all/app/windows-ime/x64/Release/OpenLessIme.dll` exists.

- [ ] **Step 8: Commit**

```powershell
git add -- openless-all/app/windows-ime
git commit -m "feat: scaffold OpenLess TSF IME DLL"
```

---

### Task 7: TSF Edit Session Text Commit

**Files:**
- Create: `openless-all/app/windows-ime/src/edit_session.h`
- Create: `openless-all/app/windows-ime/src/edit_session.cpp`
- Modify: `openless-all/app/windows-ime/src/text_service.h`
- Modify: `openless-all/app/windows-ime/src/text_service.cpp`

- [ ] **Step 1: Add edit session class**

Create `edit_session.h/.cpp` implementing `ITfEditSession`:

```cpp
class OpenLessEditSession final : public ITfEditSession {
public:
    OpenLessEditSession(ITfContext* context, std::wstring text);

    STDMETHODIMP QueryInterface(REFIID iid, void** result) override;
    STDMETHODIMP_(ULONG) AddRef() override;
    STDMETHODIMP_(ULONG) Release() override;
    STDMETHODIMP DoEditSession(TfEditCookie edit_cookie) override;

private:
    ~OpenLessEditSession() = default;

    long ref_count_ = 1;
    ITfContext* context_ = nullptr;
    std::wstring text_;
};
```

`DoEditSession` must query `ITfInsertAtSelection` from the context and call:

```cpp
insert_at_selection->InsertTextAtSelection(
    edit_cookie,
    TF_IAS_QUERYONLY,
    text_.c_str(),
    static_cast<LONG>(text_.size()),
    nullptr);
```

Then call the same method without `TF_IAS_QUERYONLY` to commit text:

```cpp
insert_at_selection->InsertTextAtSelection(
    edit_cookie,
    0,
    text_.c_str(),
    static_cast<LONG>(text_.size()),
    nullptr);
```

Return the HRESULT from the committing call. Release every COM pointer acquired in the method.

- [ ] **Step 2: Replace `SubmitTextFromPipe` with real TSF submission**

In `text_service.cpp`, implement:

```cpp
HRESULT OpenLessTextService::SubmitTextFromPipe(const std::wstring&,
                                                const std::wstring& text) {
    if (!thread_mgr_ || client_id_ == TF_CLIENTID_NULL) {
        return E_UNEXPECTED;
    }

    ITfDocumentMgr* document_mgr = nullptr;
    HRESULT hr = thread_mgr_->GetFocus(&document_mgr);
    if (FAILED(hr) || !document_mgr) {
        return FAILED(hr) ? hr : E_FAIL;
    }

    ITfContext* context = nullptr;
    hr = document_mgr->GetTop(&context);
    document_mgr->Release();
    if (FAILED(hr) || !context) {
        return FAILED(hr) ? hr : E_FAIL;
    }

    auto* session = new (std::nothrow) OpenLessEditSession(context, text);
    if (!session) {
        context->Release();
        return E_OUTOFMEMORY;
    }

    HRESULT edit_result = E_FAIL;
    hr = context->RequestEditSession(
        client_id_,
        session,
        TF_ES_SYNC | TF_ES_READWRITE,
        &edit_result);
    session->Release();
    context->Release();
    if (FAILED(hr)) {
        return hr;
    }
    return edit_result;
}
```

- [ ] **Step 3: Build the DLL**

Run:

```powershell
MSBuild openless-all/app/windows-ime/OpenLessIme.sln /p:Configuration=Release /p:Platform=x64
```

Expected: build succeeds.

- [ ] **Step 4: Commit**

```powershell
git add -- openless-all/app/windows-ime/src/edit_session.h openless-all/app/windows-ime/src/edit_session.cpp openless-all/app/windows-ime/src/text_service.h openless-all/app/windows-ime/src/text_service.cpp
git commit -m "feat: commit dictated text through TSF edit sessions"
```

---

### Task 8: C++ Named-Pipe Server in the IME DLL

**Files:**
- Create: `openless-all/app/windows-ime/src/ipc_client.h`
- Create: `openless-all/app/windows-ime/src/ipc_client.cpp`
- Modify: `openless-all/app/windows-ime/src/text_service.h`
- Modify: `openless-all/app/windows-ime/src/text_service.cpp`

- [ ] **Step 1: Add IPC server class**

Create `ipc_client.h` with:

```cpp
class OpenLessTextService;

class OpenLessPipeServer {
public:
    OpenLessPipeServer();
    ~OpenLessPipeServer();

    void Start(OpenLessTextService* service);
    void Stop();

private:
    void Run();
    HRESULT HandleSubmitLine(const std::wstring& line);
    bool WriteResult(const std::wstring& session_id, const wchar_t* status, const wchar_t* error_code);

    std::atomic<bool> stop_requested_{false};
    std::thread thread_;
    OpenLessTextService* service_ = nullptr;
};
```

- [ ] **Step 2: Implement one-submit-at-a-time JSONL handling**

Create `ipc_client.cpp` using Windows named pipes:

- Pipe name: `\\.\pipe\OpenLessImeSubmit`
- Pipe mode: message pipe, byte read mode, blocking wait.
- Accept one client at a time.
- Read one UTF-8 JSON line.
- Extract `type`, `sessionId`, and `text`.
- Reject messages whose `type` is not `submitText`.
- Convert `text` from UTF-8 to UTF-16.
- Call `service_->SubmitTextFromPipe(session_id, text)`.
- Write one JSONL `submitResult` response with `committed`, `rejected`, or `failed`.

Use a small local parser limited to the protocol keys:

```cpp
std::wstring ExtractJsonStringField(const std::wstring& json, const wchar_t* field_name);
```

The parser only needs to handle JSON emitted by Rust `serde_json` for this protocol. It must reject missing fields and return `failed` with `protocolError`.

- [ ] **Step 3: Start and stop pipe server from the text service**

In `OpenLessTextService::ActivateEx`, call:

```cpp
pipe_server_.Start(this);
```

In `OpenLessTextService::Deactivate`, call:

```cpp
pipe_server_.Stop();
```

Store `OpenLessPipeServer pipe_server_;` as a member of `OpenLessTextService`.

- [ ] **Step 4: Build the DLL**

Run:

```powershell
MSBuild openless-all/app/windows-ime/OpenLessIme.sln /p:Configuration=Release /p:Platform=x64
```

Expected: build succeeds.

- [ ] **Step 5: Commit**

```powershell
git add -- openless-all/app/windows-ime/src/ipc_client.h openless-all/app/windows-ime/src/ipc_client.cpp openless-all/app/windows-ime/src/text_service.h openless-all/app/windows-ime/src/text_service.cpp
git commit -m "feat: receive OpenLess IME submissions over a named pipe"
```

---

### Task 9: Registration and Build Scripts

**Files:**
- Create: `openless-all/app/scripts/windows-ime-build.ps1`
- Create: `openless-all/app/scripts/windows-ime-register.ps1`
- Create: `openless-all/app/scripts/windows-ime-unregister.ps1`
- Modify: `openless-all/app/scripts/windows-preflight.ps1`

- [ ] **Step 1: Add build script**

Create `windows-ime-build.ps1`:

```powershell
param(
  [ValidateSet("Debug", "Release")]
  [string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"
$appRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$solution = Join-Path $appRoot "windows-ime\OpenLessIme.sln"

$msbuild = Get-Command MSBuild.exe -ErrorAction SilentlyContinue
if (-not $msbuild) {
  throw "MSBuild.exe not found. Run from Developer PowerShell or install Visual Studio Build Tools with Desktop development with C++."
}

& $msbuild.Source $solution /p:Configuration=$Configuration /p:Platform=x64
if ($LASTEXITCODE -ne 0) {
  throw "OpenLessIme build failed with exit code $LASTEXITCODE"
}

$dll = Join-Path $appRoot "windows-ime\x64\$Configuration\OpenLessIme.dll"
if (-not (Test-Path $dll)) {
  throw "OpenLessIme.dll was not produced at $dll"
}

Write-Host "[ok] $dll"
```

- [ ] **Step 2: Add register script**

Create `windows-ime-register.ps1`:

```powershell
param(
  [ValidateSet("Debug", "Release")]
  [string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"
$appRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$dll = Join-Path $appRoot "windows-ime\x64\$Configuration\OpenLessIme.dll"

if (-not (Test-Path $dll)) {
  & (Join-Path $PSScriptRoot "windows-ime-build.ps1") -Configuration $Configuration
}

$regsvr32 = Join-Path $env:WINDIR "System32\regsvr32.exe"
& $regsvr32 /s $dll
if ($LASTEXITCODE -ne 0) {
  throw "regsvr32 failed with exit code $LASTEXITCODE"
}

Write-Host "[ok] OpenLess TSF IME registered for current user"
```

- [ ] **Step 3: Add unregister script**

Create `windows-ime-unregister.ps1`:

```powershell
param(
  [ValidateSet("Debug", "Release")]
  [string]$Configuration = "Release"
)

$ErrorActionPreference = "Stop"
$appRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$dll = Join-Path $appRoot "windows-ime\x64\$Configuration\OpenLessIme.dll"

if (-not (Test-Path $dll)) {
  Write-Host "[skip] OpenLessIme.dll not found at $dll"
  exit 0
}

$regsvr32 = Join-Path $env:WINDIR "System32\regsvr32.exe"
& $regsvr32 /u /s $dll
if ($LASTEXITCODE -ne 0) {
  throw "regsvr32 /u failed with exit code $LASTEXITCODE"
}

Write-Host "[ok] OpenLess TSF IME unregistered"
```

- [ ] **Step 4: Extend preflight**

In `windows-preflight.ps1`, add an `ime` option to the `ValidateSet` and check:

```powershell
if ($Toolchain -eq "all" -or $Toolchain -eq "msvc" -or $Toolchain -eq "ime") {
  Write-Host ""
  Write-Host "== Windows IME route =="
  if (-not (Test-Command "MSBuild.exe")) {
    Write-Host "[hint] Install Visual Studio Build Tools and run from Developer PowerShell."
    $failed = $true
  }
  $msctf = Get-ChildItem -LiteralPath (Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\Lib") -Recurse -Filter msctf.lib -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match "\\um\\x64\\msctf\.lib$" } |
    Select-Object -First 1
  if ($msctf) {
    Write-Host "[ok] msctf.lib -> $($msctf.FullName)"
  } else {
    Write-Host "[missing] msctf.lib"
    $failed = $true
  }
}
```

- [ ] **Step 5: Run scripts**

Run:

```powershell
.\openless-all\app\scripts\windows-preflight.ps1 -Toolchain ime
.\openless-all\app\scripts\windows-ime-build.ps1
```

Expected: preflight passes and the IME DLL builds.

- [ ] **Step 6: Commit**

```powershell
git add -- openless-all/app/scripts/windows-ime-build.ps1 openless-all/app/scripts/windows-ime-register.ps1 openless-all/app/scripts/windows-ime-unregister.ps1 openless-all/app/scripts/windows-preflight.ps1
git commit -m "feat: add Windows IME build and registration scripts"
```

---

### Task 10: Tauri Commands and Settings Status

**Files:**
- Modify: `openless-all/app/src-tauri/src/types.rs`
- Modify: `openless-all/app/src-tauri/src/commands.rs`
- Modify: `openless-all/app/src-tauri/src/lib.rs`
- Modify: `openless-all/app/src/lib/types.ts`
- Modify: `openless-all/app/src/lib/ipc.ts`
- Modify: `openless-all/app/src/i18n/zh-CN.ts`
- Modify: `openless-all/app/src/i18n/en.ts`
- Modify: `openless-all/app/src/pages/Settings.tsx`

- [ ] **Step 1: Add backend status types**

In `types.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WindowsImeInstallState {
    NotWindows,
    NotInstalled,
    Installed,
    RegistrationBroken,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WindowsImeStatus {
    pub state: WindowsImeInstallState,
    pub using_tsf_backend: bool,
    pub message: Option<String>,
}
```

- [ ] **Step 2: Add status command**

In `commands.rs`, add:

```rust
#[tauri::command]
pub fn get_windows_ime_status() -> WindowsImeStatus {
    #[cfg(not(target_os = "windows"))]
    {
        WindowsImeStatus {
            state: WindowsImeInstallState::NotWindows,
            using_tsf_backend: false,
            message: Some("Windows TSF IME is only available on Windows.".to_string()),
        }
    }

    #[cfg(target_os = "windows")]
    {
        match crate::windows_ime_profile::WindowsImeProfileManager::new()
            .is_openless_profile_active()
        {
            Ok(_) => WindowsImeStatus {
                state: WindowsImeInstallState::Installed,
                using_tsf_backend: true,
                message: None,
            },
            Err(err) => WindowsImeStatus {
                state: WindowsImeInstallState::NotInstalled,
                using_tsf_backend: false,
                message: Some(err.to_string()),
            },
        }
    }
}
```

Use this as a health signal only; active-profile false is not a failure because OpenLess should be active only during voice sessions.

- [ ] **Step 3: Register command**

Add `get_windows_ime_status` to the Tauri `invoke_handler!` list in `lib.rs`.

- [ ] **Step 4: Add frontend types and IPC wrapper**

In `src/lib/types.ts`:

```ts
export type WindowsImeInstallState =
  | 'notWindows'
  | 'notInstalled'
  | 'installed'
  | 'registrationBroken';

export interface WindowsImeStatus {
  state: WindowsImeInstallState;
  usingTsfBackend: boolean;
  message?: string | null;
}
```

In `src/lib/ipc.ts`:

```ts
export async function getWindowsImeStatus(): Promise<WindowsImeStatus> {
  if (isBrowserDev()) {
    return {
      state: 'notWindows',
      usingTsfBackend: false,
      message: 'Browser dev mock',
    };
  }
  return invoke<WindowsImeStatus>('get_windows_ime_status');
}
```

- [ ] **Step 5: Add Settings UI row**

In `Settings.tsx`, add a Windows-only status row using existing UI atoms. Text keys:

Chinese source:

```ts
windowsImeTitle: 'Windows 输入法后端',
windowsImeInstalled: '已安装，语音输入会临时切换到 OpenLess 输入法',
windowsImeNotInstalled: '未安装，当前使用剪贴板/WM_PASTE 回退',
windowsImeRegistrationBroken: '注册异常，请重新安装 OpenLess 输入法',
windowsImeNotWindows: '仅 Windows 可用',
```

English:

```ts
windowsImeTitle: 'Windows input method backend',
windowsImeInstalled: 'Installed. Voice input temporarily switches to the OpenLess IME.',
windowsImeNotInstalled: 'Not installed. OpenLess is using the clipboard/WM_PASTE fallback.',
windowsImeRegistrationBroken: 'Registration is broken. Reinstall the OpenLess IME.',
windowsImeNotWindows: 'Only available on Windows.',
```

- [ ] **Step 6: Run frontend build**

Run:

```powershell
cd openless-all/app
npm run build
```

Expected: TypeScript and Vite build succeed.

- [ ] **Step 7: Run backend type check**

Run:

```powershell
cargo check --manifest-path openless-all/app/src-tauri/Cargo.toml
```

Expected: backend type-checks.

- [ ] **Step 8: Commit**

```powershell
git add -- openless-all/app/src-tauri/src/types.rs openless-all/app/src-tauri/src/commands.rs openless-all/app/src-tauri/src/lib.rs openless-all/app/src/lib/types.ts openless-all/app/src/lib/ipc.ts openless-all/app/src/i18n/zh-CN.ts openless-all/app/src/i18n/en.ts openless-all/app/src/pages/Settings.tsx
git commit -m "feat: show Windows TSF IME backend status"
```

---

### Task 11: End-to-End Windows Verification

**Files:**
- Modify only files needed to fix defects found during verification.

- [ ] **Step 1: Run full automated checks**

Run:

```powershell
cargo test --manifest-path openless-all/app/src-tauri/Cargo.toml --lib
cargo check --manifest-path openless-all/app/src-tauri/Cargo.toml
cd openless-all/app
npm run build
.\scripts\windows-ime-build.ps1
```

Expected:

- Rust tests pass.
- Rust backend type-checks.
- Frontend build succeeds.
- `OpenLessIme.dll` builds.

- [ ] **Step 2: Register the IME**

Run:

```powershell
.\openless-all\app\scripts\windows-ime-register.ps1
```

Expected: script prints `[ok] OpenLess TSF IME registered for current user`.

- [ ] **Step 3: Manual Notepad verification**

1. Open Notepad.
2. Switch to Microsoft Pinyin.
3. Start OpenLess.
4. Press the configured voice hotkey to start recording.
5. Speak a short phrase.
6. Press the configured voice hotkey again to finish.

Expected:

- Input indicator briefly switches to OpenLess during the voice session.
- Final text appears at the Notepad caret.
- Input indicator returns to Microsoft Pinyin.
- Clipboard content is unchanged when TSF commit succeeds.

- [ ] **Step 4: Manual browser verification**

Repeat Step 3 in a browser text field.

Expected: text appears in the focused browser field and input profile restores.

- [ ] **Step 5: Manual VS Code verification**

Repeat Step 3 in a VS Code editor tab.

Expected: text appears at the editor caret and input profile restores.

- [ ] **Step 6: Cancellation verification**

1. Open Notepad with Microsoft Pinyin active.
2. Press the OpenLess voice hotkey to start.
3. Cancel during recording or processing using the existing cancel path.

Expected:

- No text is inserted.
- Input profile returns to Microsoft Pinyin.
- Clipboard content is unchanged.

- [ ] **Step 7: Fallback verification**

Unregister the IME:

```powershell
.\openless-all\app\scripts\windows-ime-unregister.ps1
```

Run a normal voice session in Notepad.

Expected:

- Voice input still inserts through the existing Windows fallback path.
- Settings reports the TSF backend as not installed.
- User text is not lost.

- [ ] **Step 8: Final verification review**

Run:

```powershell
git status --short
git diff -- openless-all/app/src-tauri openless-all/app/windows-ime openless-all/app/scripts openless-all/app/src docs/superpowers/plans/2026-05-01-windows-temporary-tsf-ime.md
```

Expected: every remaining diff is tied to the TSF IME implementation or a verification fix discovered in this task. If no code changed during verification, leave the branch without an extra commit. If verification changed code, stage the exact files shown by `git status --short` that are tied to this TSF IME work and commit with:

```powershell
git commit -m "fix: harden Windows TSF IME verification path"
```

---

## Self-Review Checklist

- The plan covers TSF profile activation, final-text IPC, TSF edit-session commit, restore on success/failure/cancel, fallback behavior, settings status, registration scripts, and manual verification.
- The plan keeps ASR, polish, recorder, and UI ownership in the Tauri/Rust app.
- The plan keeps third-party Chinese IME behavior by restoring the user's previous input profile after each voice session.
- The plan preserves the existing Windows `WM_PASTE` fallback.
- The plan avoids putting network, ASR, LLM, or Tauri UI inside the IME DLL.
