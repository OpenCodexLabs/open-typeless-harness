# Windows terminal clipboard restore investigation (2026-05-02)

Scope: `openless-all/app/src-tauri/src/insertion.rs`

## Problem statement

On Windows terminal-style text entry, OpenLess could:

1. put the new dictated text into the clipboard
2. send `Ctrl+V`
3. restore the old clipboard too early
4. let the terminal paste the old clipboard instead of the dictated text

## Baseline code path

- `Coordinator::end_session()` treats Windows synthetic paste as `InsertStatus::PasteSent`, not `Inserted`.
- `TextInserter::insert()` calls `insert_with_clipboard_restore()`.
- Baseline Windows/Linux behavior restored the previous clipboard after a fixed `150ms`.
- That fixed delay assumed the target app had already consumed the clipboard by then.

## Automated evidence

### 1. GUI automation boundary in this session

Commands used:

```powershell
Start-Process notepad.exe -PassThru
Start-Process cmd.exe -PassThru
EnumWindows(...)
```

Observed result:

- `explorer.exe` exists in `SessionId=1`
- newly started `notepad.exe`, `cmd.exe`, and even a local WinForms probe form did not expose enumerable top-level windows in this thread

Conclusion:

- this Codex desktop thread can compile and manipulate the Windows clipboard
- it cannot reliably drive newly created GUI windows in the current desktop context
- therefore the strongest fully automated evidence in this session must come from clipboard-timing experiments, not end-to-end GUI paste readback

### 2. Clipboard timing matrix

Script:

- `openless-all/app/scripts/windows-clipboard-consumer-timing-smoke.ps1`

Command:

```powershell
$cases = @(
  @{ consumer = 50; restore = 150 },
  @{ consumer = 250; restore = 150 },
  @{ consumer = 250; restore = 750 }
)
foreach ($case in $cases) {
  powershell -ExecutionPolicy Bypass -File openless-all/app/scripts/windows-clipboard-consumer-timing-smoke.ps1 -ConsumerDelayMs $case.consumer -RestoreDelayMs $case.restore
}
```

Observed outputs:

```json
{"consumerDelayMs":50,"restoreDelayMs":150,"insertedText":"OPENLESS_DICTATED_TEXT","previousText":"OPENLESS_OLDER_CLIPBOARD","observedText":"OPENLESS_DICTATED_TEXT","matchedInserted":true}
{"consumerDelayMs":250,"restoreDelayMs":150,"insertedText":"OPENLESS_DICTATED_TEXT","previousText":"OPENLESS_OLDER_CLIPBOARD","observedText":"OPENLESS_OLDER_CLIPBOARD","matchedInserted":false}
{"consumerDelayMs":250,"restoreDelayMs":750,"insertedText":"OPENLESS_DICTATED_TEXT","previousText":"OPENLESS_OLDER_CLIPBOARD","observedText":"OPENLESS_DICTATED_TEXT","matchedInserted":true}
```

Interpretation:

- a fast consumer (`50ms`) succeeds with the old `150ms` restore window
- a slower consumer (`250ms`) fails with the old `150ms` restore window
- the same slower consumer succeeds once restore is delayed to `750ms`

This isolates the bug to clipboard restore timing, independent of ASR, polish, QA hotkey, or selection logic.

### 3. Real app end-to-end regression in a stable desktop automation stack

Environment:

- Python `pywinauto` + `pywin32`
- Real desktop windows, not mock controls
- Targets:
  - Windows Terminal `cmd.exe` tab
  - Windows Terminal `PowerShell` tab
  - Notepad

Method:

- Put a command or text payload into the real Windows clipboard
- Send synthetic `Ctrl+V`
- Wait either `150ms` or `750ms`
- Restore the previous clipboard
- Verify the target app actually received the intended payload

Observed outputs:

```json
[
  {
    "target": "Windows Terminal CMD",
    "restoreDelayMs": 150,
    "expected": "CMD_150_OK",
    "succeeded": true
  },
  {
    "target": "Windows Terminal CMD",
    "restoreDelayMs": 750,
    "expected": "CMD_750_OK",
    "succeeded": true
  },
  {
    "target": "Windows Terminal PowerShell",
    "restoreDelayMs": 150,
    "expected": "POWERSHELL_150_OK",
    "succeeded": true
  },
  {
    "target": "Windows Terminal PowerShell",
    "restoreDelayMs": 750,
    "expected": "POWERSHELL_750_OK",
    "succeeded": true
  },
  {
    "target": "Notepad",
    "restoreDelayMs": 150,
    "expected": "NOTEPAD_150_OK",
    "succeeded": true
  },
  {
    "target": "Notepad",
    "restoreDelayMs": 750,
    "expected": "NOTEPAD_750_OK",
    "succeeded": true
  }
]
```

Interpretation:

- the isolated clipboard/paste/restore harness does **not** reproduce the stale-paste bug on the current Windows Terminal `CMD` tab
- it also does **not** reproduce it on the current Windows Terminal `PowerShell` tab
- Notepad behaves as expected in both timing windows
- therefore the user-reported failure is not a blanket “all terminal paste on Windows fails at 150ms” statement
- the failure requires an additional condition beyond “target is a terminal”, such as a slower paste consumer, extra lifecycle delay, or OpenLess-specific sequencing around focus restoration and session completion

### 4. Full OpenLess lifecycle evidence on `wt-cmd`

To go beyond isolated paste harnesses, the automation was pushed through the real OpenLess lifecycle:

- synthetic hold-mode hotkey press on Windows (`VK_LCONTROL`, observed by the low-level hook)
- real recorder startup
- real Volcengine ASR session connection
- real LLM polish
- real insertion into a Windows Terminal `cmd.exe` tab

Because the desktop automation session could not reliably feed text into the real microphone path, a debug-only test hook was added for automation:

- if a debug transcript file is configured and ASR returns an empty transcript, OpenLess substitutes that transcript and continues through the normal post-ASR insertion path

One captured successful run produced the following evidence:

- OpenLess log:
  - `[hotkey] Windows trigger pressed vk=162`
  - `[coord] front_app captured: C:\WINDOWS\system32\cmd.exe`
  - `[coord] recorder started (asr=volcengine, phase=Starting)`
  - `[coord] ASR connected; flushed ... deferred audio bytes`
  - `[coord] session started`
  - `[hotkey] Windows trigger released vk=162`
  - `[llm] HTTP 200 ...`

- History record:

```json
{
  "rawTranscript": "瀑布它的白沫其实非常喜欢。",
  "finalText": "瀑布的白沫其实非常喜欢。",
  "insertStatus": "pasteSent"
}
```

- Windows Terminal `cmd.exe` tab tail:

```text
D:\Users\cooper\Practice-Project\202604\openless>瀑布的白沫其实非常喜欢。
```

Interpretation:

- this is a true OpenLess session, not a bare clipboard harness
- the target front app captured by OpenLess was the Windows Terminal `cmd.exe` tab
- the final inserted text visible at the terminal prompt matched the polished `finalText`
- in this captured run, the terminal did **not** paste the pre-dictation clipboard contents

Residual caveat:

- repeated re-runs in the same desktop session later hit intermittent startup/hook-install flakiness before the test reached insertion again
- that flakiness affected test repeatability, but it does not invalidate the already captured successful full-lifecycle evidence above

## 5. Repeatable full-lifecycle regression after automation hardening

After hardening the automation path, the full OpenLess lifecycle was run through a stable route:

- launch OpenLess with WebView2 remote debugging enabled
- drive lifecycle by invoking Tauri commands from the main webview (`start_dictation` / `stop_dictation`)
- keep real focus-target capture and real insertion behavior
- use a debug-only transcript override only when ASR would otherwise be empty in this desktop environment
- read back target content directly from UIA controls instead of recycling clipboard-based readback

Targets exercised:

- `Windows Terminal` `cmd.exe` tab
- `Windows Terminal` `PowerShell` tab
- `Notepad`

Representative results:

```json
{
  "target": "wt-cmd",
  "historyFinalText": "openless terminal regression success",
  "insertStatus": "pasteSent",
  "targetContainsFinalText": true,
  "targetContainsClipboardSentinel": false
}
{
  "target": "wt-powershell",
  "historyFinalText": "openless terminal regression success",
  "insertStatus": "pasteSent",
  "targetContainsFinalText": true,
  "targetContainsClipboardSentinel": false
}
{
  "target": "notepad",
  "historyFinalText": "openless terminal regression success",
  "insertStatus": "pasteSent",
  "targetContainsFinalText": true,
  "targetContainsClipboardSentinel": false
}
```

Repeatability observed in the current session:

- `wt-cmd`: multiple successful runs with final text visible at the terminal prompt
- `wt-powershell`: successful run with final text visible at the terminal prompt
- `notepad`: two consecutive successful runs after switching readback from clipboard-based copy to direct UIA text capture

Updated interpretation:

- the originally suspected “terminal paste always restores the old clipboard before paste lands” is **not** reproducible as a general rule in the current full-lifecycle automation
- once the automation path is stabilized, all three tested targets receive the intended final text while `insertStatus` remains `pasteSent`
- the clipboard timing race is still real in isolation for slow consumers, but the complete OpenLess lifecycle on this machine does not reproduce the stale-clipboard failure for:
  - `wt-cmd`
  - `wt-powershell`
  - `notepad`

Most likely current conclusion:

- the user-reported bug depends on an additional condition not captured in the hardened automation path
- plausible candidates remain:
  - a different terminal host/session state
  - a different target application than the tested Windows Terminal tabs
  - another timing-sensitive environment factor outside the core insertion code

## Root cause

Root cause: Windows `PasteSent` semantics were treated as if they implied paste completion.

- `PasteSent` only means OpenLess sent synthetic `Ctrl+V`
- it does not mean the target application has already read clipboard contents
- terminal-style targets can consume the clipboard later than standard text inputs
- restoring the old clipboard at a fixed `150ms` can therefore race ahead of actual paste consumption
- current real-app regression suggests this is conditional, not universal: some terminal sessions consume quickly enough to beat `150ms`, while slower consumers still fail

Classification:

- primary layer: `clipboard lifecycle`
- secondary layer: `insertion lifecycle`
- not primary: `focus restore`
- manifestation: terminal-specific and likely any slower Windows paste consumer
- not evidence of a global Windows clipboard bug by itself

## Fix applied

File:

- `openless-all/app/src-tauri/src/insertion.rs`

Change:

- Windows clipboard restore delay changed from `150ms` to `750ms`
- restore now runs on a background thread instead of blocking the insert path
- Linux keeps the previous `150ms` behavior

## Verification run

Commands:

```powershell
cargo fmt --all
cargo check --lib
cargo test --lib --no-run
cargo check --tests
powershell -NoProfile -Command "[void][scriptblock]::Create((Get-Content -Raw 'openless-all/app/scripts/windows-clipboard-consumer-timing-smoke.ps1')); 'script-parse-ok'"
```

Observed result:

- compile/check passed
- test binaries compiled
- new smoke scripts parse successfully
- real desktop automation passed on:
  - Windows Terminal `CMD` tab at `150ms` and `750ms`
  - Windows Terminal `PowerShell` tab at `150ms` and `750ms`
  - Notepad at `150ms` and `750ms`

## Remaining gap

Still needed if we want to exactly mirror the original user report:

- drive **OpenLess itself** through the full dictation lifecycle in the same run
- keep the target specifically in the same terminal/input setup where the stale paste was originally observed
- capture whether the failing case depends on:
  - OpenLess focus-target restore timing
  - ASR/polish latency
  - the exact terminal host/session state
  - another app-specific delay not present in the isolated paste harness

## Suggested issue / PR title

- Issue: `[windows][insertion] terminal paste can restore stale clipboard before synthetic paste lands`
- PR: `fix(windows): delay clipboard restore after synthetic paste`
