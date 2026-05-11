# Open Typeless Harness

<p align="center">
  <strong>Self-improving voice input harness for focused-field dictation.</strong>
</p>

<p align="center">
  ASR -> LLM polish -> focused-field monitor -> local speech skills
</p>

<p align="center">
  <a href="README.zh.md">中文文档</a> |
  <a href="#features">Features</a> |
  <a href="#quick-start">Quick Start</a> |
  <a href="#workflow">Workflow</a> |
  <a href="#safety-model">Safety Model</a> |
  <a href="#roadmap">Roadmap</a>
</p>

<p align="center">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-Rust%20%2B%20React-24C8DB">
  <img alt="macOS preview" src="https://img.shields.io/badge/macOS-preview-0F766E">
  <img alt="Local learning" src="https://img.shields.io/badge/Learning-local%20first-111827">
  <img alt="License" src="https://img.shields.io/badge/License-MIT-blue">
</p>

> [!IMPORTANT]
> Open Typeless Harness is an experimental OpenClaudex voice-input project built on top of the OpenLess desktop runtime. It explores a tighter loop for dictation: speak into any focused text field, polish the transcript, observe what the user edits after insertion, and turn stable corrections into local speech skills for future dictation.

## Quick Navigation

- [Why This Exists](#why-this-exists)
- [Features](#features)
- [Quick Start](#quick-start)
- [Workflow](#workflow)
- [Local Files](#local-files)
- [Manual Smoke Test](#manual-smoke-test)
- [Verification](#verification)
- [Repository Layout](#repository-layout)
- [Safety Model](#safety-model)
- [Roadmap](#roadmap)
- [Related Projects](#related-projects)
- [Attribution](#attribution)

## Why This Exists

Most voice-input tools stop at speech-to-text. That is useful, but it does not solve the recurring problems that appear in real work: project names, product names, mixed Chinese/English terms, personal phrasing, and ASR/LLM mistakes that the user keeps fixing by hand.

Open Typeless Harness treats those post-insertion edits as the most valuable signal. Instead of asking the user to maintain a dictionary manually, it records local correction evidence, distills repeated patterns into speech skills, and retrieves those skills before later LLM polish.

## Features

- **Focused-field dictation**: Hold the configured hotkey, speak, and insert the polished result into the current text field.
- **Self-improving loop**: Post-insertion edits are monitored and converted into reusable speech skills.
- **Local correction evidence**: Edit monitor traces, learning candidates, and speech skills are stored on the local machine by default.
- **LLM polish with retrieved skills**: The polish prompt receives relevant learned skills before rewriting the ASR transcript.
- **Phrase-level safety**: Ambiguous repeated terms are stored as phrase-level skills instead of unsafe global replacements.
- **OpenTypeless bridge option**: The optional VIH bridge is present but disabled by default for this fusion preview.
- **Rebranded desktop shell**: The app identity, bundle identifier, window titles, logs, and settings copy now use Open Typeless Harness.

## Quick Start

```bash
cd openless-all/app
npm ci
OPENTYPELESS_EDIT_MONITOR_ENABLED=1 OPENTYPELESS_VIH_ENABLED=0 npm run tauri dev
```

The current local hotkey is configured in app preferences. During development, the main smoke path used `leftControl` in hold mode.

Because this fork uses a new app identity, macOS may ask for microphone and Accessibility permissions again. Grant both permissions, fully quit the app, and relaunch if the permission page does not refresh.

## Workflow

```text
hotkey + recording
  -> ASR transcript
  -> retrieve local speech skills
  -> LLM polish
  -> insert into the focused text field
  -> monitor post-insertion user edits
  -> record correction evidence
  -> distill stable patterns into local speech skills
```

Example learning path:

```text
inserted: 我想对表说 cold 或者 cold
user edit: 我想对标说 Claude Code 或 Codex
learned: cold 或者 cold -> Claude Code 或 Codex
```

On later dictations, that learned skill can be retrieved before polish so the model sees the user's preferred correction in context.

## Local Files

Runtime evidence is local-only by default:

- App data: `~/Library/Application Support/OpenTypelessHarness`
- App log: `~/Library/Logs/OpenTypelessHarness/open-typeless-harness.log`
- Edit monitor evidence: `~/.openless/opentypeless-edit-monitor.jsonl`
- Learning candidates: `~/.openless/opentypeless-learning-candidates.jsonl`
- Speech skill memory: `~/.openless/opentypeless-speech-skills.json`

Useful environment flags:

- `OPENTYPELESS_EDIT_MONITOR_ENABLED=0` disables post-insertion monitoring.
- `OPENTYPELESS_EDIT_MONITOR_JSONL=0` disables monitor JSONL logging.
- `OPENTYPELESS_LEARNING_CANDIDATES=0` disables learning candidate recording.
- `OPENTYPELESS_SPEECH_SKILLS_PATH=/path/to/skills.json` overrides skill memory.
- `OPENTYPELESS_VIH_ENABLED=1` enables the optional VIH rewrite bridge.

## Manual Smoke Test

Use the clean browser target:

```bash
open tools/fusion-edit-monitor-target.html
./scripts/fusion-smoke-watch.sh 120
```

Then:

1. Focus the textarea.
2. Hold the configured dictation hotkey and speak a short sentence.
3. Wait for insertion.
4. Edit the inserted text within 30 seconds.
5. Check the watcher output and the JSONL files above.

## Verification

Core preview checks:

```bash
cd openless-all/app
npm run build

cd src-tauri
cargo fmt --check
cargo test -q learning_probe -- --test-threads=1
cargo check -q --bin openless

cd ../../..
bash -n scripts/fusion-smoke-watch.sh scripts/fusion-open-smoke-target.sh
git diff --check
```

## Repository Layout

- `openless-all/app/`: runnable Tauri desktop app.
- `openless-all/app/src-tauri/src/edit_monitor.rs`: focused-field monitor.
- `openless-all/app/src-tauri/src/learning_probe.rs`: correction evidence and speech skill memory.
- `openless-all/app/src-tauri/src/vih_bridge.rs`: optional OpenTypeless VIH bridge.
- `tools/fusion-edit-monitor-target.html`: manual browser test target.
- `scripts/`: smoke-test helpers.
- `USAGE.md`: Chinese usage guide for local testing.

## Safety Model

Open Typeless Harness is designed as an input layer, not an autonomous action agent.

- It inserts text into the focused field; it should not execute user tasks on its own.
- Learned skills stay local unless the user explicitly exports or syncs them.
- The monitor records post-insertion text changes as correction evidence, not full desktop activity.
- Ambiguous corrections should become contextual phrase skills, not broad global replacements.
- Cloud APIs may be used for real-time transcription or polish, but recordings are not retained by this repository.

## Roadmap

- Stabilize the macOS focused-field monitor across more editors and chat apps.
- Add a lightweight diagnostics panel for monitor events and learned skills.
- Improve skill retrieval ranking and stale-skill pruning.
- Revisit the optional OpenTypeless VIH bridge after the base fusion path is stable.
- Package a signed macOS preview build under the Open Typeless Harness identity.

## Related Projects

- [OpenReview Agent](https://github.com/OpenClaudex/openreview-agent): OpenClaudex agent skill and CLI toolkit for safe OpenReview workflows.
- [OpenLess](https://github.com/appergb/openless): upstream desktop runtime used as the base for this experimental fusion.

## Attribution

This project is an experimental fork/fusion built on top of OpenLess and keeps the upstream MIT license. See `LICENSE` for inherited license terms.
