# Windows 临时激活式 TSF IME 设计

## 背景

OpenLess 当前 Windows 插入链路仍依赖剪贴板：先把最终文本写入剪贴板，再向焦点控件发送 `WM_PASTE`。这比模拟 `Ctrl+V` 更稳，但本质仍是粘贴工具，不是 Windows 输入法。

目标是在 Windows 上新增真正的 TSF 输入法后端，让语音结果通过系统文本输入框架提交，同时不破坏用户平时使用微软拼音、搜狗或英文键盘的手动输入体验。

## 目标

- OpenLess 在语音会话期间临时切换到 OpenLess TSF 输入法。
- 录音、ASR、润色、胶囊 UI、历史保存继续由现有 Tauri/Rust 主程序负责。
- OpenLess TSF IME DLL 只负责系统输入法身份、接收最终文本、通过 TSF 提交到当前文本上下文。
- 提交、取消或失败后自动恢复会话开始前的输入法 profile。
- 不要求用户手动切换输入法。
- 不把第三方中文输入法代理进 OpenLess IME；用户平时中文手打仍使用原输入法。

## 非目标

- 不把录音、网络请求、ASR、LLM、Tauri UI 放进 IME DLL。
- 不实现拼音候选、中文转换、词库或第三方 IME 代理。
- 不移除现有 Windows `WM_PASTE` 路径；它保留为未安装 TSF IME、切换失败或提交失败时的回退路径。
- 不承诺 UAC 安全桌面、管理员权限目标窗口、游戏、远程桌面或强隔离应用中的完整可用性。

## 架构

新增 Windows-only 输入层由三个部分组成：

1. `OpenLess` 主程序：沿用现有 `Coordinator` 状态机。语音热键开始时记录当前输入 profile 并临时激活 OpenLess TSF profile；语音结束后把最终文本发送给 IME；会话收尾时恢复原 profile。
2. `OpenLess TSF IME DLL`：COM in-proc text service，注册为 TSF input processor。它实现最小可用的激活、停用、编辑会话和文本提交能力，不持有产品业务状态。
3. `OpenLess IME IPC`：本机 IPC 通道，连接主程序和当前被 TSF 加载的 IME 实例。主程序发送带 session id 的最终文本；IME 在可写 TSF context 中调用 `ITfInsertAtSelection::InsertTextAtSelection`。

TSF IME 使用官方 profile 注册路径，而不是手写默认输入法注册表项。安装阶段注册 COM in-proc server、TSF text service、language profile，并把 OpenLess profile 加入当前用户可用输入法列表。

## 会话时序

1. 用户按下当前 OpenLess 全局热键。
2. `Coordinator` 从 `Idle` 进入录音启动流程。
3. Windows 输入 profile 守护逻辑读取并保存当前活动 profile，包括键盘布局或 TSF input processor。
4. 守护逻辑激活 OpenLess TSF profile，范围优先使用当前桌面 session。
5. 用户说话，现有 recorder、ASR、polish 流程不变。
6. 用户再次按热键结束录音；`Coordinator` 获得最终 polished text。
7. 主程序通过 IPC 向 OpenLess IME 发送 `{ session_id, text }`。
8. 当前焦点应用中的 OpenLess IME 实例在 TSF edit session 中提交文本。
9. 主程序收到提交成功、超时或失败结果。
10. 无论成功、取消还是失败，守护逻辑都尝试恢复第 3 步保存的输入 profile。
11. `Coordinator` 按现有规则保存历史并更新胶囊状态。

## Profile 切换策略

会话开始时记录完整 active profile，而不是只记录语言 ID。记录内容至少包括：

- profile type：keyboard layout 或 TSF input processor；
- language id；
- text service CLSID；
- profile GUID；
- HKL；
- 激活范围。

激活 OpenLess profile 时使用 TSF profile manager。若当前输入语言与 OpenLess profile 不一致，使用允许切换到指定 profile 的标志，避免因语言不匹配导致激活失败。

恢复时优先恢复原始 profile。若原始 profile 不再可用，记录 warning 并保持系统当前输入法，不再反复切换。恢复失败不阻塞历史保存。

## IPC 协议

MVP 使用本机低延迟 IPC，协议保持小而明确：

- `SubmitText { session_id, text, created_at }`
- `SubmitResult { session_id, status, error_code }`
- `CancelSession { session_id }`
- `Ping`

`session_id` 必须由现有 `DictationSession` 或 coordinator 会话生成，IME 只接受当前最新待提交 session，避免过期文本在焦点变化后落入错误应用。

IPC 超时策略：

- 等待 IME 连接：短超时，失败后走现有 `WM_PASTE` 回退。
- 等待提交结果：短超时，失败后恢复原 profile 并走回退或报 `CopiedFallback`。
- 会话取消：发送 `CancelSession`，IME 丢弃待提交文本。

## 失败与恢复

必须把“用户文字不丢失”作为约束：

- OpenLess profile 激活失败：不进入 TSF 提交流程，继续使用现有 Windows 插入后端。
- IME DLL 未安装或未注册：设置页显示状态，语音输入仍可用但使用回退后端。
- IPC 断开或超时：恢复原 profile，并使用现有 `WM_PASTE` 路径。
- TSF 提交返回只读、无 selection、context disconnected 或 no lock：恢复原 profile，并使用现有回退路径。
- 用户在 Processing 阶段取消：不提交文本，恢复原 profile。
- OpenLess 主程序崩溃：下次启动检查是否存在“上次会话临时切换未恢复”标记；若存在，尝试恢复最近保存的 profile。

## 用户体验

平时用户继续使用原输入法。只有语音会话期间，系统输入指示器可能短暂切到 OpenLess。会话结束后自动回到原输入法。

设置页新增 Windows-only 输入后端状态：

- TSF 输入法已安装并可用；
- TSF 输入法未安装；
- TSF 输入法注册异常；
- 当前使用剪贴板/`WM_PASTE` 回退。

默认行为保持保守：未安装 TSF IME 时，不改变现有插入体验。安装 TSF IME 后，Windows 平台优先使用临时激活式 TSF 后端。

## 文件与模块边界

计划新增或调整的主要区域：

- `openless-all/app/src-tauri/src/insertion.rs`：保留现有回退后端，新增 Windows TSF 后端选择入口。
- `openless-all/app/src-tauri/src/windows_ime_profile.rs`：封装 active profile 读取、OpenLess profile 激活、原 profile 恢复。
- `openless-all/app/src-tauri/src/windows_ime_ipc.rs`：封装主程序到 IME 的 IPC。
- `openless-all/app/windows-ime/`：新增 Windows-only TSF IME DLL 工程，包含 COM 注册、TSF text service、edit session、IPC 客户端。
- `openless-all/app/scripts/`：新增 Windows IME 注册、注销、打包脚本。
- `openless-all/app/src/lib/ipc.ts` 与设置页：暴露 Windows TSF 后端安装/健康状态。

Rust 业务模块仍遵守现有约束：叶子模块不互相调用；跨模块编排继续放在 `coordinator.rs`。

## 验证

自动验证：

- Rust backend type check：`cargo check --manifest-path openless-all/app/src-tauri/Cargo.toml`
- Windows IME 工程 build。
- profile 记录/恢复逻辑单元测试。
- IPC 协议编解码和过期 session 丢弃测试。
- 前端构建：`npm run build`

手动验证：

- Notepad：微软拼音为当前输入法，按 OpenLess 热键录音，提交后文本进入光标位置，并自动回到微软拼音。
- 浏览器文本框：同上。
- VS Code 编辑器：同上。
- 取消录音：不插入文本，并恢复原输入法。
- 未安装 OpenLess IME：语音输入仍走现有回退路径。
- 目标窗口不可写：不丢文本，恢复原输入法，并给出可理解状态。

## 参考

- Microsoft Learn: Custom Input Method Editor requirements
- Microsoft Learn: Text Services Framework
- Microsoft Learn: Text Service Registration
- Microsoft Learn: `ITfInputProcessorProfileMgr::ActivateProfile`
- Microsoft Learn: `ITfInsertAtSelection::InsertTextAtSelection`
