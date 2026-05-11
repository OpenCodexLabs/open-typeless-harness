# Issue #143 Placeholder / 占位

## 中文摘要

本 PR 是 issue #143 的 draft 占位，专门跟踪 Windows 冷启动前几秒加载异常、闪烁与 ready 前展示错位问题。
当前只记录时序边界、现象入口和后续修复出口，不引入无关功能修改。

## Scope / 范围

- visible / ready timing
- first stable paint
- startup shell exposure
- Windows cold start UX

## Evidence / 证据入口

- `openless-all/app/src-tauri/tauri.conf.json`
- `openless-all/app/src/App.tsx`
- `openless-all/app/src/components/FloatingShell.tsx`

## Merge Rule / 合并规则

- 仅当 issue #143 的启动时序统一且完成 Windows cold-start smoke 后才允许从 draft 转为 ready。
