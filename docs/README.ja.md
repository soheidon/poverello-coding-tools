[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

**Claude Code / Claude Desktop を開発環境として使い、推論を設定済みのクラウドまたはローカルモデルへルーティングし、Google Antigravity の外部モデルとルールを管理します。**

Poverello Coding Tools（アプリ内表示名 **Poverello**）は、AI 支援ソフトウェア開発のための Windows コンパニオンアプリケーションです。5 つの主要ワークフローをサポートしています。

1. **Claude Code / Claude Desktop 向け 3P ゲートウェイ** — Claude のリポジトリ探索、ツール使用、ファイル編集、テスト実行はそのままに、推論をサードパーティプロバイダーにルーティングします。
2. **Claude Code 向けローカル LLM（Ollama）** — ローカルモデルを使って、Claude Code をオフラインで実行できます。
3. **Google Antigravity 向け MCP プランナー & レビュアー** — `poverello/plan`、`poverello/revise`、`poverello/review` MCP ツールを通じて、実装計画と実装後レビューを外部モデルに委任します。
4. **Antigravity Autocode と Skills & ルール** — Antigravity Autocodeタブで外部レビューとコーディング動作、Nerf設定を管理します。設定 > Antigravity Skills & RulesではWorkflow SkillsとGlobal Rulesを管理します。
5. **Codex Autocode と Skills & ルール** — Codex AutocodeでCodex Workflow Skillsのレビュアーを設定し、設定 > Codex Skills & Rulesで3つのCodex SkillとPoverello管理領域のAGENTS.mdを管理します。共通CLIは設定 > CLI Toolsで管理します。

> [!TIP]
> **新実装の詳細はこちら**: バージョンごとの新機能や詳細な実装仕様・変更履歴については、[NEWS.md (リリースノート)](../NEWS.md) をご覧ください。

現行リリースは **v0.27.2** です。Antigravity AutocodeとCodex Autocodeでは、それぞれのSkills & Rules設定からプランフォルダーの場所を個別に設定できます。パスの直接編集、フォルダー選択、リセットに対応し、未設定時にアプリの作業ディレクトリへ暗黙にフォールバックしません。Markdown（.md）のプランファイルを直接選択してエディターで開けます。Active Planの絶対パスと状態を表示し、ファイル選択ダイアログはそのプランのフォルダーから開きます。

---

## 5 つのメインワークフロー

### 1. Claude Code / Claude Desktop と 3P ゲートウェイ

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P Gateway
             ↓
設定済みのクラウドまたはローカルモデル
```

- **ハーネスとモデルの分離**: Claude のエージェント型ツールを活かしながら、推論をサードパーティプロバイダーや Anthropic 公式エンドポイントにルーティングします。
- **動的マルチプロファイル & マルチプロバイダーゲートウェイ**: GUI からアクティブプロバイダー、OpenRouter プロファイル、または標準ルートごとに異なるプロバイダーを割り当てるマルチプロバイダーゲートウェイプロファイルを切り替えられます。
- **セットアップガイド**: [Claude Desktop / Cowork 3P ゲートウェイ セットアップ](THIRD_PARTY_INFERENCE.ja.md)

### 2. Claude Code と専用ローカル LLM（Ollama）

```text
Claude Code CLI
      ↓ (Loopback ANTHROPIC_BASE_URL)
Ollama Local (127.0.0.1:11434/v1)
      ↓
Gemma 4 / Qwen / Llama 3 / DeepSeek-R1 (Local On-Device)
```

- **完全無料・オフライン・プライベート**: API キー、利用上限、クラウドデータ送信なしで Claude Code を実行できます。
- **専用ルーティング & モデル自動検出**: Gateway と Ollama（`active_route: "ollama"`）を切り替え、`/api/tags` からローカルモデルを自動検出します。
- **思考 & コンテキスト長調整**: エイリアスごとの思考トークンバジェット（1024トークン）およびコンテキスト長上書き（`num_ctx`）を完全サポート。
- **完全な隔離性**: Claude Desktop、Cowork on 3P、Google Antigravity MCP は独立してクラウドモデルを引き続き利用できます。
- **セットアップガイド**: [Ollama Local セットアップ](OLLAMA_LOCAL.ja.md)

### 3. Antigravity と MCP プランナー & レビュアー

```text
Antigravity
    ↓ stdio
anthro-bridge.exe --mcp-server
    ↓
設定済み外部モデル (プランナー / レビュアー)
    ↓
実装計画 / レビュー判定
    ↓
Antigravity がサブスクリプションの処理能力を使って
実装・テストを実行
```

- **計画と実行の分離**: 外部モデルが高レベルの計画またはレビュー判定を生成し、Antigravity のサブスクリプション処理能力がトークン集約的なコード編集を実行します。
- **ライブ GUI 設定**: プランナーまたはレビュアーのプロバイダー、モデル、推論努力度を切り替えると、次回の呼び出し時から即座に反映されます。
- **セットアップガイド**: [Google Antigravity + Poverello MCP セットアップ](ANTIGRAVITY_MCP.ja.md)

実行ファイル名とMCPツール名には、互換性維持のため`anthro-bridge`が残っています。

**Antigravity グローバルコマンド:**

- **`/pov-mcp-plan`** (旧 `/anthro-plan`) — 設定済み外部モデルに実装計画を委任します。
- **`/pov-mcp-revise`** (旧 `/anthro-revise`) — 新しいフィードバックや制約に基づいて既存の計画を修正します。
- **`/pov-mcp-review`** (旧 `/anthro-review`) — コミット前に承認済み計画に対して完成した実装をレビューし、明示的に READY / NOT READY の判定を行います。

Antigravity Workflow Skills: `/pov-full-cycle`、`/pov-plan-only`、`/pov-build-review`（**設定 > Antigravity Skills & Rules** でインストール・修復し、Reviewer Policyは **Antigravity Autocode** で設定）。 CodexのネイティブSkillは、同じコマンド名でも設定 > Codex Skills & Rulesから別個に管理します。

**推奨ワークフロー:**

```text
/pov-mcp-plan → 実装 & テスト → /pov-mcp-review → コミット
```

### 4. Antigravity Autocode と Skills & ルール管理
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

- External ReviewとNerf設定はAntigravity Autocodeで管理します。設定 > Antigravity Skills & RulesではWorkflow SkillsとGlobal Rulesを管理し、共通CLI Toolsは設定 > CLI Toolsで管理します。
### 5. Codex Autocode と Skills & ルール管理

Codex Workflow Skills（/pov-plan-only、/pov-build-review、/pov-full-cycle）を設定 > Codex Skills & Rulesで管理し、Codex AutocodeでReviewerと優先Reviewerを設定します。PoverelloはCODEX_HOME/AGENTS.mdの管理領域だけを更新し、他のユーザー記述を保持します。

```text
Codex Workflow Skills
       ↓ configured reviewers
Codexの同一セッション内レビュー / 任意の外部DeepSeek HarnessまたはGemini CLI
       ↓
Codex Autocode review settings

Settings > Codex Skills & Rules
  • /pov-plan-only, /pov-build-review, /pov-full-cycle
  • Poverello-managed instructions in CODEX_HOME/AGENTS.md
Settings > CLI Tools
  • Shared Codex CLI, Gemini CLI, DeepSeek Harness, Node.js / npm status
```


Gemini CLIは設定 > CLI Toolsで有効にできる任意Reviewerです。有効化だけでは選択もレビュー実行も行いません。Windowsで読み取り専用sandboxを確認できない場合、Geminiレビューは利用できません。CLIの導入状態と認証状態は別々に表示され、Node.js/npmは前提条件として検出されますが、自動インストールされません。v0.26.1では、非対話のCLI・Node.js/npm状態確認でコンソールを表示せず、タブ切替時の結果を短時間キャッシュして重複起動を抑えます。`0xC0000142`などのプロセス初期化エラーは明示し、ユーザーが開始するログイン端末は引き続き表示します。

v0.26.2では、優先Reviewerは有効な判定が食い違う場合にのみ適用し、失敗・未完了のレビューや未解決のBlocking指摘を上書きしません。レビュー診断ログと、バックアップ・確認を伴う旧標準Codex Skillの安全な更新に対応します。DeepSeek Harnessのread-only設定とツール制限は、Windows OSによるファイルシステム・ネットワーク隔離を証明するものではありません。安全境界を確認できない場合、DeepSeekレビューは利用不可として扱います。

## インストール

[Releases](https://github.com/soheidon/poverello-coding-tools/releases) ページから最新の公開済み Windows インストーラーをダウンロードして実行してください。

Windowsインストーラーは8言語に対応しています。Poverelloは既存のアプリケーションIDを維持し、Anthro Bridgeからの上書きアップグレードをサポートします。初回起動時には、`%APPDATA%\Anthro Bridge`（開発版では`%APPDATA%\Anthro Bridge Dev`）の設定と実行データを対応するPoverelloデータフォルダーへコピーし、元のフォルダーを復旧用コピーとして保持する設計です。実際のWindows環境でのインストール、データ移行、アップグレード、再インストール、キャンセル、アンインストールは未検証のため、アップグレード前に重要なデータをバックアップしてください。

---

## クイックスタート

### ワークフロー 1: Claude Code / Claude Desktop 向け 3P ゲートウェイ

1. Poverello の **Settings > API Key** を開き、使用するプロバイダーの API キーを設定します。
2. ダッシュボードでプロバイダーまたは OpenRouter プロファイルを選択します。
3. **Start Gateway** をクリックします（`http://127.0.0.1:4000` で起動）。
4. Claude Code または Claude Desktop を接続します。
   - **Claude Code**: Settings の **Copy Claude Code launch command** をクリックし、コマンドを PowerShell に貼り付けます。
   - **Claude Desktop / Cowork**: [Claude Desktop 3P セットアップガイド](THIRD_PARTY_INFERENCE.ja.md) に従ってください。

### ワークフロー 2: Claude Code 向け専用ローカルLLM（Ollama）

1. Ollama がローカルで実行中であることを確認します（`http://127.0.0.1:11434`）。
2. Poverello のダッシュボードで **Ollama Local** を選択するか、**Settings > Claude Code / Local LLMs** でモデルを設定します。
3. **Copy Claude Code launch command** をクリックし、コマンドをPowerShellに貼り付けます。
4. Claude Code はローカルの Ollama インスタンスへ直接接続します。

### ワークフロー 3: Google Antigravity 向け MCP プランナー & レビュアー

1. Poverello で、選択したプランナー / レビュアーモデルの API キーを設定します。
2. **MCP** タブ（または **設定 > Antigravity MCP**）を選択し、**設定 > Antigravity MCP > MCP Plan 詳細設定** でモデルを設定します。
3. Antigravity の MCP 設定で `anthro-bridge.exe` を `["--mcp-server"]` 付きで登録します（または Poverello の **Configure Automatically** をクリックします）。
4. `/pov-mcp-plan` で計画を作成し、`/pov-mcp-revise` で計画を更新し、`/pov-mcp-review` でコミット前に実装をレビューします。
4. 完全な [Antigravity MCP セットアップガイド](ANTIGRAVITY_MCP.ja.md) に従ってください。

### ワークフロー 4: Antigravity Autocode と Skills & ルール管理
1. **Antigravity Autocode**タブでExternal ReviewとAntigravity Nerf Settingsを設定します。選択ポリシーがWorkflow Skillのレビュアー利用を制御します。
2. **設定 > Antigravity Skills & Rules**ではAntigravity Workflow SkillsとGlobal Rulesを管理します。Skill修復には確認が必要で、バックアップが保持されます。Nerf設定はAntigravity Autocodeにあります。共通CLI Toolsは**設定 > CLI Tools**で管理します。
3. Auto-Convergeは非表示、本番ディスパッチは無効、Stage 2隔離は未検証です。
### ワークフロー 5: Codex Autocode と Skills & ルール管理
1. 設定 > CLI ToolsでNode.js/npmとCLIの状態を確認します。Node.jsはPoverelloから自動インストールされません。
2. 設定 > Codex Skills & Rulesで/pov-plan-only、/pov-build-review、/pov-full-cycleを管理し、CODEX_HOME/AGENTS.mdのPoverello管理領域を管理します。
3. Codex AutocodeでReviewerと優先Reviewerを選択します。Gemini CLIは設定 > CLI Toolsで有効化した場合のみ候補に現れ、有効化だけでは選択されません。Windowsの読み取り専用sandboxが未確認ならGeminiレビューは利用できません。

## ドキュメント
- [プロバイダー詳細と推論制御](PROVIDERS.md)


- [リリースノート (NEWS.md)](../NEWS.md) — 新実装の詳細およびバージョン変更履歴
- [Claude Desktop / Cowork 3P ゲートウェイ セットアップ](THIRD_PARTY_INFERENCE.ja.md)
- [Google Antigravity + Poverello MCP セットアップ](ANTIGRAVITY_MCP.ja.md)
- [設定リファレンス（`config.json`）](CONFIGURATION.md)
- [開発 & 検証ガイド](DEVELOPMENT.md)

---

## トラブルシューティング

### ポート 4000 が使用中の場合
```powershell
netstat -ano | findstr :4000
taskkill /PID <PID> /F
```

### アップグレード後に設定が元に戻る場合
マイグレーションが実行されるよう、アプリケーションを再起動してください。設定は `%APPDATA%\Poverello\config.json` に保存されています。

### MCP プランナーの呼び出しが失敗する場合
**MCP** タブで選択したプロバイダーの API キーが設定されているか、または Windows ユーザー環境変数（例: `DEEPSEEK_API_KEY`、`OPENROUTER_API_KEY`）にエクスポートされていることを確認してください。MCP の利用に 3P ゲートウェイの起動は不要です。

---

## ライセンス

MIT ライセンス。[LICENSE](../LICENSE) を参照してください。
