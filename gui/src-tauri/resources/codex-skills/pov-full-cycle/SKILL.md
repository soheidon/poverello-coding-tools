---
name: pov-full-cycle
description: 計画作成、計画レビュー、実装・テスト、実装レビュー、および是正実装のフルサイクルを一貫して進行・管理するCodex Skill。最大3サイクルで停止する。
---

# POV Full Cycle

## 役割と権能分離 (Separation of Powers)

- **正本計画書（Active Plan File）の解決順序**: 作業開始前に必ず確認する。
  1. **引数・プロンプト明示指定**: ユーザーが引数等で計画書パスを指定している場合は最優先する。
  2. **作業リポジトリローカル設定**: 作業ディレクトリの最近傍Gitリポジトリ（`.git`）配下の `.plan/.active-plan.json`（サーフェス固有 `surfaces.codex.active_plan_path` または legacy `active_plan_path`）を解決する。
  3. **GUI設定のプロジェクトスロット**: `CODEX_HOME/pov-review-settings.json` の `slots` から作業リポジトリルートに一致するスロットの `plan_path` を解決する。
  4. **未解決時の安全な停止 (Fail Closed)**: 対応する計画書が特定できない、または複数一致で曖昧な場合は別プロジェクトの計画書を推測・誤用せず、即座に `BLOCKED — NO ACTIVE PLAN FILE SPECIFIED: Poverello GUI の Autocode 画面から作業対象のプランファイルを指定または新規作成してください。` を出力して停止する。
- **Planner (計画作成担当)**: 指定された正本計画書（`.plan/<name>.md` 等）に初期計画を作成する。
- **Reviewer (検査担当)**: 計画書および実装差分を完全な読み取り専用（Read-Only）として検査し、合否判定（`APPROVED` / `REQUEST_CHANGES`）と指摘のみを返す。**Reviewer 自身が計画書やコードを直接編集・修正・承認改ざんすることは厳格に禁止される**。
- **Reviser / Implementer**: 計画の修正および実装・是正を担当し、修正後に再度 Reviewer の検査を仰ぐ。
- **Role assignmentの実行**: `.plan/.active-plan.json` の `role_assignments.codex` を読み、`planner_id` / `reviser_id` が `codex_session` または後方互換の `codex` のときだけ現在のCodexセッションがその工程を担当する。未設定・不正ID・未選択Active Planは開始前に停止し、別担当へ切り替えない。Reviewer設定は既存の `CODEX_HOME/pov-review-settings.json` から独立して解決する。Reviewer工程を完了・終了してからReviser/Implementer工程を別工程として実行する。
- review/correction cycleは最大3回とし、上限到達時は未解決事項、試行回数、停止理由を報告して停止する。

## 共通契約

- **正本計画書への全ライフサイクル記録 (Audit Trail)**: 計画、自己レビュー結果、外部レビュー結果、実装、テスト結果、是正履歴、最終判定を正本計画書末尾の「Execution History & Audit Trail」セクションへ時系列で漏れなく記録する。
- 同一案件では正本計画書を1件だけ作成・更新する。
- 計画承認が得られるまで実装に着手しない。
- 実装・レビュー・是正のサイクルは最大3回までに制限する。3回を超えて反復しない。
- 上限（3回）に達した場合は、残存するblocking finding、実施済み修正、テスト結果、未検証事項を報告して即座に停止する。
- 承認判定はcommit、push、tag、release、deployの許可ではない。

## 実行効率最適化ルール

既存のPlan→Review→Implement→Review→Correctiveの工程、Reviewer構成、承認条件、安全制約を維持した上で、以下の実行効率ルールを適用する。

- **コマンド実行**:
  - 短時間で完了すると見込まれるコマンドは、可能な範囲で同期的に結果を受け取る。
  - 長時間処理は非同期実行してよいが、短い間隔で繰り返し状態確認しない。
  - 完了通知を利用できる場合は通知を優先する。
  - 実行中であることを知らせるだけの応答やツール操作を繰り返さない。
- **ファイル操作**:
  - 編集前に必要なファイル範囲をまとめて読み込む。
  - 変更されていないファイルの同一範囲を繰り返し読み込まない。
  - 編集失敗時は対象箇所だけ再確認する。
  - ファイルの同一性や更新有無を確認する必要がある場合は、既存の整合性検証を省略しない。
- **テスト**:
  - 実装途中は変更対象に直接関連するテストを優先する。
  - レビュー前には必要なビルド・検証を完了させる。
  - 最終承認ゲートでは既存計画が要求するテスト一式を実行する。
  - 全件テストを再実行するのは、変更の影響範囲が拡大した場合や検証上必要な場合とする。
- **レビュー・是正**:
  - Reviewerが指摘したFindingを一覧化する。
  - 同じレビューで得られた指摘を可能な限り一括修正する。
  - 関連テストを通過してから再レビューを依頼する。
  - 既存契約で必要なレビューは省略しない。
  - 承認済みの変更を再度全面レビューする必要がある場合は、その理由を明確にする。
- **ログ・監査**:
  - コマンドの正常終了時は要約結果を確認し、長大なログ全文を毎回コンテキストに取り込まない。
  - 失敗時には詳細ログを参照できるようにする。
  - 監査記録は欠落させず、許可された範囲で工程・サイクル単位に集約する。
  - 監査上必要な時刻・判定・Finding・実行結果は維持する。

## チェックポイント保存と作業再開ルール

Active Plan File（`.plan/<name>.md`）を作業状態の永続的正本（Ground Truth）とし、会話履歴に依存しない作業再開およびコンテキスト最適化を行う。

- **チェックポイントの保存領域**:
  正本計画書の `## 5. Execution History & Audit Trail`（または該当監査セクション）直前に、専用のマーカー付き領域 `## Poverello Execution Checkpoint`（`<!-- POVERELLO-CHECKPOINT:START -->` ... `<!-- POVERELLO-CHECKPOINT:END -->`）を設けて記録する。人間が記述した計画本文や既存の監査ログを一切削除・要約・破壊しない。
- 各工程境界では、まず `append_plan_audit_trail` を現在のfile identity/HashでCAS実行し、その成功応答のidentity/Hashを使って `save_plan_checkpoint` を実行する。監査追記・チェックポイント保存の両方でidentityとSHA-256の指定が必須。チェックポイント自体の保存も正本を置換するので、戻り値のidentity/Hashをその工程の最終状態として採用する。
- **保存タイミング**:
  ツール操作ごとではなく、以下の主要な工程境界（Stage Transitions）でのみ保存する：
  1. 計画作成完了時
  2. 計画レビュー判定確定時
  3. 計画修正完了時
  4. 実装完了時（局所テスト通過後）
  5. 実装レビュー判定確定時
  6. 是正実装・検証完了時
  7. 完了（COMPLETED）または安全停止（HALTED）時
- **記録内容**:
  `Checkpoint Version: 1`、`Workflow`（対象スキル名）、`Status`、`Last Completed`（直前完了工程）、`Next Action`（次に実行すべき安全な工程）、`Execution Identity`（リポジトリ、計画ファイルパス、SHA-256、Run ID）、`Completed Work`（完了タスク一覧）、`Modified Files`（変更ファイル一覧）、`Validation`（テスト結果）、`Review State`（Reviewer構成、判定、未解決Finding一覧）、`Pending Work`（未完了タスク一覧）。
- **作業再開手順 (Resumption)**:
  Workflow開始時、Active Plan Fileのチェックポイントを確認する：
  1. リポジトリ同一性・計画ファイルパス・ハッシュ整合性を確認する。
  2. 保存されたReviewer構成、未解決Finding、変更ファイル状態を照合する。
  3. 直前完了工程（`Last Completed`）に基づき、完了済み工程の不必要な再実行をスキップし、`Next Action` から安全に再開する。
  4. リポジトリ不一致、計画の外部変更、チェックポイント破損、状態不明、未解決Blocking Finding残存時は推測でやり直さず安全に停止する。
- **コンテキスト読込削減 (Context Optimization)**:
  1. 過去の会話履歴全文や長大な監査ログを毎回再読込しない。
  2. Active Plan Fileのチェックポイントおよび現在の工程に必要なセクションのみを取得して作業を進める。
  3. 正常終了したコマンドは終了コードと要約を確認し、長大な標準出力全文をコンテキストに取り込まない。

## レビュー方式と並列レビュー契約

1. **レビュー方式（review_mode）の解決**:
   - 作業開始前にリポジトリ内 `.plan/.active-plan.json` と `CODEX_HOME/pov-review-settings.json` を読み込む。Planner/Reviserは前掲のCodex role mappingと一致するときだけこのセッションで実行し、必要roleが未設定・不正なら推測やfallbackをせず停止する。Reviewer/review modeは既存のglobal reviewer設定から取得する。
   - 計画candidateは一時領域で作成し、`apply_plan_candidate` に工程開始時の `expected_file_identity` とSHA-256を渡して反映する。監査追記eventとチェックポイント保存の両方に直前のidentityとSHA-256を渡す（いずれも必須）。各成功後に返された新しいidentityとHashを次工程へ引き継ぐ。チェックポイント保存時の `plan_revision` はチェックポイント領域を除く計画本文・監査履歴のSHA-256とし、再開時に `verify_plan_resumption` がrepo、path、revision、file identity、Findingをすべて検証しない場合は停止する。
   - `active_plan_path` が未設定、または指定されたファイルが存在しない場合、**即座に `BLOCKED — NO ACTIVE PLAN FILE SPECIFIED: Poverello GUI の Autocode 画面から作業対象のプランファイルを指定または新規作成してください。` と出力して停止する**。
   - 計画書ファイルを読み込み、そのSHA-256ダイジェストを記録する。以降のすべてのフェーズ（計画作成、計画レビュー、実装、実装レビュー、是正）の更新は、直前フェーズのSHA-256ダイジェストを `expected_sha256` として指定した `append_plan_audit_trail` による CAS（Compare-And-Swap）更新を用いて、正本計画書の `## 5. Execution History & Audit Trail` へ順次追記する。
   - 計画レビューは `plan_reviewers`（未設定時は `build_reviewers` を継承）、実装レビューは `build_reviewers`（または `review_mode`）に基づいて解決し、4区分に従って両ステージのレビュー工程を実行する：
     - `codex_self_only`: Codexの同一セッション内で自己レビューを実施する。外部Reviewerは起動せず、「外部独立レビュー: SKIPPED (review_mode: codex_self_only)」と記録する。追加の `codex exec` は起動しない。
     - `external_only`: Codex自己レビューをスキップし、設定された外部Reviewer（DeepSeek / Gemini）による独立レビューのみをディスパッチャー経由で実施する。
     - `codex_plus_external`: Codexの同一セッション内自己レビューと、選択された外部Reviewer（DeepSeek / Gemini）による独立レビューの両方を実施する。実行順序は設定上の優先Reviewer（priority_reviewer）から独立させ、一方がREQUEST_CHANGESであっても他方を省略しない。自己レビューは本セッション内で直接実施し、追加の `codex exec` は起動しない。両方の結果を別々に記録し、全Reviewer完了後に判定を統合・裁定する。
     - `none`: ユーザーの明示的選択によりレビュー工程を省略し、「レビュー状態: SKIPPED_BY_USER」と記録する。
   - 旧バージョンからの移行直後で `unconfirmed_migration == true` の場合は、ユーザーがGUIで確認・保存するまで外部Reviewerを自動起動しない（未確認での自動実行防止）。
   - `gemini_cli_enabled == false` の場合は、設定ファイル内に `reviewers.gemini: true` が残存していても実効Reviewer候補（effective reviewer set）から Gemini を除外し、ディスパッチャーへ Gemini を渡さない。
   - 実効外部Reviewer（`deepseek`, オプション有効時かつ選択時のみ `gemini`）を確認する。Codex CLIは外部Reviewer候補に含まれず、ディスパッチャーへ渡さない。外部レビュー有効時に0件選択時は外部 Reviewer を起動せず、外部独立レビュー結果を `REVIEW_SKIPPED` として該当ステージのゲートで停止する（APPROVEDと主張しない）。

2. **ステージごとの送信前同意（Stage-specific Pre-send Consent）**:
   - 外部独立レビューを実行する場合、計画レビュー時および実装レビュー時の両ステージで、同一設定を用いつつも内容の異なるステージごとに新たな送信前同意を得る。
   - レビュー対象ファイル一覧、ファイル数、合計サイズ、SHA-256ダイジェスト、送信先CLI、read-only制約を提示し、明示的同意を得る。
   - `.git`、認証・秘密情報、`.env`、巨大生成物、symlink/reparse はスナップショットから除外する。
   - 同意が得られない場合は Reviewer を起動せず `REVIEW_SKIPPED` として停止する。

3. **安全なディスパッチ**:
   - 外部独立レビューを実行する場合は、同梱の `scripts/codex-review-dispatcher.ps1` を通じて固定CLI・固定引数・read-onlyスナップショットで最大2件並列実行する。
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

1. **計画作成とレビュー**:
   - 正本計画書を作成する。
   - `pov-review-settings.json` の `review_mode` および `priority_reviewer` を解決し、指定されたレビュー工程を実施する（codex_plus_externalでは自己レビューと外部独立レビューの両方を実施し、全件完了後に判定を統合・裁定する）。
   - 承認判定が得られるまで是正と再レビューを行う（承認されるまで実装へ進まない）。
2. **実装とテスト**:
   - 承認済み計画に基づき実装を行い、テストを実行・記録する。
3. **実装レビューと是正（最大3サイクル）**:
   - 実装ステージの `review_mode` および `priority_reviewer` を解決し、指定されたレビュー工程を実施する（codex_plus_externalでは自己レビューと外部独立レビューの両方を実施し、全件完了後に判定を統合・裁定する）。
   - blocking findingがある場合は是正を行い、再テスト・再レビューを実施する。
   - サイクル回数をカウントし、3回の上限に達した場合は未解決事項・試行回数・停止理由を記録して停止する。
4. **完了と停止**: 承認判定または上限到達の報告を記録して停止する。
