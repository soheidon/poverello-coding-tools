[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

> **v0.25.1 過渡說明：** Auto-Converge 實作會保留在程式碼中，但從應用程式的一般導覽中隱藏。Production dispatch 仍維持停用，Stage 2 隔離尚未驗證。這不代表已達到發行準備狀態。

**以 Claude Code / Claude Desktop 作為程式碼執行框架，將推論請求路由至第三方 LLM API，並使用外部模型作為 Google Antigravity 的規劃器與審查器。**

Poverello Coding Tools（應用程式內顯示名稱為 **Poverello**）是一款專為 AI 輔助軟體開發設計的 Windows 配套應用程式，支援五種互補的工作流程：

1. **Claude Code / Claude Desktop 的第三方閘道（3P Gateway）** — 保留 Claude 的程式庫探索、工具使用、檔案編輯與測試執行功能，同時將推論請求路由至第三方供應商。
2. **Claude Code 專用本機 LLM（Ollama）** — 使用本機模型離線執行 Claude Code。
3. **Google Antigravity 的 MCP 規劃器與審查器** — 透過 `poverello/plan`、`poverello/revise` 和 `poverello/review` MCP 工具，將實作規劃、計畫修訂與實作後審查委派給外部模型。
4. **多代理開發 Orchestrator** — 執行目前支援的有界 Planner–Reviewer 工作流程，並記錄標準化 token 用量與依已核驗供應商費率計算的成本估算。
5. **Antigravity 規則管理** — 為 Antigravity 設定、預覽並安全同步 Poverello Direct Coding 規則（`pov-direct.md`）。

> **最新消息：** 請參閱 [NEWS.md（版本說明）](../NEWS.md)，了解各版本功能、實作細節與變更記錄。

---

## 五大主要工作流程

### 1. Claude Code / Claude Desktop 搭配第三方閘道

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P Gateway
             ↓
Anthropic Direct / DeepSeek / Kimi Code / OpenRouter / MiniMax / MiMo / GLM
```

- **框架與模型分離**：保留 Claude 的智能代理工具，同時將推論請求路由至第三方供應商。
- **動態多配置檔路由**：可從 GUI 切換使用中的供應商、OpenRouter 配置檔及模型路由。
- **設定指南**：[Claude Desktop / Cowork 第三方閘道設定](THIRD_PARTY_INFERENCE.md)

### 2. Claude Code 專用本機 LLM（Ollama）

```text
Claude Code CLI
      ↓ (Loopback ANTHROPIC_BASE_URL)
Ollama Local (127.0.0.1:11434/v1)
      ↓
Gemma 4 / Qwen / Llama 3 / DeepSeek-R1（本機裝置端）
```

- **完全免費、離線與隱私**: 無需 API 金鑰，無用量限制，無雲端資料傳輸。
- **獨立路由與模型探索**: 獨立的 Claude Code 路由（`active_route: "ollama"`），支援 `/api/tags` 本機模型自動探索。
- **思考模式與上下文微調**: 支援為每個別名設定思考預算（1024 標記）與上下文長度（`num_ctx`）。
- **完全隔離**: Claude Desktop、Cowork on 3P 及 Google Antigravity MCP 繼續獨立使用雲端模型。
- **設定指南**: [Ollama Local 指南](OLLAMA_LOCAL.zh-TW.md)

### 3. Antigravity + MCP 規劃器與審查器

```text
Antigravity
    ↓ stdio
anthro-bridge.exe --mcp-server
    ↓
已設定的外部模型（規劃器 / 審查器）
    ↓
實作計畫 / 審查結論
    ↓
Antigravity 使用訂閱容量
執行實作與測試
```

- **規劃與執行分工**：外部模型負責生成高階計畫或審查結論；Antigravity 訂閱容量負責執行耗費大量 Token 的程式碼編輯。
- **即時 GUI 設定**：切換規劃器或審查器的供應商、模型或推論強度，下次呼叫時立即生效。
- **設定指南**：[Google Antigravity + Poverello MCP 設定](ANTIGRAVITY_MCP.zh-TW.md)

為維持相容性，可執行檔與 MCP 工具識別名稱仍保留 `anthro-bridge`。

Antigravity 的正式指令為 `/pov-mcp-plan`、`/pov-mcp-revise` 和 `/pov-mcp-review`；舊版 `/anthro-*` 指令仍為相容性保留。

**Antigravity 全域指令：**

- **`/pov-mcp-plan`** — 將實作規劃委派給已設定的外部模型。
- **`/pov-mcp-revise`** — 根據新的回饋或限制條件修改現有計畫。
- **`/pov-mcp-review`** — 在提交前，對照已核准的計畫審查已完成的實作，並給出明確的 READY / NOT READY 結論。

自主工作流程：`/pov-full-cycle`、`/pov-plan-only`、`/pov-build-review`。

**建議工作流程：**

```text
/pov-mcp-plan → 實作與測試 → /pov-mcp-review → 提交
```

---

### 4. 多代理開發 Orchestrator（有界工作流程與成本）

Orchestrator 使用可設定的角色設定檔與迭代上限執行目前支援的 Planner–Reviewer 工作流程。建立新的主要計畫一律需要使用者明確確認。`%APPDATA%\Poverello\runs\` 中的執行記錄會保存可取得的標準化用量、稽核中繼資料與依已核驗費率計算的成本估算。估算不是供應商帳單；不支援的路由不會計算成本。

### 5. Antigravity 規則管理（Poverello Direct Coding）

可透過 GUI 設定、預覽並同步 `pov-direct.md`。應用程式會保留自訂 frontmatter 與註解，並提供經驗證的套用、備份及復原保護。

## 支援的供應商

| 供應商 | 連線方式 | 支援系列 | 推論控制 |
|---|---|---|---|
| **Anthropic** | 直接 API | Claude Opus 5.5、Sonnet 5.5、Haiku 4.5、Sonnet 3.7 | Adaptive thinking / 推論強度 |
| **DeepSeek** | 直接 API | `deepseek-flash`（V4.1 Flash 1017）、`deepseek-v4-pro`（V4 Pro 0813） | Normal / Thinking · Low / High / Max |
| **GLM (Z.ai)** | 直接 API | GLM-5.3、GLM-5.3-Flash、GLM-5.3-FlashX、GLM-5.2 | GLM-5.3: Low / High / Max · GLM-5.2: Normal / Thinking |
| **Kimi Code** | 直接 API | kimi-for-coding、kimi-for-coding-highspeed | 思考模式 |
| **MiniMax** | 直接 API | MiniMax M3、M2.7 | 依模型而定 |
| **Kimi / Moonshot** | 直接 API | Kimi K2.x、Kimi K3 | 思考 / 推論強度 |
| **MiMo / Xiaomi** | 直接 API | MiMo V2.6 Flash、Pro、Pro-UltraSpeed（相容 V2.5） | Normal / Thinking |
| **Ollama Local** | 本機迴路 | Gemma 4、Qwen 2.5、Llama 3.3、DeepSeek-R1 | Thinking（1024 預算）/ Disabled |
| **OpenRouter** | 多配置檔閘道 | 請參閱下方 OpenRouter 章節 | 依模型 / 配置檔而定 |
| **多供應商閘道** | 自訂路由 | 任意符合資格的供應商組合 | 依路由控制 |

### Anthropic（直接連線）

使用 `https://api.anthropic.com` 和 `ANTHROPIC_API_KEY` 直接連線官方 API。支援 `claude-opus-5-5`、`claude-sonnet-5-5`、`claude-haiku-4-5`、`claude-3-7-sonnet`、`claude-3-5-sonnet` 和 `claude-3-5-haiku`，並支援上游模型提供的 adaptive thinking 與推論強度控制。

### GLM / Z.ai（直接連線）

使用 Z.ai 官方 Anthropic Messages 相容 API（`https://api.z.ai/api/anthropic/v1/messages`）搭配 `ZAI_API_KEY` 直接連線。GLM-5.3 系列可選擇推論強度（`low`、`high`、`max`）；GLM-5.2 提供 Normal / Thinking 切換。

### DeepSeek（直接連線）

內建的 **Direct DeepSeek** 預設路由：Opus 5 → V4.1 Flash / Max · Sonnet 5 → V4.1 Flash / High · Haiku 4.5 → V4.1 Flash / Low。

- `deepseek-flash` — DeepSeek-V4.1-Flash-1017 的正規穩定 API ID；上下文視窗 1M，最多輸出 384K token。
- `deepseek-v4-pro` — API ID；目前官方價格頁將其列為 DeepSeek-V4-Pro-0813。上下文視窗 1M，最多輸出 384K token。
- Flash 支援視覺輸入，Pro 不支援。DeepSeek 仍接受舊 ID `deepseek-v4-flash` 與 `deepseek-v4-flash-vision-exp`，並將它們視為 Flash 別名、按 Flash 費率計費。

### Kimi Code（直接連線）

專屬的程式碼專家 API（`KIMI_CODE_API_KEY`），與 Moonshot Kimi 分開：

- `kimi-for-coding` — 完整品質的程式碼模型。
- `kimi-for-coding-highspeed` — 低延遲變體。

### MiMo / Xiaomi（直接連線）

內建的 **Direct MiMo** 預設路由：
- Opus 5 → `mimo-v2.6-pro` / Thinking
- Sonnet 5 → `mimo-v2.6-pro` / Normal
- Haiku 4.5 → `mimo-v2.6-flash` / Thinking
- 預設模型：`mimo-v2.6-flash`

模型：`mimo-v2.6-flash`、`mimo-v2.6-pro`、`mimo-v2.6-pro-ultraspeed`（可選）。三者均支援 100 萬 Token 的上下文視窗及原生多模態能力（文字、圖像、影片）。

**Normal / Thinking**：MiMo 採用簡單的 Normal/Thinking 切換 —— 不支援推論強度等級。

**V2.5 向下相容**：已儲存的 `mimo-v2.5`、`mimo-v2.5-pro`、`mimo-v2.5-pro-ultraspeed` 路由將持續正常運作。未修改的舊版預設設定將於啟動時自動遷移至 V2.6。

### 專用本機 LLM（Ollama Local）

**專供 Claude Code CLI 使用** 的本機推論後端：
- 端點：`http://127.0.0.1:11434/v1`（原生 Anthropic messages 迴路）
- 透過迴路 `/api/tags` 動態探索已安裝的本機模型
- 支援各別名的思考預算（1024 tokens）與上下文視窗（`num_ctx`）覆寫
- 詳細設定請參閱 [Ollama Local 指南](OLLAMA_LOCAL.zh-TW.md)

### OpenRouter

支援多個命名配置檔。單一下拉選單中的目前 OpenAI 模型目錄（15 個模型）：

| 模型 ID | 顯示名稱 |
|---|---|
| `openai/gpt-6.1-sol` | GPT-6.1 Sol |
| `openai/gpt-6.1-sol-pro` | GPT-6.1 Sol Pro |
| `openai/gpt-6-sol` | GPT-6 Sol |
| `openai/gpt-6-sol-pro` | GPT-6 Sol Pro |
| `openai/gpt-6-luna` | GPT-6 Luna |
| `openai/gpt-6-luna-pro` | GPT-6 Luna Pro |
| `openai/gpt-6-astra` | GPT-6 Astra |
| `openai/gpt-6-astra-pro` | GPT-6 Astra Pro |
| `openai/gpt-astra-latest` | GPT Astra Latest |
| `openai/gpt-5.6-sol` | GPT-5.6 Sol |
| `openai/gpt-5.6-sol-pro` | GPT-5.6 Sol Pro |
| `openai/gpt-5.6-terra` | GPT-5.6 Terra |
| `openai/gpt-5.6-terra-pro` | GPT-5.6 Terra Pro |
| `openai/gpt-5.6-luna` | GPT-5.6 Luna |
| `openai/gpt-5.6-luna-pro` | GPT-5.6 Luna Pro |

**GPT-6 Astra**：1.05M 上下文 · 推論強度：`low / medium / high / xhigh / max`。
**GPT-6 Astra Pro**：1.05M 上下文 · 永久啟用 Pro 推論（`reasoning.mode = pro`），無法由使用者選擇強度。
**GPT Astra Latest**：追蹤最新 Astra 系列模型的別名。

內建 **OpenRouter: chatGPT** 預設路由：Opus 5 → GPT-6 Astra / max · Sonnet 5 → GPT-6 Astra / high · Haiku 4.5 → GPT-6 Astra / medium。

此外亦提供：**OpenRouter: Gemini**（Gemini 3.8 Flash · 推論強度 `low / medium / high`）、**OpenRouter: Poolside**、**OpenRouter: Tencent**、**OpenRouter: InclusionAI**、**OpenRouter: StepFun**。

---

## DeepSeek API 價格（核驗於 2026-10-19）

| 模型 ID | 尖峰輸入（快取未命中 / 命中） | 離峰輸入（快取未命中 / 命中） | 尖峰輸出 | 離峰輸出 |
|---|---:|---:|---:|---:|
| `deepseek-flash` | \$0.30（\$0.006） | \$0.15（\$0.003） | \$1.20 | \$0.60 |
| `deepseek-v4-pro` | \$1.32（\$0.044） | \$0.66（\$0.022） | \$3.96 | \$1.98 |

DeepSeek 費率均以每百萬 token 計。工作日 UTC 01:00–04:00、06:00–10:00 為尖峰時段（中國法定假日除外）；週末及假日全天為離峰。Poverello 內含已核驗的 2026 年中國假日與調休上班日資料，並會提示次年日曆尚未收錄。其他提供者價格為應用程式內建參考值，可能變動。Orchestrator 成本估算目前只涵蓋符合條件且快取用量明確為零的 Anthropic Direct 呼叫，不估算 DeepSeek 費用。關於 Pro，目前官方價格頁列出獨立費率，並表示 API 服務於 2026 年 9 月 14 日後持續提供；但 9 月 10 日公告曾稱 V4.1-Pro 發布前會暫時轉送至 V4.1 Flash 並採用 Flash 費率。Poverello 無法核實每筆請求的路由或帳單；顯示的是公開費率，不是帳單或保證。請參閱[目前價格頁](https://api-docs.deepseek.com/quick_start/pricing/)和[9 月 10 日公告](https://api-docs.deepseek.com/news/news260910/)。

---

## 安裝

從 [Releases](https://github.com/soheidon/poverello-coding-tools/releases) 頁面下載目前最新已發布的 Windows 安裝程式並執行。

Windows 安裝程式支援 8 種語言。Poverello 保留既有應用程式識別碼，並支援從 Anthro Bridge 原位升級。首次啟動時，程式設計為將 `%APPDATA%\Anthro Bridge`（開發版為 `%APPDATA%\Anthro Bridge Dev`）中的設定與執行資料複製到對應的 Poverello 資料夾，同時保留原資料夾作為復原副本。實際 Windows 環境中的安裝、資料移轉、升級、重新安裝、取消及解除安裝流程尚未驗證；升級前請備份重要資料。

---

## 快速開始

### 工作流程 1：Claude Code / Claude Desktop 的第三方閘道

1. 開啟 Poverello，前往 **Settings > API Key**，為所需的供應商設定 API 金鑰。
2. 在儀表板上選擇您的供應商或 OpenRouter 配置檔。
3. 點擊 **Start Gateway**（執行於 `http://127.0.0.1:4000`）。
4. 連接 Claude Code 或 Claude Desktop：
   - **Claude Code**：在 Settings 中點擊 **Copy Claude Code launch command**，並貼入 PowerShell。
   - **Claude Desktop / Cowork**：請參閱 [Claude Desktop 第三方設定指南](THIRD_PARTY_INFERENCE.md)。

### 工作流程 2：Claude Code 專用本機 LLM（Ollama）

1. 確認 Ollama 正在本機執行（`http://127.0.0.1:11434`）。
2. 在 Poverello 儀表板選擇 **Ollama Local**，或在 **Settings > Claude Code / Local LLMs** 設定模型。
3. 點擊 **Copy Claude Code launch command** 並貼到 PowerShell。
4. Claude Code 將直接連線至本機 Ollama 執行個體。

### 工作流程 3：Google Antigravity 的 MCP 規劃器與審查器

1. 在 Poverello 中為您選擇的規劃器 / 審查器模型設定 API 金鑰。
2. 選擇 **MCP** 標籤，並在 **Settings > Antigravity > MCP Plan Settings** 中設定您的模型。
3. 在 Antigravity 的 MCP 設定中以 `["--mcp-server"]` 參數註冊 `anthro-bridge.exe`（或在 Poverello 中點擊 **Configure Automatically**）。
4. 使用 `/pov-mcp-plan` 設計計畫、`/pov-mcp-revise` 更新計畫，以及 `/pov-mcp-review` 在提交前審查實作成果。
5. 請參閱完整的 [Antigravity MCP 設定指南](ANTIGRAVITY_MCP.zh-TW.md)。

### 工作流程 4：多代理開發 Orchestrator

**使用方式：** v0.25.1 已從上方導覽列隱藏 Orchestrator 分頁。請在 Antigravity 中執行上述斜線指令。

### 工作流程 5：Antigravity 規則管理

1. 開啟 **Settings > Antigravity** 並找到 **Global rules**。
2. 編輯並儲存 `deepseek-harness.md` 或 `codex-cli.md`。
3. 獨立的 Poverello Direct Coding 規則可在 **Settings > Antigravity Rules** 預覽並套用 `pov-direct.md`。

## API 金鑰

| 供應商 | 環境變數 |
|---|---|
| Anthropic | `ANTHROPIC_API_KEY` |
| DeepSeek | `DEEPSEEK_API_KEY` |
| GLM / Z.ai | `ZAI_API_KEY` |
| Kimi Code | `KIMI_CODE_API_KEY` |
| Kimi / Moonshot | `MOONSHOT_API_KEY` |
| MiniMax | `MINIMAX_API_KEY` |
| MiMo / Xiaomi | `XIAOMI_API_KEY` |
| OpenRouter | `OPENROUTER_API_KEY` |

---

## 文件

- [版本說明 (NEWS.md)](../NEWS.md) — 各版本功能與實作細節
- [Claude Desktop / Cowork 第三方閘道設定](THIRD_PARTY_INFERENCE.md)
- [Google Antigravity + Poverello MCP 設定](ANTIGRAVITY_MCP.zh-TW.md)
- [設定參考（`config.json`）](CONFIGURATION.md)
- [供應商詳情與推論控制](PROVIDERS.md)
- [開發與驗證指南](DEVELOPMENT.md)

---

## 疑難排解

### 連接埠 4000 已被佔用
```powershell
netstat -ano | findstr :4000
taskkill /PID <PID> /F
```

### 升級後設定還原
重新啟動應用程式以執行資料庫遷移。設定儲存於 `%APPDATA%\Poverello\config.json`。

### MCP 規劃器呼叫失敗
請確認在 **MCP** 標籤中所選供應商已設定 API 金鑰，或已在 Windows 使用者環境變數中匯出（例如 `DEEPSEEK_API_KEY`、`OPENROUTER_API_KEY`）。MCP 執行時不需要啟動第三方閘道。

---

## 授權條款

MIT 授權。請參閱 [LICENSE](../LICENSE)。
