## Symptom

Windows dictation / QA lifecycle previously had two event sources driving the same state machine:

- OS-level low-level keyboard hook
- renderer / window-local hotkey forwarding

That design is risky even when the product "seems to work":

- press/release edges can come from different sources
- focus switches can strand half an edge
- hold mode and toggle mode can drift differently on Windows only

## Evidence

- [openless-all/app/src/App.tsx](/D:/Users/cooper/Practice-Project/202604/openless/openless-all/app/src/App.tsx)
  - Windows window-local forwarding existed in the frontend path
- [openless-all/app/src-tauri/src/coordinator.rs](/D:/Users/cooper/Practice-Project/202604/openless/openless-all/app/src-tauri/src/coordinator.rs)
  - backend also accepted `handle_window_hotkey_event`
- [openless-all/app/src-tauri/src/hotkey.rs](/D:/Users/cooper/Practice-Project/202604/openless/openless-all/app/src-tauri/src/hotkey.rs)
  - Windows already owns a `WH_KEYBOARD_LL` low-level hook

Current convergence from this repair track:

- QA hotkey / follow-up flow works
- Windows owner source should be the backend low-level hook
- window-local forwarding should not keep driving the same lifecycle

## Root Cause Convergence

This was not just "an extra fallback path".

It was an ownership problem:

```text
Two independent input sources were allowed to influence one dictation / QA
lifecycle state machine without an explicit precedence contract.
```

## 5 Whys

1. Why is this a lifecycle issue and not just a convenience fallback?
   - Because the second path was able to trigger real start/stop edges.
2. Why is that dangerous?
   - Because mixed-source ordering can desynchronize phase transitions.
3. Why is it primarily a Windows issue?
   - Because Windows carried both the low-level hook and the renderer-forward path.
4. Why does this diverge from original intent?
   - Because one user gesture should map to one stable lifecycle transition.
5. Why is this near closure now?
   - Because current repair work has already converged on a single owner source: backend low-level hook.

## Platform Scope

- Direct symptom scope: Windows implementation risk
- Problem layer: input source ownership, lifecycle precedence, focus-sensitive edge delivery

## Related Issues

- #154 main issue anchor
- #147 settings-to-runtime listener refresh contract
- #158 governance issue for helper-window / native-window contract family

## Impact

- Without a single owner source, Windows-only lifecycle drift remains hard to reproduce and harder to trust
- With ownership clarified, regression review can focus on evidence instead of guessing which path fired

## Proposed Acceptance Criteria

- [ ] Windows lifecycle owner source is explicitly documented as backend low-level hook
- [ ] window-local forwarding no longer drives the main lifecycle unless a future explicit fallback contract is introduced
- [ ] regression review confirms no new mixed-source ordering evidence

## Status Note

Current recommendation: treat this issue as near-closure and use it as a regression-review anchor rather than a new large refactor anchor.
