# Open Typeless Harness

<p align="center">
  <img src="docs/assets/open-typeless-harness-cover.png" alt="Open Typeless Harness cover" width="780">
</p>

<p align="center">
  <strong>会记住你如何修正它的语音输入。</strong>
</p>

<p align="center">
  • Speak • Polish • Learn Locally •
</p>

<p align="center">
  <a href="README.md">English</a> •
  <a href="#为什么">为什么</a> •
  <a href="#产品预览">产品预览</a> •
  <a href="#工作原理">工作原理</a>
</p>

<p align="center">
  <img alt="Status" src="https://img.shields.io/badge/status-technical%20preview-0F766E">
  <img alt="Tauri" src="https://img.shields.io/badge/Tauri-Rust%20%2B%20React-24C8DB">
  <img alt="Learning" src="https://img.shields.io/badge/learning-local%20speech%20skills-111827">
  <img alt="License" src="https://img.shields.io/badge/License-MIT-blue">
</p>

## 为什么

语音转文字还不够。

真正的语音输入问题，往往反复出现在同一批地方：产品名、项目名、中英混排术语、个人表达习惯，以及文本插入后你马上手动修掉的小错误。

Open Typeless Harness 只围绕一个判断：

> **每一次插入后的人工改动，都是学习信号。**

你自然说话。App 负责转写、润色、插入当前输入框，然后观察你如何修正这段文本。稳定的纠错模式会沉淀成本地 speech skills，并在后续润色前被检索注入。

它的目标不是只把声音变成文字，而是让语音输入越来越懂你的词汇和表达。

## 产品预览

<p align="center">
  <img src="docs/assets/open-typeless-harness-product.png" alt="Open Typeless Harness product preview" width="780">
</p>

<p align="center">
  <img src="docs/assets/open-typeless-harness-demo.gif" alt="Open Typeless Harness demo" width="780">
</p>

<p align="center">
  <a href="docs/assets/open-typeless-harness-demo.mp4">下载 MP4 演示视频</a>
</p>

## 解决什么

- 你说 `type script`，它应该知道你想写 `TypeScript`。
- 你把 `知呼` 改成 `知乎`，它应该记住。
- 你反复说某个产品名，它不该每次都当成一次性的 ASR 错误。
- 你在任意输入框口述，它应该融入你正在使用的 app。

## 工作原理

<p align="center">
  <img src="docs/assets/open-typeless-harness-learning-loop.png" alt="Open Typeless Harness learning loop" width="780">
</p>

```text
Voice input
  -> ASR transcript
  -> LLM polish with retrieved speech skills
  -> Insert into focused text field
  -> Observe post-insertion edits
  -> Learn local speech skills
```

示例：

```text
Inserted: 我想对表说 cold 或者 cold
Edited:   我想对标说 Claude Code 或 Codex
Learned:  cold 或者 cold -> Claude Code 或 Codex
```

## 原则

- **它是输入层，不是自主 agent。** 它帮你把文字写进当前输入框，不应该主动替你执行任务。
- **从真实改动里学习。** 最好的信号是用户插入后实际修了什么。
- **默认本地。** 纠错证据和 speech skills 默认保存在本机，除非显式导出或同步。
- **语境优先，而不是全局替换。** 歧义纠错应该成为 contextual skill，而不是危险的全局 find-and-replace。

## 状态

Open Typeless Harness 目前是 technical preview，是基于 OpenLess 桌面运行时的 OpenClaudex 实验性 fork/fusion。

当前重点：

- 聚焦输入框口述
- LLM 润色插入
- 插入后的改动监控
- 本地 speech-skill 学习
- 更安全的语境化纠错记忆

## 致谢

基于 OpenLess 桌面运行时构建，并继承 MIT license。

## License

[MIT](LICENSE)
