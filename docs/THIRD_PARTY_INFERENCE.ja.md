[English](THIRD_PARTY_INFERENCE.md) | [日本語](THIRD_PARTY_INFERENCE.ja.md) | [中文(简体)](THIRD_PARTY_INFERENCE.zh-CN.md) | [中文(繁體)](THIRD_PARTY_INFERENCE.zh-TW.md) | [한국어](THIRD_PARTY_INFERENCE.ko.md) | [Français](THIRD_PARTY_INFERENCE.fr.md) | [Deutsch](THIRD_PARTY_INFERENCE.de.md) | [Español](THIRD_PARTY_INFERENCE.es.md)

# Claude Desktop / Cowork on 3P で Poverello を使う

Poverello は、Claude Desktop / Cowork on 3P から利用できる
ローカルの Anthropic互換ゲートウェイとして動作します。

Claude Desktop / Cowork on 3P では、開発者モードを有効にすると、
アプリ内の設定画面からサードパーティ推論を設定できます。

公式ドキュメント:

- [https://claude.com/docs/cowork/3p/installation](https://claude.com/docs/cowork/3p/installation)
- [https://claude.com/docs/cowork/3p/configuration](https://claude.com/docs/cowork/3p/configuration)

## 1. Poverello を起動する

まず Poverello を起動し、ゲートウェイを開始します。

デフォルトでは、Poverello は次のURLで待ち受けます。

```text
http://127.0.0.1:4000
```

Claude Desktop / Cowork on 3P から利用する間は、Poverello を起動したままにしてください。

## 2. Claude Desktop で開発者モードを有効にする

Claude Desktop を開きます。

Windowsでは、ログイン画面左上のアプリケーションメニューから操作します。

次を選択します。

```text
Help → Troubleshooting → Enable Developer Mode
```

開発者モードを有効にすると、`Developer` メニューが表示されます。

## 3. サードパーティ推論の設定画面を開く

次のメニューを開きます。

```text
Developer → Configure third-party inference
```

Claude Desktop のサードパーティ推論設定画面が開きます。

## 4. Connection を設定する

`Connection` セクションで、次を選択します。

```text
Gateway
```

続いて、以下の値を入力します。

| 項目                    | 値                       |
| --------------------- | ----------------------- |
| Gateway base URL      | `http://127.0.0.1:4000` |
| Gateway API key       | `sk-local-gateway`      |
| Gateway auth scheme   | `bearer`                |
| Gateway extra headers | 通常は空欄                   |

`Gateway API key` には、Poverello 側で設定したローカルAPIキーと同じ値を入力します。

## 5. Identity & Models を設定する

`Identity & Models` セクションで、Claude Desktop のモデル選択欄に表示したいモデルIDを追加します。

例:

```text
claude-sonnet-4-6
claude-haiku-4-5
```

モデルごとに表示名を付けることもできます。

例:

| Model ID            | 表示名             |
| ------------------- | --------------- |
| `claude-sonnet-4-6` | `Gateway Pro`   |
| `claude-haiku-4-5`  | `Gateway Flash` |

最初に登録したモデルが、モデル選択欄のデフォルトになります。

各モデル行を展開し、`Model ID` が Poverello に送信したいモデル名と完全に一致していることを確認してください。

`Offer 1M-context variant` は、上流プロバイダと選択モデルが拡張コンテキストに実際に対応している場合だけ有効にしてください。

## 6. 設定例

上記の設定は、内部的には次のようなサードパーティ推論設定に対応します。

```json
{
  "inferenceProvider": "gateway",
  "inferenceGatewayBaseUrl": "http://127.0.0.1:4000",
  "inferenceGatewayApiKey": "sk-local-gateway",
  "inferenceGatewayAuthScheme": "bearer",
  "inferenceModels": [
    {
      "name": "claude-sonnet-4-6",
      "labelOverride": "Gateway Pro"
    },
    {
      "name": "claude-haiku-4-5",
      "labelOverride": "Gateway Flash"
    }
  ]
}
```

## 7. 適用して Claude Desktop を再起動する

ゲートウェイとモデルリストを設定したら、設定をローカルに適用します。

必要に応じて Claude Desktop を再起動してください。

再起動後、Claude Desktop / Cowork on 3P からのリクエストは Poverello に送られます。Poverello は、そのリクエストを設定済みの上流プロバイダへ中継します。

## Direct DeepSeek API

Poverello で Direct DeepSeek プロバイダーを選択すると、Claude Desktop / Cowork on 3P からのリクエストは DeepSeek の Anthropic Messages 互換エンドポイントへ送られます。

正規 DeepSeek API モデルIDは `deepseek-flash` と `deepseek-v4-pro` です。現在の公式料金表では、それぞれ DeepSeek-V4.1-Flash-1017 と DeepSeek-V4-Pro-0813 として掲載されています。これらは上流APIのIDです。`inferenceModels` に設定するClaude側の表示名はローカルの選択項目であり、Poverelloが選択中のプロバイダープロフィールへ対応付けます。

両モデルはツール呼び出し、1Mトークンのコンテキスト、最大384Kの出力に対応します。画像入力に対応するのはFlashのみです。DeepSeekで廃止された `deepseek-v4-flash` と `deepseek-v4-flash-vision-exp` は、現在もFlashへの上流エイリアスとして受け付けられます。PoverelloのDirect DeepSeek組み込み設定は正規IDを使用します。Proの転送・課金について公式資料間で説明が異なります。現在の料金表は個別のPro料金と9月14日以降のAPI提供継続を記載する一方、9月10日の告知はV4.1-Pro公開までV4.1 Flashへ一時転送しFlash料金を適用するとしていました。Poverelloはリクエストごとの転送先・請求額を確認できません。表示額は公開料金表の値であり、請求額や保証ではありません。[現在の料金表](https://api-docs.deepseek.com/quick_start/pricing/)と[9月10日の告知](https://api-docs.deepseek.com/news/news260910/)を参照してください。

最新のトークン単価とピーク／オフピーク規則は、[DeepSeek公式価格表](https://api-docs.deepseek.com/quick_start/pricing/)および[プロバイダー詳細ガイド](PROVIDERS.md#deepseek)を参照してください。Poverello同梱の休日カレンダーはローカルのピーク／オフピーク料金判定に使用されます。収録状況とフォールバック動作もプロバイダー詳細ガイドに記載しています。

## Xiaomi MiMo プロバイダー

Poverello が Direct MiMo プロバイダーを使用するよう設定されている場合、Claude Desktop からの推論リクエストは小米（Xiaomi）の MiMo API にルーティングされます。

### MiMo-V2.6 モデル

| Model ID | 表示名 | マルチモーダル |
| --- | --- | --- |
| `mimo-v2.6-flash` | MiMo-V2.6-Flash | Image + Video |
| `mimo-v2.6-pro` | MiMo-V2.6-Pro | Image + Video |
| `mimo-v2.6-pro-ultraspeed` | MiMo-V2.6-Pro-UltraSpeed | Image + Video |

V2.6 の全モデルは 1,000,000 トークンのコンテキストウィンドウに対応しています。

### 通常 / Thinking

MiMo はシンプルな通常／Thinking トグルを使用します。推論努力レベル（Low / Medium / High）はありません。

| 設定 | 上流APIパラメータ | 動作 |
| --- | --- | --- |
| 通常 | `thinking: {"type": "disabled"}` | 標準推論 |
| Thinking | `thinking: {"type": "enabled"}` | 拡張思考 |

### デフォルトルート

Direct MiMo プリセットの組み込みルート:

| ゲートウェイモデル | 上流モデル | Thinking |
| --- | --- | --- |
| `claude-opus-5` | `mimo-v2.6-pro` | Thinking |
| `claude-sonnet-5` | `mimo-v2.6-pro` | 通常 |
| `claude-haiku-4-5` | `mimo-v2.6-flash` | Thinking |

### V2.5 後方互換性

保存済みの `mimo-v2.5`、`mimo-v2.5-pro`、`mimo-v2.5-pro-ultraspeed` 設定は保持され、引き続き動作します。起動時に自動移行されるのは、変更されていないファクトリーデフォルトルートのみです。カスタマイズ済みのルート、Thinking モード設定、およびカスタムの `default_model` 値は上書きされません。

## MiniMax-M3 Thinkingモード

MiniMax-M3は、Anthropic互換APIを通じてThinking（拡張思考）機能のオン／オフ切り替えに対応しています。

| 設定 | 上流APIパラメータ | 動作 |
| ---- | ---------------- | ---- |
| Thinking ON | `thinking: {"type": "adaptive"}` | 拡張思考を有効化 |
| Thinking OFF | `thinking: {"type": "disabled"}` | 拡張思考を無効化 |
| デフォルト（未設定） | *（省略）* | API既定値：Thinking無効 |

Poverelloの設定画面では、MiniMax-M3の行にThinking／通常のトグルが表示されます。

MiniMax-M2.x系モデル（`MiniMax-M2.7-highspeed`）はThinkingの無効化に対応しておらず、「Thinking専用」モード固定となります。

## Laguna S/XS 2.1（OpenRouter）設定

Poverello は OpenRouter の `claude-opus-5` と `claude-sonnet-5` を `poolside/laguna-s-2.1` へ、`claude-haiku-4-5` を `poolside/laguna-xs-2.1` へデフォルトでルーティングします。

### Thinkingの動作

サードパーティのテストにおいて、Laguna S 2.1 の Thinking 動作はシステムプロンプトの構造に影響を受けることが観測されています。明確な職業ペルソナと明示的な受諾基準を含めることで、不必要な推論が大幅に減少したという報告があります。

これらのテスト結果に基づき、Laguna Opus（`claude-opus-5`）のデフォルトは「通常」（Thinkingオフ）モードに変更されました。設定画面から再びThinkingを有効化することも可能です。

### 既知の制限：トークン上限時の無応答

特定の条件下 — 特にモデルが推論フェーズにある状態で1ターンあたりのトークン上限に達した場合 — Laguna S 2.1 は推論コンテンツのみを含み、テキスト出力やツール呼び出しがなく、`stop_reason` が `max_tokens` の応答を返すことがあります。このような応答を受け取ったクライアントは会話を継続できなくなる可能性があります。

Poverello はこの状態を検出し、ログパネルに警告を出力します。「Reasoning-only response reached the per-turn token limit」という警告が繰り返し表示される場合は、最大出力トークン数を引き上げるか、該当モデルのThinkingをオフにすることを検討してください。

これらの観測はサードパーティのコミュニティテストに基づきます。結果はプロバイダの設定やモデルのリビジョンにより異なる場合があります。

## 注意

Poverello は、Anthropic互換APIのための非公式ローカルゲートウェイです。

Anthropic、Moon Bridge、および各モデル提供元とは提携・関係ありません。

Claude Desktop / Cowork on 3P のサードパーティ推論設定画面やメニュー名は、Claude Desktop のアップデートにより変更される可能性があります。


> [!NOTE]
> **Claude Desktop / Cowork on 3P は Gateway 経由でクラウドプロバイダーを使用します**
>
> Claude Desktop および Cowork on 3P は、Poverello Gateway（`http://127.0.0.1:4000`）へリクエストを送信し、設定されたクラウドプロバイダー（DeepSeek、MiMo、Kimi、MiniMax、OpenRouter）へルーティングされます。
>
> **Ollama Local** バックエンドは **Claude Code CLI 専用** であり、Claude Desktop や MCP とは完全に独立しています。Claude Code でのローカルLLM利用については [Ollama Local ガイド](OLLAMA_LOCAL.ja.md) をご覧ください。
