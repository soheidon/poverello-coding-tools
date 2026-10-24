[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

**Claude Code / Claude Desktop をコーディングハーネスとして使用し、推論をサードパーティ LLM API にルーティングします。対応済みのマルチエージェント計画・レビューとコスト見積もりを管理し、Google Antigravity の外部モデルやルールを統合管理します。**

Poverello Coding Tools（アプリ内表示名 **Poverello**）は、AI 支援ソフトウェア開発のための Windows コンパニオンアプリケーションです。5 つの主要ワークフローをサポートしています。

1. **Claude Code / Claude Desktop 向け 3P ゲートウェイ** — Claude のリポジトリ探索、ツール使用、ファイル編集、テスト実行はそのままに、推論をサードパーティプロバイダーにルーティングします。
2. **Claude Code 向けローカル LLM（Ollama）** — ローカルモデルを使って、Claude Code をオフラインで実行できます。
3. **Google Antigravity 向け MCP プランナー & レビュアー** — `poverello/plan`、`poverello/revise`、`poverello/review` MCP ツールを通じて、実装計画と実装後レビューを外部モデルに委任します。
4. **マルチエージェント開発オーケストレーター** — CLI／APIエージェントを横断して、対応済みの上限付きプラン・レビュー・ワークフローを実行し、正規化トークン使用量とレートスケジュール検証に基づくコスト見積もり（整数ナノドル）を追跡して監査可能なディスパッチジャーナルを維持します。
5. **Antigravity ルール管理** — Google Antigravity 向けの軽量 Direct Coding ルール（`pov-direct.md`）を設定・プレビューし、安全に同期します。

> [!TIP]
> **新実装の詳細はこちら**: バージョンごとの新機能や詳細な実装仕様・変更履歴については、[NEWS.md (リリースノート)](../NEWS.md) をご覧ください。

---

## 5 つのメインワークフロー

### 1. Claude Code / Claude Desktop と 3P ゲートウェイ

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P Gateway
             ↓
Anthropic Direct / DeepSeek / Kimi Code / OpenRouter / MiniMax / MiMo / GLM
```

- **ハーネスとモデルの分離**: Claude のエージェント型ツールを活かしながら、推論をサードパーティプロバイダーや Anthropic 公式エンドポイントにルーティングします。
- **動的マルチプロファイル & マルチプロバイダーゲートウェイ**: GUI からアクティブプロバイダー、OpenRouter プロファイル、または標準ルートごとに異なるプロバイダーを割り当てるマルチプロバイダーゲートウェイプロファイルを切り替えられます。
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

実行ファイル名とMCPツール名には、互換性維持のため`anthro-bridge`が残っています。

**Antigravity グローバルコマンド:**

- **`/pov-plan`** (旧 `/anthro-plan`) — 設定済み外部モデルに実装計画を委任します。
- **`/pov-revise`** (旧 `/anthro-revise`) — 新しいフィードバックや制約に基づいて既存の計画を修正します。
- **`/pov-review`** (旧 `/anthro-review`) — コミット前に承認済み計画に対して完成した実装をレビューし、明示的に READY / NOT READY の判定を行います。

**推奨ワークフロー:**

```text
/pov-plan → 実装 & テスト → /pov-review → コミット
```

### 4. マルチエージェント開発オーケストレーター（対応ワークフロー & コスト見積もり）

```text
開発タスク / 要件
        ↓
+-------------------------------------------------------------+
| Poverello Orchestrator                                      |
|                                                             |
|  [Planner プロファイル]  ⇄  [Reviewer プロファイル]         |
|  （計画立案・修正）          （検証・監査・判定）            |
|                                                             |
|  • 上限付きプラン・レビュー（ターン上限）                  |
|  • 完全なディスパッチ履歴（%APPDATA%\Poverello\runs\*.json）|
|  • ナノドル精度コスト算出 & レートスケジュール検証          |
|  • フェイルクローズ表示バッジ（Complete / Partial / N/A）   |
+-------------------------------------------------------------+
        ↓
承認済み計画 / 監査ログ
        ↓
コーディングハーネスによる別途・ユーザー管理の実装
```

- **上限付き Planner–Reviewer プラン・レビュー**: プランナーとレビュアーに独立した LLM プロファイルを割り当て、対応済みの計画・レビュー作業を設定された上限内で実行します。
- **安全不変条件 & 人間による確認**: 新しいプライマリ計画の策定には必ず人間の明示的な確認が必要です。実装と検証は別途、ユーザーが選んだコーディングハーネスで行います。
- **監査可能なディスパッチジャーナル**: `%APPDATA%\Poverello\runs\<run_id>.json` には実行設定、任意のタスクプロンプト、ディスパッチのメタデータとペイロードダイジェスト、取得できた正規化使用量・コスト見積もり、Planner–Reviewer収束監査メタデータが記録されます。アダプターへ送る完全なプロンプトエンベロープやモデル応答本文はjournalには保存されません。
- **正規化使用量 & レート検証済みコスト見積もり**:
  - アダプターが対応形式の使用量を返した場合、`input_tokens`、`output_tokens`、`raw_total_tokens`、`cache_read_tokens`、`cache_write_tokens`（シリアライズ名 `cacheWriteTokens`）を正規化して記録します。未取得・未対応の使用量はゼロではなく不明として扱います。プロバイダー固有のキャッシュ作成使用量は `cache_write_tokens` に対応付けられます。
  - 見積額はすべて整数ナノドル（`u64 amount_nanos`、1 USD = $10^9$ nanos、JSON シリアライズ時は 10 進数文字列）として扱います。これは対応するレートスケジュールに基づく見積もりであり、プロバイダーの請求書や実際の請求額ではありません。浮動小数点演算は使用しません。
  - レートスケジュールの出所検証: スケジュール ID、プロバイダー、モデル名、通貨が検証済みスケジュール（同梱の Anthropic Opus 5.5 および Sonnet 5.5 レート等）と完全一致することを要求し、不一致や未登録は未算定（unpriced）としてフェイルクローズします。
  - 現在の同梱コスト見積もりの対象は、Anthropic Direct Opus 5.5 / Sonnet 5.5 でキャッシュ各項目が明示的にゼロの、条件を満たす Planner–Reviewer収束ディスパッチに限定されます。他のプロバイダー、ルート、実行経路は未算定／利用不可であり、コストゼロを意味しません。
  - UI 上のステータスバッジにより、見積もりの対象範囲が `Complete`、妥当な部分計が `Partial`、または見積もり不可が `Unavailable` であることを識別できます。
  - `list_journals()` 呼び出し時に不変ディスパッチ記録から動的に再集計するため、古い保存データによる検証迂回を防止します。

### 5. Antigravity ルール管理（Poverello Direct Coding）

```text
Poverello GUI（設定 > Antigravity Rules）
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
| **Anthropic** | Direct API | Claude Opus 5.5, Sonnet 5.5, Haiku 4.5, 3.7 Sonnet | Adaptive thinking / 推論努力度 |
| **DeepSeek** | Direct API | `deepseek-flash`（V4.1 Flash 1017）、`deepseek-v4-pro`（V4 Pro 0813） | Normal / Thinking · Low / High / Max |
| **GLM (Z.ai)** | Direct API | GLM-5.3, GLM-5.3-Flash, GLM-5.3-FlashX, GLM-5.2 | GLM-5.3: Low / High / Max · GLM-5.2: Normal / Thinking |
| **Kimi Code** | Direct API | kimi-for-coding, kimi-for-coding-highspeed | Thinking モード |
| **MiniMax** | Direct API | MiniMax M3, M2.7 | モデル固有 |
| **Kimi / Moonshot** | Direct API | Kimi K2.x, Kimi K3 | Thinking / 推論努力度 |
| **MiMo / Xiaomi** | Direct API | MiMo V2.6 Flash, Pro, Pro-UltraSpeed（V2.5 後方互換あり） | Normal / Thinking |
| **Ollama Local** | ローカルループバック | Gemma 4, Qwen 2.5, Llama 3.3, DeepSeek-R1 | Thinking (1024予算) / Disabled |
| **OpenRouter** | マルチプロファイルゲートウェイ | 下記 OpenRouter セクション参照 | モデル固有 / プロファイル固有 |
| **マルチプロバイダーゲートウェイ** | カスタムルート設定 | 上記の適格プロバイダーの任意の組み合わせ | プロバイダーごとの個別設定 |

### Anthropic（ダイレクト）

`https://api.anthropic.com` を利用する公式 Anthropic Direct API 接続（`ANTHROPIC_API_KEY`）:

- **専用 API キー**: アプリケーション設定またはユーザー環境変数に安全に保存される公式 Anthropic キーを使用。
- **対応モデル**: `claude-opus-5-5`, `claude-sonnet-5-5`, `claude-haiku-4-5`, `claude-3-7-sonnet`, `claude-3-5-sonnet`, `claude-3-5-haiku`。
- **推論コントロール**: アップストリームが対応するネイティブのアダプティブ思考および推論努力度をサポート。
- **トークン上限診断**: 思考のみで最終テキストが生成されないまま `stop_reason: "max_tokens"` に達したストリーミング・非ストリーミング応答を検知し、明確な診断エラーとしてフェイルクローズ。

### DeepSeek（ダイレクト）

組み込みの **Direct DeepSeek** プリセットは、公式の安定 API ID `deepseek-flash`（現在のモデルは DeepSeek-V4.1-Flash-1017）を使用します: Opus 5 → Max · Sonnet 5 → High · Haiku 4.5 → Low。

- `deepseek-v4-pro` — API ID。現在の公式料金表では DeepSeek-V4-Pro-0813 として掲載されています。
- 両モデルはコンテキスト長 1M、最大出力 384K token。Flash は画像入力に対応し、Pro は非対応です。
- 旧 API ID `deepseek-v4-flash` と `deepseek-v4-flash-vision-exp` は、DeepSeek 側で Flash の別名として受け付けられ、Flash 料金で課金されます。

### GLM / Z.ai（ダイレクト）

Z.ai 公式の Anthropic Messages 互換 API（`https://api.z.ai/api/anthropic/v1/messages`）を直接利用します。

- **API キーと利用条件**: このエンドポイントには設定済みの `ZAI_API_KEY`（または Poverello に入力したキー）を使用します。Poverello はキー種別、Coding Plan の購入履歴、アカウントの許可状態、課金資格を判定しません。Z.ai のアカウントおよびエンドポイント要件は変わる場合があるため、利用前に[公式セットアップガイド](https://zcode.z.ai/en/docs/configuration)を確認してください。
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

複数の名前付きプロファイルをサポート。単一ドロップダウンで選択できる現在のOpenAIモデルカタログ（15モデル）:

| モデル ID | 表示名 |
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

**GPT-6 Astra**: コンテキスト 1.05M · 推論努力度: `low / medium / high / xhigh / max`。
**GPT-6 Astra Pro**: コンテキスト 1.05M · 常時 Pro 推論（`reasoning.mode = pro`）、ユーザーによる努力度選択不可。
**GPT Astra Latest**: 最新の Astra ファミリーモデルを追跡するエイリアス。

組み込みの **OpenRouter: chatGPT** プリセット: Opus 5 → GPT-6 Astra / max · Sonnet 5 → GPT-6 Astra / high · Haiku 4.5 → GPT-6 Astra / medium。

その他利用可能: **OpenRouter: Gemini**（Gemini 3.8 Flash · 推論努力度 `low / medium / high`）、**OpenRouter: Poolside**、**OpenRouter: Tencent**、**OpenRouter: InclusionAI**、**OpenRouter: StepFun**。

---

## DeepSeek API 料金（2026-10-19 確認）

| モデル ID | Peak 入力（キャッシュミス / ヒット） | Off-peak 入力（キャッシュミス / ヒット） | Peak 出力 | Off-peak 出力 |
|---|---:|---:|---:|---:|
| `deepseek-flash` | \$0.30（\$0.006） | \$0.15（\$0.003） | \$1.20 | \$0.60 |
| `deepseek-v4-pro` | \$1.32（\$0.044） | \$0.66（\$0.022） | \$3.96 | \$1.98 |

DeepSeek の料金は 100 万 token あたりです。平日の UTC 01:00–04:00 と 06:00–10:00（中国の法定休日を除く）が Peak、週末と休日は終日 Off-peak です。Poverello は確認済みの 2026 年中国休日・振替出勤日を同梱し、翌年のカレンダーが未収録の場合は画面に表示します。他プロバイダーの料金はアプリ内の参考値で、変更される場合があります。Orchestrator のコスト見積もりは現在、cache 使用量が明示的にゼロの対象 Anthropic Direct 実行のみを計算し、DeepSeek は対象外です。Proについては、現在の公式料金表に個別料金が掲載され、2026年9月14日以降もAPI提供を継続したと記載されています。一方、9月10日の告知ではV4.1-Pro公開までV4.1 Flashへ一時転送しFlash料金を適用するとされていました。Poverelloはリクエストごとの転送先・請求額を確認できません。表示額は公開料金表の値であり、請求額や保証ではありません。[現在の公式料金表](https://api-docs.deepseek.com/quick_start/pricing/)と[9月10日の告知](https://api-docs.deepseek.com/news/news260910/)をご確認ください。

---

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
2. **MCP** タブを選択し、**Settings > Antigravity > MCP Plan Settings** でモデルを設定します。
3. Antigravity の MCP 設定で `anthro-bridge.exe` を `["--mcp-server"]` 付きで登録します（または Poverello の **Configure Automatically** をクリックします）。
4. `/pov-plan` で計画を作成し、`/pov-revise` で計画を更新し、`/pov-review` でコミット前に実装をレビューします。
5. 完全な [Antigravity MCP セットアップガイド](ANTIGRAVITY_MCP.ja.md) に従ってください。

### ワークフロー 4: マルチエージェント開発オーケストレーター

1. Poverello の上部タブから **Orchestrator** を開きます。
2. **Planner** および **Reviewer** 役割のエージェントプロファイルを選択またはカスタマイズします（Orchestrator 設定で組み込みプリセットの選択も可能です）。
3. 選択した対応済みOrchestratorワークフローのタスク目標と最大反復回数（ターン上限）を設定します。
4. 対応するワークフローを実行し、ディスパッチ進捗、監査判定、コスト見積もりの対象範囲を示すバッジを確認します。
5. 完了した実行履歴とディスパッチログは `%APPDATA%\Poverello\runs\` に保存され、監査できます。実行の再開可否は状態によって異なり、完了済みの実行を復元できるとは限りません。

### ワークフロー 5: Antigravity ルール管理（Poverello Direct Coding）

1. **設定**を開き、設定内のナビゲーションから **Antigravity Rules** を選択します。
2. 希望するルールモードを選択し、生成プレビューを確認します。
3. **Apply Settings** をクリックして `%USERPROFILE%\.gemini\config\rules\pov-direct.md` へ安全に書き込みます。
4. 既存のカスタムコメント、frontmatter、バックアップは安全に保持されます。

---

## API キー

| プロバイダー | 環境変数 |
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

## ドキュメント

- [リリースノート (NEWS.md)](../NEWS.md) — 新実装の詳細およびバージョン変更履歴
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
