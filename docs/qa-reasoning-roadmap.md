# 划词追问：思考能力（Reasoning）路线图

> 创建于 2026-05-01。流式输出（v2.1）已完成，**思考能力（v2.2）暂未实施**——这份文档是后续迭代的设计稿。
>
> 关联：issue #118 v2、PR #119、`openless-all/app/src-tauri/src/polish.rs`、`openless-all/app/src/pages/SelectionAsk.tsx`。

## 背景与决策

用户提出："QA 应该让 LLM 进行思考后再回复，并且可以设置思考强度"。

讨论了 3 条路径：

| 方案 | 实现 | 优 | 劣 |
|---|---|---|---|
| A | prompt-engineered（system prompt 加 `<thinking>` 块要求） | 0 配置改动；现 model 即可 | 思考质量受小模型限制；不可控 |
| B | OpenAI 标准 `reasoning_effort: low/medium/high` 字段 | 标准化 | DeepSeek-v4-flash 不识别该字段 |
| **C** | **切换 reasoner 模型（deepseek-r1 / o1 / claude extended thinking）** | **真**思考；可视化推理过程 | 用户得多配一个 model；UI 复杂度高 |

**结论**：选 C。A/B 在当前 provider 下等于无效。

## 实施分解

### 后端

1. **凭据存储**：`CredentialAccount` 加两条
   - `ArkReasonerModelId`（如 `deepseek-r1`、`doubao-seed-1.6-thinking`）
   - 复用现有 `ArkApiKey` / `ArkEndpoint`（同一 provider 不同 model）

2. **prefs**：`Preferences` 加字段
   - `qa_reasoning_effort: ReasoningEffort` 枚举 `Off | Low | Medium | High`
   - 默认 `Off`（与现行为一致）

3. **`answer_chat_streaming` 重载**：根据 effort 决定走 chat 还是 reasoner endpoint
   - `Off`：走 v2.1 现路径（chat 模型 + stream）
   - `Low/Medium/High`：走 reasoner 模型；强度通过 system prompt hint 调（"简短思考即可" / "详细思考" / "深度推理多角度"）
   - SSE 解析时同时收 `delta.content` + `delta.reasoning_content`，两者通过不同事件 emit：
     - `qa:state {kind:"reasoning_delta", chunk}`
     - `qa:state {kind:"answer_delta", chunk}` （已存在）

4. **answer_chat 拼装最终 message** 时，`reasoning_content` 不写入 `messages` 数组（只显示用，不进上下文）。多轮提问只把最终答案带回上下文。

### 前端

1. **SelectionAsk.tsx** 新增配置块：
   - 「思考强度」下拉：关闭 / 浅 / 中 / 深
   - 「思考模型」输入框（model id；默认 `deepseek-r1`）
   - i18n：zh-CN / en

2. **QaPanel.tsx** 新增「思考过程」可折叠区块：
   - 在 user 气泡下方、最终 assistant 气泡上方
   - 默认折叠，标题 `思考中…` / `思考过程（X 字）`，点击展开
   - 流式期间：实时拼接 `reasoning_delta`，气泡有打字 caret
   - 答案完成：折叠收起；用户随时可点开看推理

3. **types.ts**：`QaStateKind` 加 `'reasoning_delta'`；payload 加 `reasoning_chunk?: string`

### 边界与风险

- **Provider 兼容性**：火山 Ark 的 deepseek-r1 / doubao-thinking 都返回 `reasoning_content`；OpenAI o1 通过 thinking blocks（不是 reasoning_content），需要单独 adapter
- **Token 成本**：reasoner 模型 token 价格高 5-10x；用户开「深度」就是真烧钱，UI 应该有提示
- **延迟**：reasoner 首 token 可能 > 5s（思考阶段无 content 输出）。要在 UI 上区分「思考中」（reasoning streaming）vs「答题中」（content streaming），避免用户以为卡了

## 工作量估算

- 后端 reasoner 通路 + SSE 双流解析：~2h
- 前端折叠思考区块 + 打字 caret + 状态切换：~1.5h
- prefs / SelectionAsk 配置 UI + i18n：~0.5h
- 端到端测试（三档强度 × 单/多轮 × 错误回退）：~1h
- **总计**：~5h

## 实施先决条件

1. 用户配置好一个 reasoner model（deepseek-r1 / doubao-thinking-pro 等）
2. 后端凭据 vault 写入对应 model id
3. v2.1 流式输出已稳定（已完成 ✅）
