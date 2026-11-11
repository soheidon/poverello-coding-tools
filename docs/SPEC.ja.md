[English](../SPEC.md) | [日本語](SPEC.ja.md) | [中文(简体)](SPEC.zh-CN.md) | [中文(繁體)](SPEC.zh-TW.md) | [한국어](SPEC.ko.md) | [Français](SPEC.fr.md) | [Deutsch](SPEC.de.md) | [Español](SPEC.es.md)

# SPEC: Poverello Coding Tools

> **v0.26.0:** Codex Autocode、Codex Skills & Rules、共通CLI Toolsを追加。Geminiは既定で無効の任意Codex Reviewerで、設定 > CLI Toolsから有効化します。有効化だけではReviewer選択・起動されません。導入と認証状態は別々です。Node.js/npmは検出しますが自動導入しません。Auto-Convergeは非表示、本番dispatch無効、Stage 2隔離未検証です。
> **v0.26.1:** 非対話のCLI・Node.js/npm状態確認ではコンソールを表示せず、タブ切替時は短時間のキャッシュ結果を再利用します。`0xC0000142`などのWindowsプロセス初期化エラーを明示し、ユーザーが開始するログイン端末は引き続き対話式で表示します。
> **v0.26.2:** Codex AutocodeではCodexの同一セッション内作業と外部Reviewerを区別し、有効な判定の不一致に限って優先Reviewerで裁定します。DeepSeekのheadless設定やツール制限はWindows OSレベルのファイルシステム・ネットワーク隔離を証明しません。診断ログと、履歴のあるSkillテンプレートの安全な更新に対応します。
> **v0.27.0:** リポジトリ単位のActive Plan、画面別Planner / Reviser割当、同一性と指摘をfail-closedで検査するチェックポイント再開を追加します。OpenRouter Tencent Hy4のモデル・料金メタデータを追加し、Antigravity Nerf設定をGUIへ集約して旧スラッシュコマンドを廃止します。自動アップデートは有効化されていません。
> **v0.27.2:** Antigravity AutocodeとCodex Autocodeに、独立したプランフォルダー基準場所の設定を追加します。プラン操作全体で設定場所を一貫して使い、未設定時にアプリの作業ディレクトリへ暗黙にフォールバックしません。

## 概要

Claude Desktop / Claude Code の API リクエストを、複数プロバイダーの Anthropic 互換エンドポイントへルーティングする薄型プロキシ + GUI 管理ツール。

### アーキテクチャ

```
[Claude Desktop / Cowork 3P]      [Google Antigravity]
             |                              | stdio (--mcp-server)
             v                              v
+-------------------------------------------------------------+
| Poverello Gateway (127.0.0.1:4000)                      |
| (axum 0.7 + reqwest プロキシ, APIキー検証, ルート解決)      |
+-------------------------------------------------------------+
             |
             +---> クラウドプロバイダー (Anthropic Direct / DeepSeek / MiMo / Kimi / MiniMax / GLM / OpenRouter / ゲートウェイプロファイル)

[Claude Code CLI] (active_route: "gateway")
             |
             v
+-------------------------------------------------------------+
| Poverello Gateway (127.0.0.1:4000)                      |
+-------------------------------------------------------------+
             |
             +---> クラウドプロバイダー

[Claude Code CLI] (active_route: "ollama")
             |
             v (Loopback ANTHROPIC_BASE_URL)
+-------------------------------------------------------------+
| Ollama Local バックエンド (127.0.0.1:11434/v1)              |
| (/v1/messages, thinking 予算 1024, num_ctx 上書き)          |
+-------------------------------------------------------------+
             |
             +---> ローカルモデル (Gemma 4 / Qwen / Llama 3 / DeepSeek-R1)
```

#### 設計方針

- **Claude Code 専用ローカル LLM 対応**: Claude Code CLI は独立してローカルの Ollama ループバック（`http://127.0.0.1:11434/v1`）へルーティング可能（APIキー不要）。`claude_code.active_route`（`gateway` / `ollama`）で制御され、Claude Desktop や MCP とは完全に独立。
- **シェルモデル + プロバイダー選択**: Claude Desktop には常に `claude-opus-5` / `claude-sonnet-5` / `claude-haiku-4-5` の3モデルが表示される。実際の LLM は GUI で選択する（DeepSeek / MiniMax / Kimi / MiMo / OpenRouter）。アクティブプロバイダーのモデルマッピングがルーティングに使われる。
- **OpenRouter 対応**: Poolside Laguna S/XS をデフォルトとして、OpenRouter の Anthropic 互換エンドポイントへルーティングする。専用の thinking モード制御（Max/On/Off）は、リクエスト時に OpenRouter の `reasoning` フォーマットへ変換される。
- **API キーが必須なのはアクティブプロバイダーのみ**: v0.5.0 以降、起動時にチェックされるのはルートテーブルで参照されるプロバイダーのみ。非アクティブプロバイダーのキーは不要。
- **薄型プロキシ**: `model` フィールド以外は一切変更しない。SSE はバイト単位で透過転送。
- **ロスレス転送**: メッセージ本文、ツール呼び出し、thinking ブロックは加工されずにそのまま通過する。
- **Windows ネイティブ GUI**: Tauri v2 + React 19 + TypeScript。バックエンドは Rust、フロントエンドは Vite + React 19。
- **ゼロ外部依存**: v0.3.0 以降、プロキシは Tauri バイナリに内蔵。Python は不要。
- **多言語対応**: 8言語（en, ja, zh-CN, zh-TW, ko, fr, de, es）。`lang/` にファイルを置くだけで新言語を追加できる。初回起動時に言語選択画面を表示。
- **Direct DeepSeek モデル**: 正規 API ID は `deepseek-flash` と `deepseek-v4-pro`。現在の公式料金表では、それぞれ DeepSeek-V4.1-Flash-1017、DeepSeek-V4-Pro-0813 として掲載されている。両モデルは Anthropic Messages 互換エンドポイント、ツール呼び出し、1M コンテキスト、最大384K出力に対応する。Flash は画像入力に対応し、Pro は非対応。旧 ID `deepseek-v4-flash` と `deepseek-v4-flash-vision-exp` は DeepSeek 側で Flash の別名として引き続き受け付けられ、Poverello の組み込み設定は正規 ID を使用する。公式資料間でProの転送・課金説明が異なり、Poverelloは公開料金表を表示するが、個々のリクエストの実行先・請求額は確認できない。[公式料金表](https://api-docs.deepseek.com/quick_start/pricing/)と[9月10日の告知](https://api-docs.deepseek.com/news/news260910/)を参照。
- **機能検出**: OpenRouter API から取得したライブの機能フラグ（supports_image_url、supports_image_base64、supports_video_url、supports_video_base64）を config.json に永続化する。
- **ピーク/バレー料金の認識**: DeepSeek と OpenRouter のピーク時間帯をローカルタイムゾーンで表示する。
- **DeepSeek の推論・料金カレンダー**: Thinking モードでは Low / High / Max を `output_config.effort` で送信し、Normal は推論を無効化する。平日 UTC 01:00–04:00 と 06:00–10:00（中国の法定休日を除く）がピークで、週末・休日は終日 Off-peak。Poverello は確認済みの2026年中国休日・振替出勤日を同梱し、カレンダーの状態（`current` / `update_due` / `expired`）を表示する。未収録年は通常の曜日・時間帯判定にフォールバックする。DeepSeek は Orchestrator のコスト見積もり対象外。
- **MiniMax-M3 thinking トグル**: MiniMax-M3 は Anthropic 互換 API で Thinking ON/OFF に対応する（`thinking: {"type":"adaptive"}` / `{"type":"disabled"}`）。M2.x 系モデルは引き続き thinking 専用。起動時マイグレーションが既存ユーザーのレガシー `thinking_only` を `thinking` へ変換する。
- **Xiaomi MiMo-V2.6 対応**: MiMo-V2.6-Flash、MiMo-V2.6-Pro、MiMo-V2.6-Pro-UltraSpeed を Direct MiMo プロバイダーモデルとして追加。3 モデルすべてが 1,000,000 トークンのコンテキストウィンドウとネイティブマルチモーダル対応（テキスト、画像、動画）を持つ。組み込みデフォルトルート: Opus 5 → `mimo-v2.6-pro`（Thinking）、Sonnet 5 → `mimo-v2.6-pro`（Normal）、Haiku 4.5 → `mimo-v2.6-flash`（Thinking）。MiMo は Normal/Thinking トグルのみ対応 — 推論強度レベルなし（`supportsReasoningEffort: false`）。
- **MiMo V2.5 後方互換性**: レガシーの `mimo-v2.5`、`mimo-v2.5-pro`、`mimo-v2.5-pro-ultraspeed` は元の機能を持ったまま静的に認識され続ける。変更されていない既存のデフォルトルートは起動時に V2.6 へ自動マイグレーションされる。ユーザーがカスタマイズしたルートターゲット、thinking モード、default_model 設定は保持される。`models` オブジェクトを持たない `model_map` を使用するレガシー設定は、プロバイダー全体のフラグにフォールバックするのではなく、静的機能リゾルバー（`try_resolve_static_model_capabilities`）からルート単位の機能を解決する。これにより、レガシー設定でも V2.6 ルートが正しい動画機能を受け取ることが保証される。
- **レスポンスモデル ID 正規化**: API レスポンス（SSE ストリーミング / ノンストリーミングの両方）内の upstream モデル名を、Anthropic 公式モデル名へ書き戻す。config.json の `normalize_response_model_identity` と実行時の `AtomicBool` で制御する。サーバー設定の保存との相互汚染を避けるため、独立した保存コマンド（`update_normalize_model_identity`）を使用する。
- **構造化通信ログ**: `tracing` + `tracing-appender` が構造化ログを `%APPDATA%\Poverello\Communication-Logs\proxy-*.log` に書き込む。各リクエストには `AtomicU64` カウンターから相関 ID が付与される。ログエントリにはリクエストモデル、ゲートウェイモデル、upstream モデル、正規化の結果、スキップ理由が含まれる。機密データ（プロンプト、本文、API キー）は記録されない。
- **PEAK バッジ**: ピーク価格のモデルに対して、ダッシュボードに色分けされたピンクのバッジを表示する。
- **UTC オフセット表示**: タイムゾーンセレクターに、各オプションの横へ動的な UTC オフセット（例: UTC+09:00）を表示する。
- **Laguna S/XS 2.1 トークン上限失敗の検出**: SSE ストリームとノンストリームレスポンスの両方で、`stop_reason: "max_tokens"` を伴う reasoning のみのレスポンスを検出する。利用可能なテキストやツール呼び出しを生成せずに毎ターンのトークン上限に達した場合、警告をログに記録する。OpenRouter 経由のすべての Poolside Laguna モデルで利用可能。
- **Poolside thinking:disabled パススルー**: Poolside モデル向けに、クライアントが送信した `thinking: { type: "disabled" }` を OpenRouter の `reasoning: { enabled: false }` フォーマットへ変換し、保存済みの設定がなくても disabled thinking が正しく転送されるようにする。
- **Laguna Opus デフォルト移行**: `poolside/laguna-s-2.1` の OpenRouter ユーザー向けに、`claude-opus-5` のデフォルトを thinking-on から通常モードへ変更する、1回限りの冪等なマイグレーション。新規インストール用テンプレートは更新後のデフォルトを反映する。
- **OpenRouter マルチモデルセット**: ユーザーごとに複数の OpenRouter モデルセットを持つことができ、各モデルセットは独自の API キーとモデル設定を持つ。モデルセットの CRUD は Tauri コマンド経由で行う。ダッシュボードまたは設定からアクティブなモデルセットを切り替える。モデルセットはドラッグ & ドロップで並べ替え、非表示化、設定した順序で永続化が可能。
- **OpenRouter ダッシュボードカード**: ダッシュボードは、表示中の OpenRouter モデルセットごとに1枚のカードを作成し、モデルセットが無い場合はフォールバックカードを表示する。モデル概要では OpenRouter 表示専用に、最初の `/` より前のベンダーネームスペースを非表示にする。ルーティング用の完全な upstream ID は変更されない。
- **OpenRouter モデルレジストリ**: 既知の OpenRouter モデルのローカル組み込みレジストリ（`model_capabilities.rs`、`builtinOpenRouter.ts`）。事前設定済みの機能（vision、video、thinking ポリシー、推論強度）、ベンダーグルーピング、価格データを含む。ライブ API 呼び出しなしでモデル分類に使用する。
- **OpenRouter 価格詳細**: 組み込み価格は、入力、出力、キャッシュ入力レートの現在価格と改定後標準価格に対応する。GPT-5.6 の Sol、Terra、Luna、Pro 各バリアントを含む。GUI はプロモーション価格と標準価格の両方が利用可能な場合、両方を併記して表示する。
- **GPT-5.6 モデル対応**: OpenRouter モデルセットは Sol、Terra、Luna のモデルバリアントを使用でき、機能を考慮した thinking 制御と、該当する場合の長文コンテキスト料金に関する価格メモを備える。組み込みの OpenAI GPT-5.6 Balanced モデルセットは、新規インストールでは Opus 5 → GPT-5.6 Sol、Sonnet 5 → GPT-5.6 Terra、Haiku 4.5 → GPT-5.6 Luna へ、3ルートすべてで Thinking High の推論強度でルーティングする。既存の保存済みルーティングは自動では変更されない。
- **ダッシュボード駆動のウィンドウサイズ調整**: 初期表示時と行数変更時に、3列グリッド内の表示中のダッシュボードカードからウィンドウの高さを計算する。この計算はカードの高さ、グリッドの隙間、ネイティブの最小サイズ、モニターのワークエリア、DPI スケーリング、ウィンドウ装飾を考慮しつつ、行数が変わらない場合は手動リサイズを保持する。
- **ローカライズされた NSIS インストーラー**: Windows インストーラーは英語、日本語、中国語（簡体）、中国語（繁体）、韓国語、フランス語、ドイツ語、スペイン語の言語選択を提供し、Poverello のアプリケーションアイコンを同梱する。
- **リグレッションカバレッジ**: Vitest のカバレッジには、OpenRouter モデルセットの並び順と保存レース、本番価格データ、ダッシュボードのカード数セマンティクス、モニター考慮のウィンドウサイズ調整が含まれる。
- **OpenRouter 経由の新プロバイダー**: InclusionAI と StepFun を OpenRouter モデルプロバイダーとして追加する。専用の機能フラグ、thinking モード制御、ベンダーグルーピングを備える。
- **Tencent Hy3 thinking モード**: Tencent の Hunyuan モデルで推論強度 Low/High に対応する。proxy.rs の thinking モード変換は `thinking_mode` を OpenRouter の `reasoning` フォーマットへマッピングする。UI は Low/High をドロップダウンオプションとして表示する。
- **Kimi K3 修正**: 機能定義からハードコードされた `forced_reasoning_effort` を削除した。固定の「Max」表示を設定可能なドロップダウンセレクターに置き換えた。デフォルト値は保存済み設定から取得し、フォールバックとして "max" を使用する。
- **設定書き込みの直列化**: config を書き込むすべての Tauri コマンドは、`Mutex` ガード付きの `execute_serialized_config_mutation` を介して直列化される。`ConfigState` 構造体は検証付きで `applied_config`、`in_flight_config`、`pending_ops` を追跡する。複数の設定変更が同時に保存される際のレースコンディションを防ぐ。
- **OpenRouter UI レース修正**: (1) `syncUiFromSavedRouteRef` の最新コールバック ref が、古いクロージャによる新ルート UI の上書きを防ぐ。(2) `rollbackRouteId` ガードがルート間の Phase 2 ロールバックを防ぐ。(3) `useRouteSaveGeneration` フックが全ハンドラーに `begin()`/`isCurrent()` の世代ガードを提供する。(4) 保存キューフック（`useOpenRouterSaveQueue`）がドレインループ、supersede 検出、再起動時の OR 再集計を提供する。
- **Dev/Stable アプリ ID の分離**: `paths.rs` の `AppChannel` enum（`Stable`/`Dev`）が、それぞれ別の識別子（`com.soheidon.anthro-bridge` vs `.dev`）、設定ディレクトリ（`Poverello` vs `Poverello Dev`）、キャッシュパスを選択する。Dev チャンネルは `tauri.dev.conf.json` を使用する。NPM スクリプト: `npm run dev`（dev）、`npm run dev:stable`（stable）。
- **設定テンプレートの埋め込み**: `include_str!()` が `config_template.rs` をコンパイル時に埋め込み、同梱の `config.json` への実行時依存を排除する。`merge_bundled_providers` は型付きエラーハンドリング付きで `Result` を返す。
- **フロントエンドのリグレッションテスト**: `QueueHarness` と `GenerationHandlerHarness` を使用した、OpenRouter 保存レース条件に対する vitest リグレッションテスト 7 件。テスト対象: 最新コールバック ref、ルート間ロールバックガード、ID キャプチャ、リフレッシュ再試行（失敗 + 成功パス）、実行中 supersede、世代ガード。
- **Claude Code コンテキスト管理**: Claude Code 向けのモデル認識型自動圧縮。`resolve_effective_auto_compact` は各標準ルート（claude-opus-5、claude-sonnet-5、claude-haiku-4-5）をその upstream モデルへ解決し、各モデルのコンテキスト容量を静的レジストリ `model_catalog.json` で参照し、Auto モードでは既知の最小容量を安全なコンテキストウィンドウとして使用する。コンテキスト制御は3つすべての容量が既知の場合のみ適用される（それ以外はステータスが Incomplete）。ヘッダーのトグルでコンテキスト管理のオン/オフを切り替える。高度なモードとしきい値は `config.json` の `claude_code.auto_compact` 配下で設定する。モード: `auto`、`manual`（`window_tokens`）、`claude_default`。
- **Claude Code 起動コマンド生成**: `build_claude_code_launch_command` は、ゲートウェイ接続変数（ローカルゲートウェイを指す `ANTHROPIC_BASE_URL`、`ANTHROPIC_AUTH_TOKEN` = `sk-local-gateway`）と Claude Code コンテキスト制御変数（`CLAUDE_CODE_AUTO_COMPACT_WINDOW`、`CLAUDE_AUTOCOMPACT_PCT_OVERRIDE`）を組み合わせた完全な PowerShell コマンドを生成する。コンテキスト管理が無効、Incomplete、または Claude デフォルト設定の場合、コマンドは `Remove-Item Env:... -ErrorAction SilentlyContinue` で古いコンテキスト変数を削除し、以前設定されたセッション値が新しい起動に漏れないようにする。Claude 設定パネルの「Claude Code起動コマンドをコピー」ボタンがコマンドをクリップボードにコピーする。Poverello はコマンドの生成とコピーのみを行い、実行は一切しない。
- **共有モデルルーティングモジュール**: `model_routing.rs` は、ルートから upstream への解決を `proxy.rs` とコンテキストリゾルバーが共有する純粋関数に抽出し、コンテキストウィンドウがプロキシが実際に転送する upstream モデルと同一のモデルを解決することを保証する。
- **コンテキスト容量レジストリ**: `model_catalog.json` は、既知のコンテキスト容量の静的レジストリ。組み込みの直接プロバイダーモデル（DeepSeek、MiniMax、Kimi、MiMo V2.6 および V2.5）と組み込みの OpenRouter モデル（Poolside、Tencent、InclusionAI、StepFun、OpenAI GPT-6.1、GPT-6、GPT Astra Latest、GPT-5.6、Google Gemini）をカバーする。不明なカスタム OpenRouter モデルは有効なルートターゲットのままであるが、メタデータが追加されるか手動モードが設定されるまで、コンテキスト管理は Incomplete として報告される。
- **Google Gemini モデル対応**: OpenRouter プロファイルで Gemini 3.1 Pro Preview と Gemini 3.7 Flash を組み込みモデルとして利用でき、料金・機能データも収録。Gemini 3.7 Flash はプロモーション価格と、取り消し線付きの通常価格を併記する。
- **GLM (Z.ai) ダイレクトプロバイダー対応**: 設定された `ZAI_API_KEY`（または Poverello に入力したキー）を使って Z.ai の Anthropic 互換エンドポイントへリクエストを送信。Poverello はキー種別、Coding Plan の購入履歴、アカウントの許可状態、課金資格を判定しないため、利用前に[Z.ai の現行セットアップガイド](https://zcode.z.ai/en/docs/configuration)でアカウントおよびエンドポイント要件を確認してください。4モデル構成（`glm-5.3`、`glm-5.3-flash`、`glm-5.3-flashx`、`glm-5.2`）。GLM-5.3 系列は推論強度（`low`, `high`, `max`）を選択可能な常時思考、GLM-5.2 は Normal/Thinking トグル。厳格な出力検証により、思考ブロックのみで最終テキストが空の場合（思考中の `max_tokens` 到達等）は Orchestrator・MCP の双方で明示的なプロバイダーエラーとしてフェイルクローズする。
- **v0.25.3モデルとThinking安全制御**: MiniMax M3.1 Flash、Kimi K3.1 Pro / Highspeed、GLM-5.3 Directモデルを追加し、Kimi Code K3.1 IDを分離。8つのIDはThinking専用で、無効化要求は上流送信前に拒否します。Kimi Codeのmediumはhigh、xhighはmaxへ正規化。価格基準日は2026-10-31。Kimi K3.1のUSD表示はCNY料金からの参考換算で、OpenRouterは実行時価格を優先します。
- **Antigravity Autocode と Skills & Rules（v0.25.4）**: 上部のAntigravity AutocodeタブにExternal ReviewとCustomize Coding Settingsを配置。Reviewer presetを廃止し、Codex CLIとDeepSeek Harnessを直接設定します。Codexのアカウント識別、認証方式・状態、設定profileは区別し、本人を特定できない場合は推測しません。設定 > Antigravity Skills & RulesではWorkflow Commands（バックアップ付きの確認制修復を含む）、CLI Tools、Task Presets、Global Rules、Nerf Skillsを管理します。OpenRouterにGPT-6.1 TerraとTerra Proの機能・推論・料金メタデータを追加。Reviewer Policyはschema v3を使用し、既定値はnone（外部レビュアーなし）です。none、codex_only、codex_plus_deepseekをサポートし、旧Reviewer presetは無視します。
- **Codex Autocode と Skills & Rules（v0.26.0）**: 上部Codex AutocodeタブでCodex Workflow SkillのReviewerを設定します。設定 > Codex Skills & Rulesでは3つのCodex SkillとCODEX_HOME/AGENTS.md内のPoverello管理領域を管理します。共通の設定 > CLI ToolsではNode.js/npm、Codex CLI、Gemini CLI、DeepSeek Harnessを管理します。Geminiは既定で無効で、有効化だけでは選択・起動されません。
- **Windows CLIプロセス処理（v0.26.1）**: 非対話CLIおよびNode.js/npm状態確認のコンソール表示を抑止し、画面遷移間で有効期限付きのメモリ内結果を共有します。`0xC0000142`などの初期化失敗を明示します。ユーザーが明示的に開始するログインは引き続き対話式です。個々の環境で過去に発生した初期化失敗の原因を特定したと主張するものではありません。
- **Codexレビューと診断（v0.26.2）**: Codexの同一セッション内作業と外部レビューを分離します。優先Reviewerは有効な判定の不一致だけを裁定し、失敗・未完了や未解決のBlocking指摘は上書きできません。DeepSeekの設定overlayはOS強制隔離の証明ではありません。JSONL診断と、バックアップ・確認付きの履歴Skill更新を提供します。
- **Anthropic 公式 Direct プロバイダー対応**: `https://api.anthropic.com` を `ANTHROPIC_API_KEY` 経由で直接利用。ネイティブの Messages プロトコルをプロトコル変換なしで透過転送。検証済みモデルカタログ（`claude-opus-5-5`, `claude-sonnet-5-5`, `claude-haiku-4-5` 等）、ストリーミングおよび非ストリーミングのトークン上限障害診断（思考ブロックのみで最終テキストが生成されないまま `stop_reason: "max_tokens"` となった場合に明示的エラーでフェイルクローズ）、およびサンプリングパラメータ検証。
- **マルチプロバイダーゲートウェイプロファイル（`gateway_profile`）**: 3つの標準 Claude ルート（`claude-opus-5`, `claude-sonnet-5`, `claude-haiku-4-5`）それぞれに独立して異なるプロバイダーとモデルを割り当て可能。`active_provider: "gateway_profile"` および `active_gateway_profile_id` で制御され、`gateway_profiles` 配下の指定プロファイルを参照する。ルートごとの機能チェックおよび Claude Code 自動圧縮のためのコンテキスト容量計算に統合される。
- **正本 Poverello スキル & MCP 名前空間の移行**: 正本 MCP ツール（`poverello/plan`, `poverello/revise`, `poverello/review`）およびスラッシュコマンド（`/pov-mcp-plan`, `/pov-mcp-revise`, `/pov-mcp-review`）。Poverello は Antigravity の `mcp_config.json` に `poverello` サーバーキーを加法的に登録し、既存の `anthro-bridge` キーや旧 `/anthro-*` スキルを完全な後方互換性のために保持する。親ディレクトリを削除しない堅牢な単一ファイルライフサイクル管理。 直接MCPスキルに加え、上位ワークフローSkill `/pov-full-cycle`、`/pov-plan-only`、`/pov-build-review`も提供します。
- **Orchestrator 正規化使用量 & 財務コスト計算エンジン**:
  - `NormalizedUsage` スキーマ: アダプターが対応形式の使用量を返した場合に `input_tokens`, `output_tokens`, `raw_total_tokens`, `cache_read_tokens`, `cache_write_tokens` を捕捉し、取得できた値を `RunJournal`（`runs/<run_id>.json`）および収束監査ログに永続化する。未取得・不正形式の値はゼロではなく不明として扱う。正規化フィールド `cache_write_tokens` のシリアライズ名は `cacheWriteTokens` であり、プロバイダー固有のキャッシュ作成使用量はこのフィールドに対応付けられる。
  - 現在の同梱コスト見積もりは、Anthropic Direct Opus 5.5 / Sonnet 5.5 でキャッシュ各項目が明示的にゼロの、条件を満たす Planner–Reviewer収束ディスパッチに限定される。他のプロバイダー、ルート、実行経路は未算定／利用不可であり、コストゼロを意味しない。
  - 高精度ナノドル整数財務エンジン: 金額は `amount_nanos: u64`（1 USD = $10^9$ ナノドル）で表現され、JavaScript の float64 精度損失を防ぐため文字列としてシリアライズされる。浮動小数点演算（`f32`/`f64`）は一切使用しない。
  - スケジュールID・プロバイダー・モデル・通貨の厳格な一致検証（`lookup_and_verify_schedule`）: 同梱スケジュールとして `anthropic:claude-opus-5-5:2026-10:uncached`（入力 $4.00 / 出力 $20.00 / MTok）および `anthropic:claude-sonnet-5-5:2026-10:uncached`（入力 $2.00 / 出力 $10.00 / MTok）に対応。未知ID、不一致、未指定はすべて未算定（unpriced）としてフェイルクローズ。
  - フェイルクローズ集計（`RunCostSummary`）: すべてのディスパッチが検証済みスケジュールで算出できた場合は `Complete`、一部のみ算出できた場合は未算定数を記録し小計を保持する `Partial`、1件も算出できない場合は数値を非表示とする `Unavailable`。寄与したスケジュール一覧（`schedules`）および算出方法（`method`）を明示。
  - `list_journals()` による動的再集計: ジャーナル一覧取得時にディスク上の不変ディスパッチ記録から動的に再集計を実行し、ディスク上の古い集計値が厳格な検証を迂回して表示されるのを完全に防止。
### GUI 管理ツール

Tauri v2 + React 19 + TypeScript。トップレベルナビゲーション: Gateway（ダッシュボード）、MCP、Antigravity Autocode、Codex Autocode、Settings。Antigravity AutocodeにはExternal ReviewとNerfのコーディング設定を配置し、Codex AutocodeはCodex Workflow Skills用Reviewerを設定します。SettingsにはGeneral、CLI Tools、Claude Desktop、Antigravity MCP、Antigravity Skills & Rules、Codex Skills & Rulesがあります。Antigravity Skills & RulesではWorkflow SkillsとGlobal Rulesを管理します。旧コマンド `/pov-nerf-balanced` と `/pov-nerf-lightweight` は廃止されています。Codex Skills & Rulesは3つのCodex Workflow SkillsとCODEX_HOME/AGENTS.md内のPoverello管理領域を管理します。OrchestratorとAuto-Convergeは通常ナビゲーションから非表示です。実運用のdispatchは無効で、Stage 2の分離は未検証です。

```
+------------------------------------------+
|  Poverello                   |
|  [Start/Stop Gateway] [Status]    [=]   |
+------------------------------------------+
|  ダッシュボード                            |
|  +- LLMプロバイダ選択 -------------------+|
|  | [DeepSeek] [MiMo] [MiniMax] [Kimi]          ||
|  +- ステータス ----------------------------+
|  | Port 4000 | APIキー | Gateway URL    ||
|  | モデルルーティングテーブル            ||
|  +- 最新ログ -----------------------------+
|  | Pro/Flashカウンター付きログビューア   ||
|  +---------------------------------------+
+------------------------------------------+

設定 (=):
  +- 言語 ---------------------------------+
  | 即時切り替え用ドロップダウン           |
  +- APIキー -------------------------------+
  | プロバイダーごとのAPIキー管理          |
  +- Claude Desktop セットアップ ----------+
  | 設定JSONの生成、コピー、                |
  | 設定ファイルの検出                      |
  +- ゲートウェイ設定 ---------------------+
  | config.json エディタ（上級者向け）     |
  +---------------------------------------+
```

### Active Plan とWorkflow担当割当

- Active Planファイルはリポジトリごとに1つ選択し、Codex AutocodeとAntigravity Autocodeで共有します。PlannerとReviserは画面ごとに別々に保存し、Reviewer設定は既存のレビュー設定を参照します。
- Workflow Skillsは画面ごとの担当割当を読み込みます。必要な担当が未設定、非対応、または利用できない場合は、暗黙に別担当へ切り替えず停止します。担当割当自体がエージェントを起動したり、Reviewerの安全ゲートを迂回したりすることはありません。
- チェックポイントはActive Plan内にワークフロー状態、完了作業、変更ファイル、検証結果、レビュー状態、次の操作を保存します。再開前にリポジトリ同一性、Active Planのパス、計画revisionとfile identity、未解決指摘、変更ファイル状態を検査し、不一致があれば停止します。
- AntigravityとCodexは、それぞれのSkills & Rules設定で独立したプロジェクトルートを保存します。対応するAutocodeのプラン一覧、選択、作成、Active Planと役割割当は、そのルートを一貫して使用します。未設定・無効時は設定を促し、アプリの作業ディレクトリを暗黙に使いません。
- ルートパスは直接編集、フォルダー選択、リセットができます。リポジトリルートまたはその`.plan`フォルダーを選んだ場合、プラン管理用ルートを正規化します。
- 計画候補の反映、監査追記、チェックポイント保存は、期待file identityとSHA-256の両方を使うCASで保護します。関連IPCは `apply_plan_candidate`、`append_plan_audit_trail`、`save_plan_checkpoint`、`get_plan_checkpoint`、`verify_plan_resumption` です。

### Tencent Hy4 と Antigravity Nerf設定

- OpenRouterの厳選カタログに `tencent/hy4` を追加しました。コンテキスト長は1,048,576トークンで、画像URL/base64入力とThinking / High推論設定に対応します。組み込み `OpenRouter: Tencent` プロファイルではOpusルートの既定モデルです。Generalの料金表には2026-11-08基準の参考価格を掲載します。
- Nerf設定はAntigravity Autocodeで管理します。旧 `/pov-nerf-balanced` と `/pov-nerf-lightweight` コマンドは廃止しました。
- v0.27.0では自動アップデートを設定していません。署名済みUpdater成果物と本番署名鍵は含まれません。

### Tauri コマンド一覧

| # | コマンド | 種別 | 説明 |
|---|---------|------|------|
| 1 | `check_health` | async | プロキシの死活確認 |
| 2 | `check_gateway_status` | sync | ポート 4000 + tokio タスクの生存確認 |
| 3 | `check_api_key` | sync | アクティブプロバイダーの API キー状態 |
| 4 | `set_env_api_key` | sync | setx で API キーを永続保存 |
| 5 | `get_port_4000_process` | sync | netstat でポート 4000 の PID を取得 |
| 6 | `read_config` | sync | config.json を読み込む |
| 7 | `read_config_raw` | sync | config.json の生テキスト + エンコーディング判定 |
| 8 | `write_config` | sync | config.json を保存（UTF-8 / Shift-JIS） |
| 9 | `read_latest_log` | sync | 最新ログを読み込む |
| 10 | `read_log` | sync | 指定したログファイルを読み込む |
| 11 | `list_logs` | sync | ログファイルの一覧 |
| 12 | `create_new_log` | sync | 新しいログファイルを作成 |
| 13 | `open_logs_folder` | sync | ログフォルダを開く |
| 14 | `open_path` | sync | 任意のパスを開く |
| 15 | `find_claude_configs` | sync | Claude Desktop 設定ファイルを自動検出 |
| 16 | `start_proxy` | sync | プロキシを起動（設定解決 -> spawn -> ポート確認） |
| 17 | `stop_proxy` | sync | プロキシを停止（グレースフルシャットダウン） |
| 18 | `proxy_status` | sync | タスクの生存確認 |
| 19 | `check_all_api_keys` | sync | 全プロバイダーの API キー状態 |
| 20 | `update_active_provider` | sync | active_provider を保存 |
| 21 | `update_provider_api_key_env` | sync | プロバイダーの api_key_env を保存 |
| 22 | `get_user_language` | sync | 保存済みの言語設定を取得 |
| 23 | `set_user_language` | sync | 言語設定を保存 |
| 24 | `is_first_run` | sync | 初回起動かどうかを判定（user_prefs.json の有無） |
| 25 | `openrouter_get_models` | async | OpenRouter モデルカタログを取得/キャッシュ |
| 26 | `set_model_upstream` | sync | ゲートウェイモデルの upstream モデル + thinking 設定 + 機能フラグを保存 |
| 27 | `update_server_config` | sync | サーバーのホスト/ポート/CORS 設定を保存 |
| 28 | `update_normalize_model_identity` | sync | レスポンスモデル ID 正規化トグルを保存（config + 実行時 AtomicBool を更新） |
| 29 | `update_claude_code_auto_compact_global` | sync | グローバルの Claude Code コンテキスト管理をトグル（有効化 + トリガー割合） |
| 30 | `update_claude_code_auto_compact_target` | sync | プロバイダー/モデルセットごとのコンテキストモード（auto / manual / claude_default）+ 手動ウィンドウトークンを設定 |
| 31 | `update_claude_code_context_settings` | sync | グローバル + ターゲットのコンテキスト設定をアトミックにまとめて更新 |
| 32 | `resolve_claude_code_auto_compact` | sync | 有効なコンテキスト設定（モード、ウィンドウトークン、トリガー割合、ステータス）を解決 |
| 33 | `build_claude_code_launch_command` | sync | 完全な PowerShell の Claude Code 起動コマンドを生成（ゲートウェイ + コンテキスト環境変数） |
| 34 | `fetch_ollama_models` | async | ループバック `GET http://127.0.0.1:11434/api/tags` からローカルモデルを自動検出（2秒タイムアウト） |
| 35 | `get_claude_code_config` | sync | Claude Code のルーティング、auto_compact、third_party_provider 設定を取得 |
| 36 | `update_claude_code_third_party` | sync | Ollama Local 設定（models, thinking_modes, context_windows, show_on_dashboard）を保存 |
| 37 | `set_claude_code_active_route` | sync | Claude Code の有効ルート（`gateway` または `ollama`）を切り替え |
| 38 | `get_pov_direct_status` | sync | Antigravity direct coding ルールのステータス、ハッシュ、メタデータを取得 |
| 39 | `preview_pov_direct_content` | sync | 生成される `pov-direct.md` の内容および管理ブロックをプレビュー |
| 40 | `apply_pov_direct_settings` | sync | バックアップ、競合検出、検証付きロールバックを伴い direct coding ルールをアトミックに保存 |
| 41 | `get_reviewer_policy` | sync | アクティブReviewerポリシー（`none / codex_only / codex_plus_deepseek`）を取得 |
| 42 | `set_reviewer_policy` | sync | アクティブReviewerポリシーを設定し `~/.gemini/antigravity-ide/pov-reviewer-policy.json` に永続化 |
| 43 | `get_workflow_skills_policy_status` | sync | ワークフロースキルのポリシー適合状況・詳細一覧を取得 |
| 44 | `get_antigravity_global_rules_status` | sync | Global Ruleテンプレートのインストール状況、ハッシュ一致、およびプラットフォーム削除可否（`can_remove`）を取得 |
| 45 | `install_antigravity_global_rule` | sync | 製品同梱の標準Global Ruleテンプレート（`codex-cli.md` または `deepseek-harness.md`）を安全にインストール |
| 46 | `remove_antigravity_global_rule` | sync | SHA-256ハッシュ検証付きで標準Global Ruleを安全に削除（Windows環境限定） |
| 47 | `install_all_antigravity_mcp_commands` | sync | MCP画面専用のMCP Skill 3件（`/pov-mcp-plan`、`/pov-mcp-revise`、`/pov-mcp-review`）のみを一括インストール |

### プロキシサーバー (proxy.rs)

v0.3.0 で Python から Rust（axum 0.7/reqwest）に移植。

#### エンドポイント

| Method | Path | 動作 |
|--------|------|------|
| GET | `/health` | 死活確認 |
| GET | `/v1/models` | 公開モデル一覧（`visible: true` のみ） |
| POST | `/v1/messages` | モデル解決 -> thinking 注入 -> メディアチェック -> 転送（stream/non-stream） |
| POST | `/v1/messages/count_tokens` | 対応時のみ upstream へ転送 |

#### モデルルーティング

各プロバイダーの `models` セクションを使用して、gateway model -> (provider, upstream model) の逆引きテーブルを構築する。全プロバイダーが同じ gateway モデル名を使用するため、衝突時は `active_provider` が優先される。結果として、アクティブプロバイダーのモデルのみがルートテーブルに残る。

#### API キー検証（v0.5.0〜）

Pass 1: モデルルートテーブルを構築（API キー不要）
Pass 2: ルートテーブルで参照されるプロバイダーの API キーのみチェック

#### Thinking 注入

設定エントリに `thinking: "disabled"` が含まれるモデルに対して、ユーザーが明示的に thinking を設定していない場合のみ `{"type": "disabled"}` を注入する。

#### レスポンスモデル正規化

`normalize_response_model_identity` が有効な場合、プロキシは upstream レスポンス内の `model` フィールドを書き換える:

- **ノンストリーミング**: JSON レスポンスをパースし、`model` を Anthropic 標準名へ書き換えて再シリアライズ
- **ストリーミング (SSE)**: `message_start` イベントフレームをインターセプトし、SSE のフォーマットと空白を保つためバイト範囲置換で `model` をその場で書き換え
- **スキップ理由**: `disabled`（トグルオフ）、`non_success_status`（200 以外のレスポンス）、`content_encoding_not_transformable`（gzip/brotli）、`stream_error`、`stream_cancelled`
- **判定ロジック**: 本番コードとテストの両方で使用される純粋関数（`should_normalize_nonstream`、`nonstream_skip_reason`）

#### メディアチェック / 画像サニタイズ

モデル単位の `supports_vision` / `supports_video` フラグが動作を決定する。画像を受け取った vision 非対応モデルには `non_vision_image_policy` が適用される:
- `replace`（デフォルト）: 画像ブロックをプレースホルダーテキストに置換
- `drop`: 画像ブロックを削除（コンテンツが空になった場合はプレースホルダーを挿入）
- `reject`: 400 エラーを返す

動画ブロックは常に 400 エラーを返す。`non_vision_image_policy` は `/health` で確認できる。

#### Claude Code コンテキスト管理

Claude Code のコンテキスト制御は、公式の環境変数 2 つを使用する:

```
CLAUDE_CODE_AUTO_COMPACT_WINDOW
CLAUDE_AUTOCOMPACT_PCT_OVERRIDE
```

リゾルバーパイプライン:

1. 各標準ルート（claude-opus-5、claude-sonnet-5、claude-haiku-4-5）をその upstream モデルへ解決
2. `model_catalog.json` で各 upstream モデルのコンテキスト容量を参照
3. 3 つすべての容量が既知であることを要求
4. 既知の最小容量を安全なコンテキストウィンドウとして使用
5. 設定されたトリガー割合を適用

モード: `auto`（既知の最小容量）、`manual`（`window_tokens`）、`claude_default`（Claude Code 独自のデフォルト。変数を設定しない）。実効ステータスは `applied`、`disabled`、`incomplete` のいずれか。

起動コマンドは、ゲートウェイ接続変数とコンテキスト変数を組み合わせる:

```powershell
$env:ANTHROPIC_BASE_URL='http://127.0.0.1:4000'; $env:ANTHROPIC_AUTH_TOKEN='sk-local-gateway'; $env:CLAUDE_CODE_AUTO_COMPACT_WINDOW='262144'; $env:CLAUDE_AUTOCOMPACT_PCT_OVERRIDE='90'; claude
```

コンテキスト制御が適用されない場合、コマンドはまず古い変数を削除する:

```powershell
Remove-Item Env:CLAUDE_CODE_AUTO_COMPACT_WINDOW -ErrorAction SilentlyContinue;
Remove-Item Env:CLAUDE_AUTOCOMPACT_PCT_OVERRIDE -ErrorAction SilentlyContinue;
```

割合オーバーライドは圧縮を早める方向にのみ作用する。Claude Code のデフォルトよりも圧縮を遅らせる値は無視される場合がある。Poverello はコマンドの生成とコピーのみを行い、実行は一切しない。また、特定の Claude Code バージョンがこれらの変数を尊重することを保証するものではない（最終確認には Claude Code の診断情報または実際に観測された圧縮動作が必要）。

### 多言語対応

1 言語 1 ファイルの構成。`import.meta.glob` で自動検出:

```
gui/src/i18n/lang/
  en.ts      English (canonical — defines TranslationKey type)
  ja.ts      Japanese
  zh-CN.ts   Chinese Simplified
  zh-TW.ts   Chinese Traditional
  ko.ts      Korean
  fr.ts      French
  de.ts      German
  es.ts      Spanish
```

言語を追加するには: `en.ts` をコピーし、翻訳して、再ビルドする。コード変更は不要。

### config.json リファレンス

```json
{
  "active_provider": "deepseek",
  "providers": {
    "<provider_id>": {
      "display_name": "Display name",
      "upstream_url": "Anthropic-compatible API base URL",
      "api_key_env": "API key env var name",
      "default_model": "Fallback model name",
      "force_anthropic_version": null,
      "supports_count_tokens": false,
      "supports_vision": false,
      "supports_video": false,
      "model_map": { "claude-sonnet-4-5": "real-model-name" },
      "visible_models": ["claude-public-model-name"],
      "models": {
        "claude-sonnet-4-6": {
          "upstream_model": "real-model-name",
          "thinking_mode": "normal",
          "reasoning_effort": "high",
          "supports_vision": true,
          "supports_video": true,
          "visible": true
        }
      }
    },
    "openrouter": {
      "display_name": "OpenRouter",
      "upstream_url": "https://openrouter.ai/api/v1",
      "api_key_env": "OPENROUTER_API_KEY",
      "default_model": "openrouter/auto",
      "models": {
        "claude-opus-5": {
          "upstream_model": "poolside/laguna-s-2.1",
          "thinking_mode": "thinking",
          "reasoning_effort": "max",
          "supports_image_url": false,
          "supports_image_base64": false,
          "supports_video_url": false,
          "supports_video_base64": false
        },
        "claude-sonnet-5": {
          "upstream_model": "poolside/laguna-s-2.1",
          "thinking_mode": "normal",
          "supports_image_url": false,
          "supports_image_base64": false,
          "supports_video_url": false,
          "supports_video_base64": false
        },
        "claude-haiku-4-5": {
          "upstream_model": "poolside/laguna-xs-2.1",
          "thinking_mode": "thinking",
          "supports_image_url": false,
          "supports_image_base64": false,
          "supports_video_url": false,
          "supports_video_base64": false
        }
      }
    }
  },
  "non_vision_image_policy": "replace",
  "normalize_response_model_identity": true,
  "server": { "host": "127.0.0.1", "port": 4000, "enable_cors": false },
  "claude_code": {
    "auto_compact": {
      "enabled": false,
      "trigger_percent": 90
    }
  }
}
```

各プロバイダーまたは OpenRouter モデルセットは、`claude_code: { "auto_compact": { "mode": "auto" } }` でデフォルトのコンテキストモードを設定することもできる。ルートの実効モードはプロバイダー/モデルセットの値で、グローバルブロックへフォールバックする。`resolve_claude_code_auto_compact` が解決結果を返す。
