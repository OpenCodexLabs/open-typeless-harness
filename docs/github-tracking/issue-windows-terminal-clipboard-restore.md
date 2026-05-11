## 现象 / Symptom

Windows terminal 文本输入场景历史上出现过两类现象：

- 用户反馈 terminal 里不会自动上屏，需要再手动 `Ctrl+V`
- 本地测试曾观察到一次“目标最终拿到的是旧剪贴板，而不是本次听写结果”的现象

这两类现象都指向同一条 Windows insertion 链路：OpenLess 通过 clipboard + synthetic `Ctrl+V` 完成插入，而 terminal 是最敏感的目标类型之一。

### 证据 / Evidence

- `openless-all/app/src-tauri/src/insertion.rs`
  - Windows 路径的成功语义是 `PasteSent`
  - `PasteSent` 只代表已经发出 synthetic `Ctrl+V`
  - 它不代表目标已经完成 clipboard 消费
- `docs/2026-05-02-windows-terminal-clipboard-restore-investigation.md`
  - 已沉淀完整隔离实验、真实目标回归、完整生命周期自动化和最终结论
- 历史反馈层面
  - terminal 场景曾出现“不能自动上屏、需要手动 `Ctrl+V`”的真实用户反馈
- 隔离时序实验层面
  - 快消费者 + `150ms` restore：通过
  - 慢消费者 + `150ms` restore：读到旧剪贴板
  - 慢消费者 + `750ms` restore：恢复正常
- 完整生命周期回归层面
  - 稳定化自动化已覆盖 `wt-cmd`、`wt-powershell`、`notepad`
  - 当前机器上三类目标都能拿到本次 `finalText`

### 根因分析 / 追索过程

#### 1. 从用户现象到怀疑方向

最初现象不是“某个 API 报错”，而是目标内容不对：

- 目标没上屏
- 或者看起来像 paste 进了旧内容

这类问题天然需要同时排查三层：

- clipboard lifecycle
- insertion lifecycle
- focus / target restore

#### 2. 为什么先聚焦 clipboard restore

代码阅读后，Windows 插入链路具备一个明显特征：

- 先把本次文本写入 clipboard
- 再发 synthetic `Ctrl+V`
- 再恢复旧 clipboard

而状态语义里 `PasteSent` 并不等于“目标已经完成 paste”。
因此最早的根因假设是：

- 如果目标消费 clipboard 较慢，restore 可能会抢在目标 paste 之前发生

#### 3. 如何证明这个假设不是猜测

我们补了独立的时序实验，把 OpenLess 业务链路先拆开，只验证：

- clipboard 写入
- synthetic paste
- restore 时机
- 目标何时读取 clipboard

实验结果明确证明：

- race 在模型上真实存在
- `150ms` 对慢消费者不安全
- 增加 restore 窗口后可以避免慢消费者读到旧 clipboard

这一步把“怀疑”变成了“已确认的风险点”。

#### 4. 为什么还要继续做完整生命周期自动化

隔离实验只能说明风险存在，不能证明用户原始现象在真实 OpenLess 生命周期里一定复现。

因此后续又补了：

- 真实 OpenLess 启动
- 真实 focus-target capture
- 真实 insertion 尾链
- `wt-cmd` / `wt-powershell` / `notepad` 的目标读回

同时为了绕过桌面音频路由波动，又加了 debug-only transcript override，只在 ASR 为空时替换 transcript，保证：

- 前半段生命周期仍然真实
- 后半段 insertion / clipboard / target readback 仍然真实

#### 5. 最终根因判断

最终可以明确的根因不是“terminal 当前一定有 bug”，而是：

- Windows insertion 链路原本存在一个真实的 clipboard restore timing 风险
- 这个风险可以解释历史上 terminal 场景里的不稳定反馈
- 我们已经把这个风险点补了 hardening 修复

换句话说，这次 issue 真正承接的是：

- 一条历史上确实不够稳的 Windows terminal insertion 链路
- 以及其中一个已经被确认和修补的底层时序风险

### 平台边界 / Platform Scope

- 直接范围：Windows
- 关注层次：`clipboard lifecycle`、`insertion lifecycle`
- terminal 是重点观察目标，但不是唯一可能受影响的慢消费者
- `focus restore` 不是本轮主要根因

### 认领 / Ownership

- owner intent：`@Cooper-X-Oak`
- 当前对应 draft/ready PR：`#160`

## 影响 / Impact

- 影响 Windows terminal 文本输入的稳定性认知
- 会让 `PasteSent` 的用户语义和目标实际表现产生偏差
- 增加“为什么目标没上屏 / 为什么需要手动 Ctrl+V”的排障成本
- 对 Windows insertion 这条核心路径的可信度有直接影响

## 建议接受标准 / Proposed Acceptance Criteria

- [x] 明确 Windows `PasteSent` 与“目标已完成 paste”不是同一语义
- [x] 明确并记录 clipboard restore timing 风险模型
- [x] 完成最小 hardening 修复：
  - [x] Windows restore 延后到 `750ms`
  - [x] restore 改为异步执行
- [x] 提供隔离时序实验，证明 race 模型成立
- [x] 提供稳定化完整生命周期自动化，覆盖：
  - [x] `wt-cmd`
  - [x] `wt-powershell`
  - [x] `notepad`
- [x] 记录当前环境下的最终结论：
  - [x] 历史风险真实存在
  - [x] 当前回归未再出现目标吃到旧 clipboard 的结果
  - [x] 当前稳定性较历史状态已有改善

## TODO / 不确定项

- 是否需要进一步收紧 `PasteSent` 相关用户文案，避免被理解为“已确认粘贴成功”
- 若后续再收到用户现场反馈，是否需要补充更细的环境标签：
  - terminal host / profile
  - 输入法状态
  - 前台切换时序

建议 issue 标题：`[windows][insertion] 终端旧剪贴板粘贴风险已收敛，当前整链路回归稳定`
