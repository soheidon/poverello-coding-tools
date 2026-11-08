---
name: pov-plan-only
description: 計画作成と独立計画レビュー専用のCodex Skill。承認判定後も実装へ進まず計画記録のみで停止する。
---

# POV Plan Only

## 役割と権能分離 (Separation of Powers)

- **正本計画書（Active Plan File）**: 作業開始前に必ず確認する。GUIでアクティブプランファイルが指定されていない場合、即座に `BLOCKED — NO ACTIVE PLAN FILE SPECIFIED` を出力して停止する。
- **Planner (計画作成担当)**: 指定された正本計画書（`.plan/<name>.md` 等）に初期計画を作成する。
- **Reviewer (検査担当)**: 計画書を完全な読み取り専用（Read-Only）として検査し、合否判定（`APPROVED` / `REQUEST_CHANGES`）と具体的根拠付き指摘（P0/P1/P2）のみを返す。**Reviewer 自身が正本計画書を直接編集・修正・承認改ざんすることは厳格に禁止される**。
- **Reviser (計画修正担当)**: Reviewer の指摘を受けて計画書を修正する担当。修正は必ず Reviser（Codex作業セッション等）が行い、修正完了後に正本計画書を固定して再度 Reviewer の検査を仰ぐ。
- 承認判定（`APPROVED` / `APPROVED_WITH_NON_BLOCKING_FINDINGS`）が得られた場合でも、実装へ進まず計画記録のみで停止する。
- コード、テスト、設定は変更しない。

## 共通契約

- **正本計画書への全ライフサイクル記録 (Audit Trail)**: 計画、自己レビュー結果、外部レビュー結果、是正履歴、最終判定を正本計画書末尾の「Execution History & Audit Trail」セクションへ時系列で漏れなく記録する。
- 同一案件では正本計画書を1件だけ作成・更新し、別版や依頼書を作らない。
- Blocking finding（P0/P1）は、(1) 反する要件・契約、(2) 具体的失敗条件、(3) 必要な最小修正、(4) 判定可能なテスト/証拠の4点を満たすものに限る。
- Approved判定は実装、commit、push、tag、release、deployの許可ではない。

## チェックポイント保存と作業再開ルール

Active Plan File（`.plan/<name>.md`）を作業状態の永続的正本（Ground Truth）とし、会話履歴に依存しない作業再開およびコンテキスト最適化を行う。

- **チェックポイントの保存領域**:
  正本計画書の `## 5. Execution History & Audit Trail`（または該当監査セクション）直前に、専用のマーカー付き領域 `## Poverello Execution Checkpoint`（`<!-- POVERELLO-CHECKPOINT:START -->` ... `<!-- POVERELLO-CHECKPOINT:END -->`）を設けて記録する。人間が記述した計画本文や既存の監査ログを一切削除・要約・破壊しない。
- **保存タイミング**:
  1. 計画作成完了時
  2. 計画レビュー判定確定時
  3. 計画修正完了時
  4. 完了（COMPLETED）または停止時
- **記録内容**:
  `Checkpoint Version: 1`、`Workflow: pov-plan-only`、`Status`、`Last Completed`、`Next Action`、`Execution Identity`（リポジトリ、計画ファイルパス、SHA-256、Run ID）、`Completed Work`、`Validation`、`Review State`、`Pending Work`。
- **作業再開手順 (Resumption)**:
  計画作成・レビュー開始時、Active Plan Fileのチェックポイントを確認する：
  1. リポジトリ同一性・計画ファイルパス・ハッシュ整合性を確認する。
  2. 直前完了工程（`Last Completed`）に基づき、確定済みの計画と未解決Findingを活用して `Next Action` から安全に再開する。
  3. 不一致や外部変更検出時は推測でやり直さず安全に停止する。
- **コンテキスト読込削減 (Context Optimization)**:
  1. 過去の会話履歴全文や長大な監査ログを毎回再読込しない。
  2. Active Plan Fileのチェックポイントおよび現在の工程に必要なセクションのみを取得して作業を進める。

## レビュー方式と並列レビュー契約

1. **レビュー方式（review_mode）の解決**:
   - 作業開始前にリポジトリ内 `.plan/.active-plan.json` または `CODEX_HOME/pov-review-settings.json` を読み込む。
   - `active_plan_path` が未設定、または指定されたファイルが存在しない場合、**即座に `BLOCKED — NO ACTIVE PLAN FILE SPECIFIED: Poverello GUI の Autocode 画面から作業対象のプランファイルを指定または新規作成してください。` と出力して停止する**。
   - 計画書ファイルを読み込み、そのSHA-256ダイジェストを記録する。以降のすべてのフェーズ（計画作成、レビュー判定、是正）の更新は、直前フェーズのSHA-256ダイジェストを `expected_sha256` として指定した `append_plan_audit_trail` による CAS（Compare-And-Swap）更新を用いて、正本計画書の `## 5. Execution History & Audit Trail` へ順次追記する。
   - 計画レビュー担当Reviewerは `plan_reviewers`（未設定時は `build_reviewers` または `review_mode` を継承）に基づいて解決し、4区分に従ってレビュー工程を実行する：
     - `codex_self_only`: Codexの同一セッション内で自己レビューを実施する。外部Reviewerは起動せず、「外部独立レビュー: SKIPPED (review_mode: codex_self_only)」と記録する。追加の `codex exec` は起動しない。
     - `external_only`: Codex自己レビューをスキップし、設定された外部Reviewer（DeepSeek / Gemini）による独立レビューのみをディスパッチャー経由で実施する。
     - `codex_plus_external`: Codexの同一セッション内自己レビューと、選択された外部Reviewer（DeepSeek / Gemini）による独立レビューの両方を実施する。実行順序は設定上の優先Reviewer（priority_reviewer）から独立させ、一方がREQUEST_CHANGESであっても他方を省略しない。自己レビューは本セッション内で直接実施し、追加の `codex exec` は起動しない。両方の結果を別々に記録し、全Reviewer完了後に判定を統合・裁定する。
     - `none`: ユーザーの明示的選択によりレビュー工程を省略し、「レビュー状態: SKIPPED_BY_USER」と記録する。
   - 旧バージョンからの移行直後で `unconfirmed_migration == true` の場合は、ユーザーがGUIで確認・保存するまで外部Reviewerを自動起動しない（未確認での自動実行防止）。
   - `gemini_cli_enabled == false` の場合は、設定ファイル内に `reviewers.gemini: true` が残存していても実効Reviewer候補（effective reviewer set）から Gemini を除外し、ディスパッチャーへ Gemini を渡さない。
   - 実効外部Reviewer（`deepseek`, オプション有効時かつ選択時のみ `gemini`）を確認する。Codex CLIは外部Reviewer候補に含まれず、ディスパッチャーへ渡さない。外部レビュー有効時に0件選択時は外部 Reviewer を起動せず、外部独立レビュー結果を `REVIEW_SKIPPED` として計画承認ゲートで停止する（APPROVEDと主張しない）。

2. **送信前同意（Pre-send Consent）**:
   - 外部独立レビューを実行する場合、レビュー対象ファイル一覧、ファイル数、合計サイズ、SHA-256ダイジェスト、送信先CLI、read-only制約を提示し、送信前の明示的同意を得る。
   - `.git`、認証・秘密情報、`.env`、巨大生成物、symlink/reparse はスナップショットから除外する。
   - 同意が得られない場合は Reviewer を起動せず `REVIEW_SKIPPED` として停止する。

3. **安全なディスパッチ**:
   - 外部独立レビューを実行する場合は、同梱の `scripts/codex-review-dispatcher.ps1` を通じて固定CLI・固定引数・read-onlyスナップショットで実行する。
   - `codex` は外部Reviewerとして受け付けず、ディスパッチャーへ渡さない。
   - スナップショットおよびその全配下ファイルの symlink/reparse point を再帰検証し、外部参照や脱出経路が存在する場合は即座に実行拒否する。
   - Windows環境ではプロセスを `CREATE_SUSPENDED` で生成し、親パイプハンドルの継承制御を行ったうえで Windows Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) に割り当てた後に `ResumeThread` で実行する。タイムアウト・キャンセル・異常終了時は `TerminateJobObject` により子孫プロセスを含めて確実に強制終了し、回収を確認する（終了未確認時は自動再試行せず結果不明として停止）。
   - Gemini CLI は、公式sandboxが実効的に検証されない限り起動せず、即座に `UNAVAILABLE` として報告する。モデルは `gemini-2.5-pro` または `gemini-2.5-flash` のみを受け付ける。
   - DeepSeek Harness は公式headless仕様（位置引数タスク: `dsh --profile headless <prompt>`）に従い、読み取り専用の実行制約が検証されない限り起動せず即座に `UNAVAILABLE` として報告する。未対応のCLIフラグ（`--model`, `--reasoning-effort`, `--permission-mode`, `--approval-policy`）は渡さない。
   - stdoutは各1 MiBでキャップし、stderr本文（推論・トークン等）は保存・返却しない。

4. **結果の統合と判定（§19 裁定契約）**:
   - Codex自身のセッション内自己レビュー判定と、外部Reviewerによる独立レビュー判定は報告上も明確に分離して記録する。
   - 自己レビュー結果を外部独立承認へすり替えない。
   - 各Reviewerの有効な最終判定（`APPROVED` または `REQUEST_CHANGES`）が揃った場合にのみ判定を統合・裁定する。
   - 外部Reviewerのいずれかが失敗・timeout・unavailable・truncated（1 MiB超過）・未実行の場合、バッチ全体を `INCOMPLETE` とし、優先Reviewerの設定にかかわらず承認としない。
   - **判定一致**: 全Reviewerの有効判定が一致した場合、その判定を採用する。
   - **判定不一致の裁定**: 全有効判定が揃った上で判定が食い違う場合、実効Reviewer集合に含まれる `priority_reviewer` の判定を採用する。
   - **Blocking 指摘等のオーバーライド禁止**: いずれかのReviewerに未解決の具体的Blocking Finding、セキュリティ違反、必須受入テスト失敗、または計画上の非免除条件がある場合、priority reviewer が `APPROVED` であっても全体判定を `APPROVED` にできない（`REQUEST_CHANGES` または `INCOMPLETE`）。priorityで無視できる指摘は、具体的根拠に基づき非Blockingと判断された指摘のみとする。
   - 優先されなかったReviewerの判定、指摘事項、重大度、根拠はすべて維持し、最終報告やログから削除・隠蔽しない。

## 進行手順

1. **計画作成**: 要件・制約を確認し、正本計画書を作成する。
2. **レビュー工程**:
   - `pov-review-settings.json` の `review_mode` を解決する。
   - `codex_self_only`: セッション内でCodex自己レビューを実施し、結果を記録する。
   - `external_only`: スナップショットを合成し、送信前同意を得た上でディスパッチャー経由で外部独立レビューを実行・記録する。
   - `codex_plus_external`: セッション内自己レビューおよびディスパッチャー経由の外部独立レビューの双方を実行・記録し、全件完了後に判定を統合・裁定する（一方がREQUEST_CHANGESでも他方を省略しない）。
   - `none`: レビューを省略（`SKIPPED_BY_USER`）として記録する。
   - blocking findingがあれば計画書を是正し、再レビューを行う。
3. **停止**: 計画承認判定を記録して停止し、ユーザーの指示を待つ。
