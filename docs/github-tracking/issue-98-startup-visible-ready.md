## 现象 / Symptom

Windows 冷启动路径里，`visible` 与 `ready` 目前是脱钩的：主窗口可以先被用户看见，但 global hotkey / runtime lifecycle 还在后台异步安装。

这不是单纯的 UI 小闪烁，而是 startup lifecycle ownership 不统一：

- `main` 在配置层默认 `visible:false`
- backend 负责 `show_main_window()` / tray reopen / single-instance focus
- frontend `App.tsx` 又在 mount 后主动 `currentWindow.show()`
- Windows 路径下 `gate` 初始值直接是 `ready`

### 证据 / Evidence

- `openless-all/app/src-tauri/tauri.conf.json:17-30`
  - `main.visible = false`
- `openless-all/app/src-tauri/src/lib.rs:314-356`
  - backend 明确拥有 `show_main_window()` / `hide_main_window()` 生命周期入口
- `openless-all/app/src-tauri/src/lib.rs:158-163`
  - hotkey listener 与 QA hotkey listener 在 setup 后异步启动
- `openless-all/app/src/App.tsx:23-52`
  - Windows 路径初始化时直接 `gate='ready'`
  - mount 后又在 `requestAnimationFrame` 里调用 `currentWindow.show()`
- [2026-05-02-platform-lifecycle-audit.md](/D:/Users/cooper/Practice-Project/202604/openless/docs/2026-05-02-platform-lifecycle-audit.md)
  - 审计已将该问题归类为 startup lifecycle ownership 偏差

### 5 Whys / 根因分析

1. 为什么用户会看到一个看似 ready 的窗口，但热键/运行态未必已经 ready？
   - 因为窗口可见时机和 runtime readiness 时机不是一个 source of truth。
2. 为什么这两个时机分离了？
   - 因为 backend 和 frontend 同时持有 `main` visibility 的一部分控制权。
3. 为什么 Windows 上更明显？
   - 因为 Windows 启动路径跳过了 macOS 那种明确的 permission gate / startup shell，正式 UI 更早暴露。
4. 为什么这偏离了 macOS 的原始设计意图？
   - 原始意图是“用户看见主窗口时，它已经进入可用或可解释的阶段”；Windows 当前更像“窗口先到，能力后到”。
5. 为什么之前没被系统性识别？
   - 现有 smoke 主要验证“进程活着 + 稍后日志出现 hotkey installed”，没有验证“first visible frame == operationally ready”。

### 平台边界 / Platform Scope

- 直接症状范围：当前主要在 Windows 冷启动观察到。
- 问题层面：startup lifecycle ownership、window visibility contract、runtime readiness contract。
- 全平台风险判断：这是全平台架构层风险，但 Windows 因跳过 startup gate、前端主动 show，最先表现为真实用户问题。

### 认领 / Ownership

- owner intent：`@Cooper-X-Oak`
- 对应 draft PR：待创建

### 当前状态 / Current status

- startup lifecycle 主线修复已生效
- 最新测试入口改为 frontend-managed first show，不再用 backend immediate show 污染结果
- 人工冷启动体验反馈：几乎没有问题，人眼很难分辨
- 当前建议：保留 draft，继续观察 first-paint / startup latency，而不是继续扩大主修补丁

## 影响 / Impact

- 用户会把尚未 ready 的窗口误判为已经 ready
- 会放大“热键没反应 / 运行态未安装”的首屏困惑
- 让后续任何 Windows 启动问题更难分辨是 UI 问题、hotkey 问题，还是 lifecycle contract 问题

## 建议接受标准 / Proposed Acceptance Criteria

- [ ] `main` 窗口的首次可见时机只由一个 owner 控制
- [ ] first visible frame 与 runtime readiness 的关系被明确定义并可验证
- [ ] Windows 冷启动下，用户首次看到主窗口时，至少处于明确的 `startup` 或 `ready` 状态，而不是 ambiguous ready
- [ ] 增加一条启动 smoke：覆盖 `visible`、`hotkey installed`、`first usable state` 的先后顺序

## TODO / 不确定项

- 是否应把 `main` visibility 完全收回 backend，frontend 只负责内容 gate
- 是否要把现有 `issue #143` 的 first-paint 问题作为本 issue 的下游视觉子问题处理，还是继续分票并行跟踪
建议 issue 标题：`[tauri][windows] 冷启动时 visible 与 ready 脱钩`
