## 摘要

Closes #98  
References #143

这条 PR 已经不再只是 tracking 入口，而是承接本轮 Windows startup lifecycle 主线修复的实际变更。

当前结论：

- `visible / ready` 脱钩的主问题已收敛
- 冷启动入口已从 backend immediate show 调整为 frontend-managed first show
- 最新人工回归反馈是：启动过程基本流畅，人眼很难再分辨出明显的一闪
- `#143` 现在更适合作为已收敛的 first-paint 症状票引用，而不是继续作为主 closure 目标

## 修复 / 新增 / 改进

- 收口 Windows 启动阶段的 first-show ownership
- 在 `checking -> ready` 之间加入明确 gate，避免正式壳层在 startup transient phase 过早暴露
- 增加冷启动测试脚本，默认优先拉最新 debug build，并区分：
  - frontend-managed first show
  - backend immediate show（仅调试用）
- 增加 startup lifecycle contract test，锁住 hidden-on-create 与 readiness gate 语义

## 兼容

- 不包含：主窗口圆角 / 外框 / titlebar frame 等纯视觉适配
- 不包含：更细粒度 startup latency 优化
- 对现有用户 / 本地环境 / 构建流程的影响：聚焦 startup lifecycle 主线，不扩张到 UI polish 线

## 测试计划

- [x] 命令：`node openless-all/app/scripts/windows-startup-lifecycle-contract.test.mjs`
- [x] 结果：通过
- [x] 证据路径：本地命令输出

- [x] 命令：`npm run build`
- [x] 结果：通过
- [x] 证据路径：本地命令输出

- [x] 命令：`powershell -ExecutionPolicy Bypass -File openless-all/app/scripts/windows-cold-start.ps1 -PreferDebug -ShowMain`
- [x] 结果：能够走 frontend-managed first show
- [x] 证据路径：本地命令输出

- [x] 命令：冷启动截图与人工主观回归
- [x] 结果：首屏体验明显改善，当前主观反馈为“几乎没有问题，人眼很难分辨”
- [x] 证据路径：`artifacts-cold-start-screenshot.png`、`artifacts-cold-start-screenshot-8s.png`、`artifacts-cold-start-screenshot-front-managed.png` 与当前线程回归记录
