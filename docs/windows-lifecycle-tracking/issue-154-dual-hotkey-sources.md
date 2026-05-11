# Issue #154 Tracking

Scope: Windows dictation lifecycle driven by two hotkey event sources

Current stage:

- This branch is a draft PR placeholder.
- No runtime fix is included yet.
- The goal is to lock down source ownership and failure modes before changing behavior.

Problem statement:

- Windows currently has both OS-level low-level keyboard hook input and focused-window renderer forwarding.
- macOS/Linux do not have the same dual-source lifecycle driver.
- Shared dedupe exists, but source precedence is not yet a first-class contract.

Implementation target to converge before coding:

- Decide whether Windows should have one owner source or an explicit precedence model.
- Define expected behavior for mixed-source press/release ordering.
- Add testable scenarios for hold mode, toggle mode, and focus switching.

Non-goals in this draft:

- No hotkey adapter rewrite yet
- No input-stack refactor without agreed target contract
- No unrelated QA hotkey changes
