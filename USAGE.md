# Open Typeless Harness 使用指南

Open Typeless Harness 是一个自学习语音输入实验应用：按快捷键说话，应用完成 ASR 转写、LLM 润色、插入当前光标，并在插入后观察你对文本的人工改动，把稳定纠错沉淀为本地 speech skills。

## 启动

```bash
cd openless-all/app
npm ci
OPENTYPELESS_EDIT_MONITOR_ENABLED=1 OPENTYPELESS_VIH_ENABLED=0 npm run tauri dev
```

首次启动需要授予麦克风和辅助功能权限。由于这是新的 app identity，macOS 可能要求重新授权。

## 基本使用

1. 把光标放到 TextEdit、浏览器输入框、微信、Notion 等真实输入框。
2. 按设置里的录音快捷键开始说话。本地测试常用 `leftControl` 按住说话。
3. 松开快捷键后等待转写和润色。
4. 文本插入后，如果你手动改了这段文字，edit monitor 会在短窗口内记录改动证据。
5. 后续口述时，相关 speech skills 会按 raw transcript 检索并注入润色 prompt。

## 本地文件

- App 数据：`~/Library/Application Support/OpenTypelessHarness`
- App 日志：`~/Library/Logs/OpenTypelessHarness/open-typeless-harness.log`
- 监控证据：`~/.openless/opentypeless-edit-monitor.jsonl`
- 学习候选：`~/.openless/opentypeless-learning-candidates.jsonl`
- Speech skill 记忆：`~/.openless/opentypeless-speech-skills.json`

## Smoke Test

```bash
open tools/fusion-edit-monitor-target.html
./scripts/fusion-smoke-watch.sh 120
```

然后聚焦网页里的 textarea，录一句话，等文本插入后在 30 秒内手动修改。 watcher 里应看到 `text_changed` 或 learning candidate 相关日志。

## 归属说明

本项目基于 OpenLess 的 MIT 代码底座做实验性融合，保留上游 license 和 attribution；当前重点是 OpenTypeless-style learning layer、focused-field edit monitor 和 speech-skill memory。
