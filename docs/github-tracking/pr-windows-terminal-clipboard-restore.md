## 摘要

Closes #159

这个 PR 承接的是 Windows terminal insertion 链路的一次收敛修复：

- 历史上 terminal 场景出现过“不能自动上屏、需要手动 `Ctrl+V`”的用户反馈
- 本地测试也曾观察到一次“目标最终拿到旧剪贴板”的现象
- 本轮排查确认了其中一处真实存在的底层风险：clipboard restore timing

因此，这个 PR 的目标不是去声称“当前存在一个稳定复现的 terminal bug”，而是：

- 修补一处已经被确认的 Windows insertion 时序风险
- 把整条链路的回归覆盖补齐
- 把最终结论收敛到可审阅、可维护的状态

## 修复 / 新增 / 改进

- Windows clipboard restore 从 `150ms` 提高到 `750ms`
- clipboard restore 改为后台线程执行，不阻塞插入返回
- 新增 Windows clipboard timing smoke，用于验证慢消费者 race
- 新增完整生命周期自动化脚本，覆盖：
  - `wt-cmd`
  - `wt-powershell`
  - `notepad`
- 稳定化自动化入口：
  - 通过 WebView2 remote debugging 连接主页面
  - 通过 Tauri invoke 驱动 `start_dictation` / `stop_dictation`
- 新增 debug-only transcript override
  - 仅用于桌面音频路由不稳定时继续覆盖真实 insertion 尾链
- 调整目标读回方式：
  - terminal 走 UIA 读取 `TermControl`
  - notepad 走 UIA 直接读取文本
- 更新调查文档与 tracking 文档

## 兼容

- 正常用户路径不依赖 debug transcript override
- debug transcript override 仅在 `debug_assertions` / test 构建下参与
- Linux restore delay 保持原行为
- 不涉及 UI/视觉顺手修改
- 不涉及 QA hotkey / selection 主线逻辑修改

## 测试计划

- [x] `cargo fmt --all`
- [x] `cargo check --lib`
- [x] `python -m py_compile openless-all/app/scripts/windows-openless-lifecycle-e2e.py`
- [x] `windows-real-asr-insertion-smoke.ps1` 脚本解析通过
- [x] 隔离时序实验：
  - [x] 快消费者 + `150ms`
  - [x] 慢消费者 + `150ms`
  - [x] 慢消费者 + `750ms`
- [x] 完整生命周期自动化：
  - [x] `wt-cmd`
  - [x] `wt-powershell`
  - [x] `notepad`
- [x] 证据路径：
  - `docs/2026-05-02-windows-terminal-clipboard-restore-investigation.md`
  - `docs/github-tracking/issue-windows-terminal-clipboard-restore.md`

## 当前结论

- 历史上的 Windows terminal insertion 不稳定反馈是真实的
- 本轮排查确认并修补了一处真实存在的 clipboard restore timing 风险
- 稳定化完整生命周期自动化下：
  - `wt-cmd` 通过
  - `wt-powershell` 通过
  - `notepad` 通过
- 当前环境中，目标最终都拿到本次 `finalText`，未再出现旧 clipboard 上屏

因此，这个 PR 的技术定位应当是：

- 针对历史不稳定现象的一次 hardening 修复
- 外加完整的回归覆盖补强

## 剩余风险

- `750ms` 仍然是启发式保护，不是目标确认式握手
- 如果未来再出现 terminal 现场问题，更可能是更窄的环境因子，而不是当前这条主链路已经明确存在的稳定故障

建议 PR 标题：`fix(windows): 延后剪贴板恢复并补齐插入回归覆盖`
