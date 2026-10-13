[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

**Claude Code / Claude Desktop をコーディングハーネスとして使用し、推論をサードパーティ LLM API にルーティングしながら、外部モデルを Google Antigravity のプランナー・レビュアーとして活用します。**

Poverello Coding Tools（アプリ内表示名 **Poverello**）は、AI 支援ソフトウェア開発のための Windows コンパニオンアプリケーションです。4 つの補完的なワークフローをサポートしています。

1. **Claude Code / Claude Desktop 向け 3P ゲートウェイ** — Claude のリポジトリ探索、ツール使用、ファイル編集、テスト実行はそのままに、推論をサードパーティプロバイダーにルーティングします。
2. **Claude Code 向けローカル LLM（Ollama）** — ローカルモデルを使って、Claude Code をオフラインで実行できます。
3. **Google Antigravity 向け MCP プランナー & レビュアー** — `anthro-bridge/plan` および `anthro-bridge/review` MCP ツールを通じて、実装計画と実装後レビューを外部モデルに委任します。
4. **Antigravity ルール管理** — Google Antigravity 向けの軽量 Direct Coding ルール（`pov-direct.md`）を設定・プレビューし、安全に同期します。

---

## 4 つのメインワークフロー

### 1. Claude Code / Claude Desktop と 3P ゲートウェイ

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P Gateway
             ↓
DeepSeek / Kimi Code / OpenRouter / MiniMax / MiMo
```

- **ハーネスとモデルの分離**: Claude のエージェント型ツールを活かしながら、推論をサードパーティプロバイダーにルーティングします。
- **動的マルチプロファイルルーティング**: GUI からアクティブなプロバイダー、OpenRouter プロファイル、モデルルートを切り替えられます。
- **セットアップガイド**: [Claude Desktop / Cowork 3P ゲートウェイ セットアップ](THIRD_PARTY_INFERENCE.ja.md)

### 2. Claude Code と専用ローカル LLM（Ollama）

Claude Code CLI をローカルの Ollama モデルへ直接接続できます。この経路は 3P Gateway や Antigravity MCP から独立しています。

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
- **オプトイン型 Planner–Reviewer Auto-Convergence**: Orchestratorで有効にすると、計画候補を生成し、独立したReviewerが検査する上限付きワークフローを実行します。スコープ内として承認された変更は自動追記される場合がありますが、新しいprimary planには必ずユーザーの明示的な確認が必要です。実モデルの挙動はプロバイダーやアダプターによって異なり、すべての組み合わせを網羅的にスモークテストしたわけではありません。

実行ファイル名とMCPツール名には、互換性維持のため`anthro-bridge`が残っています。

**Antigravity グローバルコマンド:**

- **`/anthro-plan`** — 設定済み外部モデルに実装計画を委任します。
- **`/anthro-revise`** — 新しいフィードバックや制約に基づいて既存の計画を修正します。
- **`/anthro-review`** — コミット前に承認済み計画に対して完成した実装をレビューし、明示的に READY / NOT READY の判定を行います。

**推奨ワークフロー:**

```text
/anthro-plan → 実装 & テスト → /anthro-review → コミット
```

### 4. Antigravity ルール管理（Poverello Direct Coding）

```text
Poverello GUI（"Antigravity Rules" タブ）
          ↓（検証付き適用 / ロールバック）
%USERPROFILE%\.gemini\config\rules\pov-direct.md
          ↓
Google Antigravity がスコープを無闇に広げず
指示通りの直接実装ルールに従う
```

- **スコープと方針の制御**: Antigravity が局所的な修正タスクに対して自律的に計画立案や不要な大規模リファクタリングを広げないよう、軽量な Direct Coding ルールを構成します。
- **単一ファイルの整合性保護**: YAML の `trigger` 条件および管理ブロック（`<!-- pov-direct:managed:begin -->` ... `<!-- pov-direct:managed:end -->`）のみを安全に更新し、ユーザー独自の frontmatter、説明文、Skill インクルードを完全に維持します。
- **安全な適用保証**: 置換直前の SHA-256 検証、外部削除競合の検出、重複しない保持バックアップ、読み戻し検証失敗時のロールバック、旧 sidecar フォルダの存在チェックにより、安全に書き込みます。

---

## 対応プロバイダー

| プロバイダー | 接続方式 | 対応ファミリー | 推論コントロール |
|---|---|---|---|
| **DeepSeek** | Direct API | DeepSeek V4.1 Flash, V4 Pro 0813 | Normal / Low / High / Max |
| **GLM (Z.ai)** | Direct API | GLM-5.3, GLM-5.3-Flash, GLM-5.3-FlashX, GLM-5.2 | GLM-5.3: Low / High / Max · GLM-5.2: Normal / Thinking |
| **Kimi Code** | Direct API | kimi-for-coding, kimi-for-coding-highspeed | Thinking モード |
| **MiniMax** | Direct API | MiniMax M3, M2.7 | モデル固有 |
| **Kimi / Moonshot** | Direct API | Kimi K2.x, Kimi K3 | Thinking / 推論努力度 |
| **MiMo / Xiaomi** | Direct API | MiMo V2.6 Flash, Pro, Pro-UltraSpeed（V2.5 後方互換あり） | Normal / Thinking |
| **Ollama Local** | ローカルループバック | Gemma 4, Qwen 2.5, Llama 3.3, DeepSeek-R1 | Thinking (1024予算) / Disabled |
| **OpenRouter** | マルチプロファイルゲートウェイ | 下記 OpenRouter セクション参照 | モデル固有 / プロファイル固有 |

### DeepSeek（ダイレクト）

組み込みの **Direct DeepSeek** プリセット: Opus 5 → V4.1 Flash / Max · Sonnet 5 → V4.1 Flash / High · Haiku 4.5 → V4.1 Flash / Low。

- `deepseek-v4.1-flash` — 現行フラッグシップ推論モデル（$0.27 / 1M 入力 · $1.10 / 1M 出力）。
- `deepseek-v4-pro-0813` — 拡張推論なしの高品質ベースラインモデル（$0.27 / 1M 入力 · $1.10 / 1M 出力）。

### GLM / Z.ai（ダイレクト）

Z.ai 公式の Anthropic Messages 互換 API（`https://api.z.ai/api/anthropic/v1/messages`）を直接利用します。

- **専用 API キー**: `ZAI_API_KEY`（従量課金）を使用します。Coding Plan トークンは非対応（送信前にフェイルクローズ）です。
- **モデル**:
  - `glm-5.3` — 常時思考のフラッグシップ推論モデル。
  - `glm-5.3-flash` — 常時思考の高速推論モデル。
  - `glm-5.3-flashx` — 常時思考の極低遅延推論モデル。
  - `glm-5.2` — Normal / Thinking を切り替え可能な汎用コーディングモデル。
- **推論コントロール**: GLM-5.3 ファミリーは推論努力度（`low`, `high`, `max`）を選択可能。GLM-5.2 は Normal / Thinking のトグル切り替えに対応。
- **厳格な出力検証**: 思考ブロックのみで最終テキストが空の場合（思考中に `max_tokens` に達した場合など）、Orchestrator・MCP ともに成功扱いせず明示的なプロバイダーエラーとしてフェイルクローズし、空計画の実行や出力漏洩を防ぎます。

### Kimi Code（ダイレクト）

Moonshot Kimi とは別の、コーディング専用 API（`KIMI_CODE_API_KEY`）:

- `kimi-for-coding` — フルクオリティのコーディングモデル。
- `kimi-for-coding-highspeed` — 低レイテンシ版。

### MiMo / Xiaomi（ダイレクト）

組み込みの **Direct MiMo** プリセットルート:
- Opus 5 → `mimo-v2.6-pro` / Thinking
- Sonnet 5 → `mimo-v2.6-pro` / Normal
- Haiku 4.5 → `mimo-v2.6-flash` / Thinking
- デフォルトモデル: `mimo-v2.6-flash`

モデル: `mimo-v2.6-flash`、`mimo-v2.6-pro`、`mimo-v2.6-pro-ultraspeed`（選択可能）。3 つすべてが 100 万トークンのコンテキストウィンドウとネイティブマルチモーダル機能（テキスト、画像、動画）に対応。

**Normal / Thinking**: MiMo はシンプルな Normal/Thinking の切り替えのみ — 推論努力度レベルはありません。

**V2.5 後方互換性**: 保存済みの `mimo-v2.5`、`mimo-v2.5-pro`、`mimo-v2.5-pro-ultraspeed` ルートはそのまま機能し続けます。変更されていないレガシーデフォルトは起動時に自動的に V2.6 へ移行します。

### 専用ローカル LLM（Ollama Local）

**Claude Code CLI 専用** のローカル推論バックエンド：
- エンドポイント: `http://127.0.0.1:11434/v1`（ネイティブ Anthropic messages ループバック）
- ループバック `/api/tags` からインストール済みモデルを自動検出
- エイリアスごとの思考予算（1024トークン）およびコンテキスト長（`num_ctx`）設定
- 詳細は [Ollama Local ガイド](OLLAMA_LOCAL.ja.md) をご覧ください

### OpenRouter

複数の名前付きプロファイルをサポート。OpenAI の全モデルカタログ（単一ドロップダウン）:

| モデル ID | 表示名 |
|---|---|
| `openai/gpt-6-astra` | GPT-6 Astra |
| `openai/gpt-6-astra-pro` | GPT-6 Astra Pro |
| `openai/gpt-astra-latest` | GPT Astra Latest |
| `openai/gpt-5.6-sol` | GPT-5.6 Sol |
| `openai/gpt-5.6-sol-pro` | GPT-5.6 Sol Pro |
| `openai/gpt-5.6-terra` | GPT-5.6 Terra |
| `openai/gpt-5.6-terra-pro` | GPT-5.6 Terra Pro |
| `openai/gpt-5.6-luna` | GPT-5.6 Luna |
| `openai/gpt-5.6-luna-pro` | GPT-5.6 Luna Pro |

**GPT-6 Astra**: コンテキスト 1.05M · 推論努力度: `low / medium / high / xhigh / max`。
**GPT-6 Astra Pro**: コンテキスト 1.05M · 常時 Pro 推論（`reasoning.mode = pro`）、ユーザーによる努力度選択不可。
**GPT Astra Latest**: 最新の Astra ファミリーモデルを追跡するエイリアス。

組み込みの **OpenRouter: chatGPT** プリセット: Opus 5 → GPT-6 Astra / max · Sonnet 5 → GPT-6 Astra / high · Haiku 4.5 → GPT-6 Astra / medium。

その他利用可能: **OpenRouter: Gemini**（Gemini 3.8 Flash · 推論努力度 `low / medium / high`）、**OpenRouter: Poolside**、**OpenRouter: Tencent**、**OpenRouter: InclusionAI**、**OpenRouter: StepFun**。

---

## モデル料金（v0.23.0 時点）

| モデル | 入力 | 出力 |
|---|---|---|
| DeepSeek V4.1 Flash | \$0.27 / 1M | \$1.10 / 1M |
| DeepSeek V4 Pro 0813 | \$0.27 / 1M | \$1.10 / 1M |
| GPT-6 Astra / Astra Pro / Astra Latest | \$10 / 1M | \$50 / 1M |
| GPT-5.6 Sol / Terra / Luna | \$5 / 1M | \$25 / 1M |
| GPT-5.6 Sol Pro / Terra Pro / Luna Pro | \$5 / 1M | \$25 / 1M |
| Gemini 3.8 Flash (OpenRouter) | \$0.75 / 1M | \$3.75 / 1M |
| MiMo-V2.6-Flash | \$0.14 / 1M | \$0.28 / 1M |
| MiMo-V2.6-Pro | \$0.435 / 1M | \$0.87 / 1M |
| MiMo-V2.6-Pro-UltraSpeed | \$4.35 / 1M | \$8.70 / 1M |

---

## インストール

[Releases](https://github.com/soheidon/poverello-coding-tools/releases) ページから最新の公開済み Windows インストーラーをダウンロードして実行してください。

Windowsインストーラーは8言語に対応しています。Poverello v0.24.1は既存のアプリケーションIDを維持し、Anthro Bridgeからの上書きアップグレード経路を備えています。初回起動時には、`%APPDATA%\Anthro Bridge`（開発版では`%APPDATA%\Anthro Bridge Dev`）の設定と実行データを対応するPoverelloデータフォルダーへコピーし、元のフォルダーを復旧用コピーとして保持する設計です。実際のWindows環境でのインストール、データ移行、アップグレード、再インストール、キャンセル、アンインストールは未検証のため、アップグレード前に重要なデータをバックアップしてください。

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
2. **MCP** タブを選択し、**Settings > Antigravity > MCP Plan Settings** でモデルを設定します。
3. Antigravity の MCP 設定で `anthro-bridge.exe` を `["--mcp-server"]` 付きで登録します（または Poverello の **Configure Automatically** をクリックします）。
4. `/anthro-plan` で計画を作成し、`/anthro-revise` で計画を更新し、`/anthro-review` でコミット前に実装をレビューします。
5. 完全な [Antigravity MCP セットアップガイド](ANTIGRAVITY_MCP.ja.md) に従ってください。

### ワークフロー 4: Antigravity ルール管理（Poverello Direct Coding）

1. Poverello の上部タブから **Antigravity Rules** を開きます。
2. 希望するルールモードを選択し、生成プレビューを確認します。
3. **Apply Settings** をクリックして `%USERPROFILE%\.gemini\config\rules\pov-direct.md` へ安全に書き込みます。
4. 既存のカスタムコメント、frontmatter、バックアップは安全に保持されます。

---

## API キー

| プロバイダー | 環境変数 |
|---|---|
| DeepSeek | `DEEPSEEK_API_KEY` |
| GLM / Z.ai | `ZAI_API_KEY` |
| Kimi Code | `KIMI_CODE_API_KEY` |
| Kimi / Moonshot | `MOONSHOT_API_KEY` |
| MiniMax | `MINIMAX_API_KEY` |
| MiMo / Xiaomi | `XIAOMI_API_KEY` |
| OpenRouter | `OPENROUTER_API_KEY` |

---

## ドキュメント

- [Claude Desktop / Cowork 3P ゲートウェイ セットアップ](THIRD_PARTY_INFERENCE.ja.md)
- [Google Antigravity + Poverello MCP セットアップ](ANTIGRAVITY_MCP.ja.md)
- [設定リファレンス（`config.json`）](CONFIGURATION.md)
- [プロバイダー詳細 & 推論コントロール](PROVIDERS.md)
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
