[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

**使用 Claude Code / Claude Desktop 作為開發環境，將推論請求路由至已設定的雲端或本機模型，並管理 Google Antigravity 的外部模型與規則。**

Poverello Coding Tools（應用程式內顯示名稱為 **Poverello**）是一款專為 AI 輔助軟體開發設計的 Windows 配套應用程式，支援五種互補的工作流程：

1. **Claude Code / Claude Desktop 的第三方閘道（3P Gateway）** — 保留 Claude 的程式庫探索、工具使用、檔案編輯與測試執行功能，同時將推論請求路由至第三方供應商。
2. **Claude Code 專用本機 LLM（Ollama）** — 使用本機模型離線執行 Claude Code，零 API 成本。
3. **Google Antigravity 的 MCP 規劃器與審查器** — 透過 `poverello/plan`、`poverello/revise` 和 `poverello/review` MCP 工具，將實作規劃、計畫修訂與實作後審查委派給外部模型。
4. **Antigravity Autocode 與 Skills & Rules** — 在 Antigravity Autocode 中設定外部審查、編碼行為與 Nerf 設定；在 Settings > Antigravity Skills & Rules 管理 Workflow Skills 與 Global Rules。
5. **Codex Autocode 與 Skills & Rules** — 在 Codex Autocode 設定 Codex Workflow Skills 的審查器；在 Settings > Codex Skills & Rules 管理三個原生 Codex Skill 與 Poverello 管理的 AGENTS.md 區域。共用 CLI Tools 在 Settings 中分開管理。

> [!TIP]
> **最新消息：** 請參閱 [NEWS.md（版本說明）](../NEWS.md)，了解各版本功能、實作細節與變更記錄。

下一版目標為 **v0.27.3**（尚未發布）。每個 Autocode 介面固定顯示一個專案-計畫列，並可視需要新增第二列。選取 Markdown 計畫檔時，會將它關聯至包含該檔案的儲存庫。Antigravity 與 Codex 分別儲存各自的選擇。計畫資料夾設定僅在沒有 Active Plan 時作為檔案選擇器的起始資料夾。移除選用的第二列只會清除 Poverello 中的關聯，不會刪除專案或計畫檔。畫面會顯示所選計畫的完整路徑與狀態，並提供獨立的開啟操作。

---

## 五大主要工作流程

### 1. Claude Code / Claude Desktop 搭配第三方閘道

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P Gateway
             ↓
已設定的雲端或本機模型
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

自主工作流程：`/pov-full-cycle`、`/pov-plan-only`、`/pov-build-review`（在 **設定 > Antigravity Skills & Rules** 中安裝與修復；在 **Antigravity Autocode** 中設定 Reviewer Policy）。

**建議工作流程：**

```text
/pov-mcp-plan → 實作與測試 → /pov-mcp-review → 提交
```

### 4. Antigravity Autocode 與 Skills & Rules 管理
```text
Antigravity Autocode tab
  • External Review policy / reviewer configuration
  • Antigravity Nerf Settings / Direct Coding

Settings > Antigravity Skills & Rules
  • Workflow Commands / repair
  • Global Rules

Settings > CLI Tools
  • Shared Node.js / npm and CLI management
```

- External Review 與 Nerf 設定在 Antigravity Autocode 中設定。Settings > Antigravity Skills & Rules 管理 Workflow Skills 與 Global Rules；共用 CLI Tools 在 Settings > CLI Tools 管理。
### 5. Codex Autocode 與 Skills & Rules

在 Settings > Codex Skills & Rules 管理 Codex Workflow Skills（/pov-plan-only、/pov-build-review、/pov-full-cycle），並在 Codex Autocode 選擇審查器與優先審查器。Poverello 只修改 CODEX_HOME/AGENTS.md 中有標記的管理區域，並保留其他使用者指示。

```text
Codex Workflow Skills
       ↓ configured reviewers
Codex 工作階段內審查 / 選用的外部 DeepSeek Harness 或 Gemini CLI
       ↓
Codex Autocode review settings

Settings > Codex Skills & Rules
  • /pov-plan-only, /pov-build-review, /pov-full-cycle
  • Poverello-managed instructions in CODEX_HOME/AGENTS.md
Settings > CLI Tools
  • Shared Codex CLI, Gemini CLI, DeepSeek Harness, Node.js / npm status
```


Gemini CLI 是可在 Settings > CLI Tools 啟用的選用審查器。啟用本身不會選取該審查器，也不會啟動審查。若 Windows 上無法驗證唯讀 sandbox，Gemini 審查即無法使用。安裝狀態與驗證狀態分開顯示。Node.js/npm 會作為前置條件偵測，但 Poverello 不會自動安裝。自 v0.26.1 起，非互動式 CLI 與 Node.js/npm 狀態檢查不會開啟主控台視窗，並在切換分頁時重用短效快取結果，以避免重複啟動程序。`0xC0000142` 等程序初始化錯誤會明確回報。使用者主動啟動的登入仍使用互動式終端機。

自 v0.26.2 起，優先審查器僅用於裁決有效判定之間的分歧，不能覆蓋失敗或未完成的審查，也不能忽略尚未解決的阻擋問題。新增診斷記錄，以及含備份與確認步驟的歷史 Codex Skill 範本安全更新。DeepSeek Harness 的唯讀設定與工具限制無法證明 Windows 作業系統層級的檔案系統或網路隔離；若無法驗證該安全界線，DeepSeek 審查將維持不可用。

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
2. 選擇 **MCP** 標籤頁（或 **設定 > Antigravity MCP**），並在 **設定 > Antigravity MCP > MCP Plan 詳細設定** 中設定您的模型。
3. 在 Antigravity 的 MCP 設定中以 `["--mcp-server"]` 參數註冊 `anthro-bridge.exe`，伺服器鍵為 `poverello`（或在 Poverello 中點擊 **自動設定**）。
4. 使用 `/pov-mcp-plan` 設計計畫、`/pov-mcp-revise` 更新計畫，以及 `/pov-mcp-review` 在提交前審查實作成果。
5. 請參閱完整的 [Antigravity MCP 設定指南](ANTIGRAVITY_MCP.zh-TW.md)。

### 工作流程 4：Antigravity Autocode 與 Skills & Rules
1. 在 **Antigravity Autocode** 中設定 External Review 與 Antigravity Nerf Settings。所選策略控制工作流程 Skill 是否呼叫審查器。
2. 在 **Settings > Antigravity Skills & Rules** 管理 Antigravity Workflow Skills 與 Global Rules。修復 Skill 需要確認並保留備份。Nerf 設定位於 Antigravity Autocode。共用 CLI Tools 在 **Settings > CLI Tools** 管理。
3. Auto-Converge 仍隱藏，Production dispatch 停用，Stage 2 隔離未驗證。
### 工作流程 5：Codex Autocode 與 Skills & Rules
1. 在 Settings > CLI Tools 檢查 Node.js/npm 與 CLI 狀態。Poverello 不會自動安裝 Node.js。
2. 在 Settings > Codex Skills & Rules 管理 /pov-plan-only、/pov-build-review、/pov-full-cycle，以及 CODEX_HOME/AGENTS.md 中由 Poverello 管理的區域。
3. 在 Codex Autocode 選擇審查器與優先審查器。只有在 Settings > CLI Tools 啟用 Gemini CLI 後才會顯示；啟用不會自動選取。Windows 唯讀 sandbox 未驗證時，Gemini 審查無法使用。

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
