## Summary

Closes #154

This draft PR now serves as a near-closure tracking anchor for the Windows dual-hotkey-source problem.

Current conclusion:

- Windows dictation / QA lifecycle should be owned by the backend low-level hook
- renderer / window-local forwarding should not keep driving the same lifecycle
- future work here should focus on regression review, not on reopening the architecture without new evidence

## Current Status

- keep draft for now
- close to regression review
- not a parked native-strategy problem like #153

## Scope

- source ownership
- lifecycle precedence
- mixed-source risk on Windows

Out of scope:

- helper-window drag semantics
- main window / radius / appearance work
- broad hotkey adapter rewrites without new evidence

## Key Finding

```text
One lifecycle needs one owner source.
On Windows, that owner source should be the backend low-level hook.
```

## Evidence

- QA hotkey and follow-up flow remain healthy after ownership tightening
- no evidence from this repair track suggests the renderer-forward path should remain a co-owner

## Next Step

- use this PR as the place to summarize regression evidence
- only reopen architecture scope if new mixed-source failures appear

## Validation Plan

- [x] Manual verification: QA hotkey flow remains functional
- [x] Manual verification: lifecycle tightening did not break follow-up QA
- [ ] Regression review: confirm no new mixed-source phase drift evidence
