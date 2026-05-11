# Open Typeless Harness

<p align="center">
  <strong>面向聚焦输入框的自学习语音输入 harness。</strong>
</p>

<p align="center">
  ASR 转写 -> LLM 润色 -> 输入框改动监控 -> 本地 Speech Skills
</p>

<p align="center">
  <a href="README.md">English</a> |
  <a href="#功能亮点">功能亮点</a> |
  <a href="#快速开始">快速开始</a> |
  <a href="#工作流">工作流</a> |
  <a href="#安全模型">安全模型</a> |
  <a href="#路线图">路线图</a>
</p>

<p align="center">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-Rust%20%2B%20React-24C8DB">
  <img alt="macOS preview" src="https://img.shields.io/badge/macOS-preview-0F766E">
  <img alt="Local learning" src="https://img.shields.io/badge/Learning-local%20first-111827">
  <img alt="License" src="https://img.shields.io/badge/License-MIT-blue">
</p>

> [!IMPORTANT]
> Open Typeless Harness 是 OpenClaudex 语音输入方向的实验项目，基于 OpenLess 桌面运行时做融合。它探索的是一个更完整的口述闭环：在任意聚焦输入框说话、转写、润色、插入，然后观察用户对插入文本的人工改动，把稳定纠错沉淀成本地 speech skills，供后续口述复用。

## 快速导航

- [为什么做这个](#为什么做这个)
- [功能亮点](#功能亮点)
- [快速开始](#快速开始)
- [工作流](#工作流)
- [本地文件](#本地文件)
- [手动 Smoke Test](#手动-smoke-test)
- [验证命令](#验证命令)
- [仓库结构](#仓库结构)
- [安全模型](#安全模型)
- [路线图](#路线图)
- [相关项目](#相关项目)
- [归属说明](#归属说明)

## 为什么做这个

大多数语音输入工具停在 speech-to-text。这当然有用，但真实工作里的痛点往往不是“有没有转成文字”，而是项目名、产品名、中英混排术语、个人表达习惯，以及用户反复手动修正的 ASR/LLM 错误。

Open Typeless Harness 把“插入后的人工改动”视为最有价值的学习信号。它不要求用户手工维护词典，而是在本地记录纠错证据，把重复出现的稳定模式沉淀成 speech skills，并在后续 LLM 润色前按语境检索注入。

## 功能亮点

- **聚焦输入框口述**：按住配置好的快捷键说话，润色后的文本会插入当前输入框。
- **自学习闭环**：插入后的人工改动会被监控，并转化为可复用 speech skills。
- **本地纠错证据**：edit monitor 轨迹、学习候选、speech skills 默认只保存在本机。
- **带技能检索的 LLM 润色**：ASR transcript 进入润色前，会先检索相关 speech skills 注入 prompt。
- **短语级安全策略**：歧义较高的重复术语会存成短语级 skill，避免变成危险的全局替换。
- **可选 OpenTypeless bridge**：OpenTypeless VIH bridge 已保留，但在当前 fusion preview 中默认关闭。
- **新项目身份**：App 展示名、bundle identifier、窗口标题、日志路径和设置页文案已切到 Open Typeless Harness。

## 快速开始

```bash
cd openless-all/app
npm ci
OPENTYPELESS_EDIT_MONITOR_ENABLED=1 OPENTYPELESS_VIH_ENABLED=0 npm run tauri dev
```

当前本地快捷键可以在 App 设置里配置。开发阶段主要使用 `leftControl` 按住说话模式做 smoke test。

因为这个 fork 使用新的 app identity，macOS 可能会重新要求麦克风和辅助功能权限。授权后如果权限页没有刷新，需要完全退出 App 再重新打开。

## 工作流

```text
快捷键录音
  -> ASR 转写
  -> 检索本地 speech skills
  -> LLM 润色
  -> 插入当前聚焦输入框
  -> 监控插入后的人工改动
  -> 记录纠错证据
  -> 把稳定模式沉淀成本地 speech skills
```

示例学习路径：

```text
inserted: 我想对表说 cold 或者 cold
user edit: 我想对标说 Claude Code 或 Codex
learned: cold 或者 cold -> Claude Code 或 Codex
```

后续再口述相似内容时，这条 skill 可以在润色前被检索出来，让模型看到用户真实偏好的修正。

## 本地文件

运行证据默认只保存在本机：

- App 数据：`~/Library/Application Support/OpenTypelessHarness`
- App 日志：`~/Library/Logs/OpenTypelessHarness/open-typeless-harness.log`
- 监控证据：`~/.openless/opentypeless-edit-monitor.jsonl`
- 学习候选：`~/.openless/opentypeless-learning-candidates.jsonl`
- Speech skill 记忆：`~/.openless/opentypeless-speech-skills.json`

常用环境变量：

- `OPENTYPELESS_EDIT_MONITOR_ENABLED=0` 关闭插入后监控。
- `OPENTYPELESS_EDIT_MONITOR_JSONL=0` 关闭 monitor JSONL 日志。
- `OPENTYPELESS_LEARNING_CANDIDATES=0` 关闭学习候选记录。
- `OPENTYPELESS_SPEECH_SKILLS_PATH=/path/to/skills.json` 覆盖 skill memory 路径。
- `OPENTYPELESS_VIH_ENABLED=1` 启用可选 VIH rewrite bridge。

## 手动 Smoke Test

使用干净的浏览器测试目标：

```bash
open tools/fusion-edit-monitor-target.html
./scripts/fusion-smoke-watch.sh 120
```

然后：

1. 聚焦 textarea。
2. 按住配置好的语音输入快捷键，说一句短句。
3. 等待文本插入。
4. 在 30 秒内手动修改这段文本。
5. 查看 watcher 输出和上面的 JSONL 文件。

## 验证命令

核心 preview 检查：

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

## 仓库结构

- `openless-all/app/`：可运行的 Tauri 桌面 App。
- `openless-all/app/src-tauri/src/edit_monitor.rs`：聚焦输入框监控。
- `openless-all/app/src-tauri/src/learning_probe.rs`：纠错证据与 speech skill memory。
- `openless-all/app/src-tauri/src/vih_bridge.rs`：可选 OpenTypeless VIH bridge。
- `tools/fusion-edit-monitor-target.html`：手动浏览器测试目标。
- `scripts/`：smoke-test 辅助脚本。
- `USAGE.md`：本地测试使用指南。

## 安全模型

Open Typeless Harness 被设计成输入层，而不是自主执行任务的 agent。

- 它向当前聚焦输入框插入文本，不应该主动替用户执行任务。
- 学到的 speech skills 默认保存在本机，除非用户显式导出或同步。
- Monitor 记录的是插入后的文本变化证据，不是完整桌面活动。
- 歧义纠错应进入语境化短语 skill，而不是危险的全局替换。
- 云端 API 可用于实时转写或润色，但本仓库不保留录音。

## 路线图

- 稳定 macOS focused-field monitor 在更多编辑器和聊天应用里的表现。
- 增加轻量诊断面板，展示 monitor 事件和已学习 skills。
- 优化 skill 检索排序和过期 skill 清理。
- 等基础 fusion 路径稳定后，再重新评估可选 OpenTypeless VIH bridge。
- 以 Open Typeless Harness 身份打包签名版 macOS preview。

## 相关项目

- [OpenReview Agent](https://github.com/OpenClaudex/openreview-agent)：OpenClaudex 面向 OpenReview 工作流的 agent skill 与 CLI 工具包。
- [OpenLess](https://github.com/appergb/openless)：本实验融合使用的上游桌面运行时。

## 归属说明

本项目是基于 OpenLess MIT 代码底座的实验性 fork/fusion，不代表 OpenLess 官方项目。上游 license 保留在 `LICENSE`。
