---
name: pov-full-cycle
description: 計画作成、外部Codex/DeepSeek独立計画レビュー、実装・テスト、外部独立実装レビュー、および是正実装のフルサイクルを一貫して進行・管理するSkill。
---

# POV Full Cycle

## 役割と権能分離 (Separation of Powers)

- **正本計画書（Active Plan File）の解決順序**: 作業開始前に必ず確認する。
  1. **引数・プロンプト明示指定**: ユーザーが引数等で計画書パスを指定している場合は最優先する。
  2. **作業リポジトリローカル設定**: 作業ディレクトリの最近傍Gitリポジトリ（`.git`）配下の `.plan/.active-plan.json`（サーフェス固有 `surfaces.antigravity.active_plan_path` または legacy `active_plan_path`）を解決する。
  3. **GUI設定のプロジェクトスロット**: `%USERPROFILE%\.gemini\antigravity-ide\pov-reviewer-policy.json` の `slots` から作業リポジトリルートに一致するスロットの `plan_path` を解決する。
  4. **未解決時の安全な停止 (Fail Closed)**: 対応する計画書が特定できない、または複数一致で曖昧な場合は別プロジェクトの計画書を推測・誤用せず、即座に `BLOCKED — NO ACTIVE PLAN FILE SPECIFIED: Poverello GUI の Autocode 画面から作業対象のプランファイルを指定または新規作成してください。` を出力して停止する。計画書ファイルを読み込み、そのSHA-256ダイジェストを記録する。以降のすべてのフェーズ（計画作成、計画レビュー、実装、実装レビュー、是正）の更新は、直前フェーズのSHA-256ダイジェストを `expected_sha256` として指定した `append_plan_audit_trail` による CAS（Compare-And-Swap）更新を用いて、正本計画書の `## 5. Execution History & Audit Trail` へ順次追記する。
- 計画candidateは一時領域で作成し、`apply_plan_candidate` に工程開始時の `expected_file_identity` とSHA-256を渡して反映する。監査追記eventとチェックポイント保存の両方にidentityとSHA-256を指定し（いずれも必須）、各信頼済み書込み後に返されたidentity/Hashを次工程へ引き継ぐ。再開時は `get_plan_checkpoint` と `verify_plan_resumption` を実行し、repo、path、revision、file identity、Findingの全検査が成功しなければ停止する。
- **Planner (計画作成担当)**:
  - Plannerは `.plan/.active-plan.json` の `role_assignments.antigravity.planner_id` を用いる。未設定・不正なら停止し、global設定や別roleへのfallbackを禁止する。
  - `planner_role` が `codex`（推奨）の場合: Antigravity によるハルシネーション・代筆を完全に禁止。Antigravity は自ら計画を書かず、`codex exec` を起動して計画書を作成させる。
  - `planner_role` が `antigravity` の場合: Antigravity が計画書を作成する。
- **Reviewer (検査担当)**: 外部Reviewer（Codex CLI / DeepSeek Harness）は計画レビュー・実装レビューともに完全な読み取り専用（Read-Only）として検査し、合否判定（`APPROVED` / `REQUEST_CHANGES`）と指摘のみを返す。**Reviewer 自身による計画書・コードの直接変更は禁止される**。
- **Reviser / Implementer**: 計画の修正および実装・是正を担当し、修正後に再度 Reviewer の検査を仰ぐ。
  - Reviserは `.plan/.active-plan.json` の `role_assignments.antigravity.reviser_id` を用いる。未設定・不正なら停止し、別roleへのfallbackを禁止する。
- **Role assignmentの実行**: 設定値 `antigravity` は現在のAntigravityセッション、`codex` / `codex_cli` は `codex exec` によるCodex CLI実行を意味する。PlannerまたはReviser工程前に保存値とActive Plan Fileを再確認し、未設定・不正ID・実行失敗時は担当名と理由を報告して停止する。Reviewer工程を終了した後にだけReviser/Implementerを別工程として実行し、同じIDでもReviewerのread-only制約を共有・解除しない。
- 各工程境界では監査イベントをfile identity/HashでCAS追記し、その成功応答を使ってチェックポイントを保存する。チェックポイント保存後のidentity/Hashを次工程の期待状態にする。
- 外部Reviewer構成は、共通設定ファイル `%USERPROFILE%\.gemini\antigravity-ide\pov-reviewer-policy.json`（他OSでは対応するホームディレクトリ配下）の設定に従う。
- 実装コード、テスト、設定を変更できるのは実装担当だけである。外部Reviewerはread-onlyとする。
- `pov-mcp-plan`、`pov-mcp-review`、`pov-mcp-revise`はPoverello MCP用の別Skillであり、このSkillから呼ばない。

## 共通契約

- **正本計画書への全ライフサイクル記録 (Audit Trail)**: 計画、レビュー結果、実装報告、是正計画、テスト結果、最終判定を正本計画書末尾の「Execution History & Audit Trail」セクションへ時系列で漏れなく記録する。
- 外部CLIは各グローバルルールに従って実行する。内部エージェントで代替しない。
- 同一案件では正本計画書を1件だけ使う。別版、レビュー依頼書、修正計画書を作らない。
- レビュー実行前に実効ポリシー（評価優先順位: Evaluation Precedence）を決定する:
  1. **実行時明示指定:** コマンドライン引数（`--review none` 等）またはユーザープロンプト内の明示指示（「Codex CLI や DeepSeek Harness によるレビューは不要」「外部レビュー不要」等）が存在する場合は、最優先でその値を実効ポリシーとする。
  2. **GUI設定ファイル:** 実行時指定がない場合、共通設定ファイル `%USERPROFILE%\.gemini\antigravity-ide\pov-reviewer-policy.json`（`schema_version: 6`）またはリポジトリ内 `.plan/.active-plan.json` を読み取る。フェーズ2（計画レビュー）では個別設定の `plan_review_policy`、フェーズ4（実装レビュー）では `build_reviewers`（複数選択リスト、未指定時は `build_review_policy` / `policy`）を採用する。計画レビュー個別設定がない場合は実装レビュー担当の選択をそのまま適用する（複数Reviewer選択時は有効な `priority_reviewer` ("codex_cli" または "deepseek") が必須。未選択または旧schemaの場合はマイグレーション未完了として dispatch を拒否し、`none` 相当で停止する）。Reviewer実行時のモデルおよび推論強度は同設定内の `codex`（`model`, `reasoning_effort`）および `deepseek`（`model`, `reasoning_effort`）に従う。
  3. **安全側既定値 (Fail-Safe):** ファイル不在、構文エラー、未知の policy 値、未対応 schema_version、未選択 priority、書込障害等の例外時は、安全側既定値 `none` (`Disabled`) を適用する。
- `codex_only` では Codex CLI のみを使用し、DeepSeek Harness は起動・待機・比較・Synthesis のいずれにも使用しない。DeepSeek Rule の有無も要求しない。Codex CLI 失敗時に他Reviewerへ暗黙フォールバックしない。
- `deepseek_only` では DeepSeek Harness (`dsh headless`) のみを使用し、Codex CLI は起動・待機・比較・Synthesis のいずれにも使用しない。Codex CLI への暗黙フォールバックは絶対に行わない。DeepSeek 実行時のモデルおよび推論強度は同設定内の `deepseek`（`model`, `reasoning_effort`）に従う。
- `codex_plus_deepseek` では Codex と DeepSeek に同一ハッシュの資料（計画書・コード・差分・テスト証拠）を渡し、互いの回答を見せない。
- Reviewerが自分でテストを再実行していない場合、「テストが通ったことを確認した」と記録せず、「実装者が提示した実行証拠を確認した」と記録する。Reviewer自身が再実行した結果とは明確に分離する。
- 最終承認状態は`APPROVED`、`APPROVED_WITH_NON_BLOCKING_FINDINGS`、`REQUEST_CHANGES`、`BLOCKED`の4種だけを使う。「条件付き承認」「完了推奨」等の自由文を自動的に`APPROVED`へ丸めない。
- `APPLY_AND_FINALIZE`は最終承認状態ではなく、SynthesisがAntigravityへ返す限定的な修正指示である。指定修正と検証が成功した場合に、`APPROVED`または`APPROVED_WITH_NON_BLOCKING_FINDINGS`へ遷移する。
- blocking findingは、承認済み要件・安全契約・受入条件への具体的な違反であり、次の4点をすべて伴う場合に限る。
  1. どの承認済み要件・安全契約・受入条件に反するか。
  2. 具体的にどの入力・状態・処理順で失敗するか。
  3. 承認に必要な最小修正は何か。
  4. 修正済みかを判定できる具体的なtestまたは証拠は何か。
- 文体、好み、将来強化、計画外の追加機能、一般論、未採用の脅威モデルだけをblocking findingにしない。同一原因の言い換えを新しいblocking findingとして数えない。
- 新たに確認された重大な不具合はblockingにできるが、同じ4条件を満たすこと。
- 修正版で既存findingの解決条件を満たした後、別の任意改善だけを理由にレビューを長期化させない。
- P2だけで修正・再レビューしない。ただしP2が計画の明示的受入条件を破壊し、具体的な失敗を生む場合はblockingとして扱える。
- 回数、時間、再試行、停止条件はグローバルルールを適用する。
- 実装前作業ツリー状態と既存変更を記録し、今回の作業差分と既存差分を混同しない。reset、clean、stash等は許可がない限り行わない。
- 計画承認および実装レビューのApprovedは、commit、push、tag、release、deploy、本番変更、実機受入の承認ではない。
- ユーザーが`pov-full-cycle`を明示的に呼び出した場合、その呼び出しは、計画承認後に同一依頼範囲内の実装、自動テスト、独立実装レビュー、承認範囲内の是正まで進める許可を含む。計画承認後に同じ実装許可を再取得しない。
- ただし、live model、ステージング、本番相当環境、課金を伴う外部サービス、資格情報利用、commit、push、tag、release、deploy、本番変更は、計画またはグローバルルールで明示的に許可されていない限り別ゲートとする。

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

## 1. 計画作成

1. 指示書、リポジトリ、ローカルルール、既存計画、関連コードを確認する。
2. 同じ目的の正本計画書があれば再利用し、なければ1件だけ作る。
3. 目的、確認事実と仮説、範囲、変更対象、不変条件、実装方法、テスト、停止点を記載する。
4. Antigravityが内部レビューし、事実誤認、矛盾、不足を計画書内で修正する。

## 2. 独立計画レビュー

1. 現在の計画書と必要なコードを固定する。外部Reviewerはread-onlyとする。
2. 共通設定の `plan_reviewers`（未指定時または実装レビュー継承時は `build_reviewers` または `plan_review_policy` / `policy`）に従ってレビューを実施する：
   - **`none` の場合（外部レビュー無効）**：
     - 外部レビューア呼出コマンド（`codex exec`, `poverello/review` 等）を一切生成・実行しない（呼出数厳密に 0）。
     - 独立レビューループを開始しない。
     - 計画フェーズ完了後、`PLAN COMPLETE — EXTERNAL REVIEW DISABLED (Awaiting user approval)` を記録して停止し、ユーザーの明示的承認を待つ（自動で実装フェーズへ突入しない）。
     - **No-Fallback 原則:** レビューア利用不可・エラー等のいかなる例外状況下でも、Codex や DeepSeek へのフォールバック呼出を絶対に実行しない。
   - **Antigravity のみ選択の場合（自己レビュー）**：
     - 外部CLI（Codex CLI、DeepSeek Harness）呼出コマンドを一切生成・実行しない（呼出数厳密に 0）。
     - Antigravity自身が計画書を検査し、自己レビュー所見と合否判定を正本計画書へ記録する。
   - **Codex CLI 単独選択（`codex_only`）の場合**：
     - 固定した計画書とコードを外部 Codex CLI のみへ渡す。DeepSeek Harness は一切呼び出さない（呼出数厳密に 0）。
     - 各findingにつき、対象要件、具体的な失敗条件、severity/blocking、必要な最小修正、合否判定可能なtestまたは証拠を報告させる。
     - Synthesis は行わず、Codex 単一のレビュー結果に基づき合否判定を行う。
   - **DeepSeek Harness 単独選択（`deepseek_only`）の場合**：
     - 固定した計画書とコードを外部 DeepSeek Harness (`dsh headless`) のみへ渡す。Codex CLI は呼び出さない（呼出数厳密に 0）。
     - 各findingにつき、対象要件、具体的な失敗条件、severity/blocking、必要な最小修正、合否判定可能なtestまたは証拠を報告させる。
     - Synthesis は行わず、DeepSeek 単一のレビュー結果に基づき合否判定を行う。
     - DeepSeek が利用不可またはエラーの場合、Codex CLI への暗黙フォールバックは絶対に行わず `INCOMPLETE` / `UNAVAILABLE` として停止する。
     - `REQUEST_CHANGES` の場合、指定された Reviser（GUIで設定されたAI）が正本計画書を修正する（DeepSeek Reviewer自身は編集しない）。
   - **Antigravity ＋ Codex CLI 選択の場合**：
     - DeepSeek Harness は一切呼び出さない（呼出数厳密に 0）。
     - Antigravity（自己レビュー）および Codex CLI（外部独立レビュー）の両者がレビューを実施し、所見を正本計画書へ記録する。
   - **Antigravity ＋ DeepSeek Harness 選択の場合**：
     - Codex CLI は一切呼び出さない（呼出数厳密に 0）。
     - Antigravity（自己レビュー）および DeepSeek Harness（外部独立レビュー）の両者がレビューを実施し、所見を正本計画書へ記録する。
   - **Codex CLI ＋ DeepSeek Harness 選択（`codex_plus_deepseek`）の場合**：
     - 同一版を Codex CLI と DeepSeek Harness CLI へ別々に渡す。
     - 各Reviewerは、findingごとに次を示す：対象要件、具体的な失敗条件、severity/blocking、必要な最小修正、合否判定可能なtestまたは証拠。
     - Antigravityはblocking findingが「共通契約」の4条件を満たすことを確認し、重複・同一原因の言い換え・範囲外の任意改善を統合または非blockingへ降格する。
     - **判定統合と裁定（Arbitration）契約:**
       - 両Reviewer（Codex CLI と DeepSeek Harness）に同一版を渡し、両方の結果を待つ（実行順序の変更や一方の失敗による早期停止は行わない）。
       - いずれかのReviewerが `INCOMPLETE`、`UNAVAILABLE`、`ERROR`、タイムアウト、未実行、打ち切りの場合、全体を `INCOMPLETE` とする（優先設定で上書きしない。DeepSeek Harness のWindows読み取り専用条件が未達の間は `UNAVAILABLE` となり、Codex単独への暗黙フォールバックは禁止）。
       - 両Reviewerが有効判定（`APPROVE` または `REQUEST_CHANGES`）を返した場合：
         - 判定が一致していれば、その共通判定を採用する。
         - 判定が食い違った場合、保存された `priority_reviewer`（`codex_cli` または `deepseek`）の判定を採用する。
         - **Blocking Finding 例外規定:** いずれかのReviewerが未解決の Blocking Finding、安全性違反、または必須検査失敗を指摘している場合、優先Reviewerが `APPROVE` であっても全体を `APPROVE` にしてはならず、`REQUEST_CHANGES`（または実行・環境問題なら `INCOMPLETE`）とする。
       - 優先されなかった側の指摘、根拠、severity もすべて最終報告と正本計画書に記録し、破棄しない。
   - **3者すべて（Antigravity ＋ Codex CLI ＋ DeepSeek Harness）選択の場合**：
     - 3者それぞれのレビュー結果を収集する。Antigravityの自己レビュー所見を記録するとともに、Codex CLI と DeepSeek Harness の外部独立レビュー結果に対して上記裁定（Arbitration）契約を適用する。
3. Synthesis（または `codex_only` 時のCodex判定）は次のいずれかを返す。
   - `APPROVE`
   - `APPROVE_WITH_NON_BLOCKING_FINDINGS`
   - `APPLY_AND_FINALIZE`
   - `REQUEST_CHANGES`
   - `BLOCKED`
4. `APPLY_AND_FINALIZE`は、承認に必要な修正が限定的・具体的・機械的に適用可能で、新しい設計判断を必要としない場合だけ使用する。Synthesisは変更箇所、置換内容または変更条件、確認方法、修正後に到達すべき最終承認状態を明示する。
5. `APPLY_AND_FINALIZE`の場合、Antigravityは指定された修正だけを正本計画書へ適用し、指定された静的検査・自己整合確認を行う。指定外の設計変更を加えない。確認に合格すれば外部再レビューへ戻らず、Synthesisが指定した最終承認状態を記録する。確認に失敗した場合は`REQUEST_CHANGES`として扱う。
6. `REQUEST_CHANGES`は設計判断を伴う修正、複数の妥当な選択肢がある修正、未確定な安全条件、または修正後の妥当性を外部独立レビューで再確認すべき場合に使用する。Antigravityが計画修正後、有効な policy に従って再度独立レビューさせる。
7. P2だけでは再レビューせず、グローバルの回数・時間・同一finding上限を超えてはならない。
8. 計画承認（`APPROVED`または`APPROVED_WITH_NON_BLOCKING_FINDINGS`）が得られるまで実装に着手しない。
9. `pov-full-cycle`の呼び出し時点で実装まで許可されているため、計画承認後は同じ依頼範囲内の実装へそのまま進む。ただし共通契約で別ゲートとした操作は実行しない。

## 3. 実装と自己レビュー

1. Antigravityが承認済み計画の範囲だけを実装する。
2. 指定テストと必要な回帰テストを実行する。実行したコマンド、終了コード、要約を記録する。
3. テスト未実行、実行不能、結果不明はPASSにしない。`UNVERIFIED`または具体的な停止理由として記録する。
4. Antigravityが実装を自己レビューし、計画との差分、不変条件、未検証事項、逸脱を確認する。
5. 変更ファイル、実装内容、差分、テスト証拠、未検証事項、逸脱、必要な是正候補を正本計画書へ記録する。
6. 実機・ステージング・本番相当の受入が計画で別ゲートなら、自動テスト完了を「改修全体完了」と呼ばない。`CODE_IMPLEMENTATION_APPROVED`や`STAGING_ACCEPTANCE_PENDING`等、実際の到達点を明記する。

## 4. 独立実装レビューと是正

1. 現在の実装、正確な差分、承認済み計画、実装者のテスト証拠、未検証事項を固定する。
2. 外部Reviewerはread-onlyとする。
3. 共通設定の `build_reviewers`（未指定時は `build_review_policy` / `policy`）に従ってレビューを実施する：
   - **`none` の場合（外部レビュー無効）**：
     - 外部レビューア呼出コマンド（`codex exec`, `poverello/review`, DeepSeek Harness 等）を一切生成・実行しない（呼出数厳密に 0）。
     - 独立レビューループを開始しない。
     - 実装・検証テスト完了後、外部実装レビューを行わずに `FULL CYCLE COMPLETE — EXTERNAL REVIEW DISABLED` として停止する。
     - **No-Fallback 原則:** レビューア利用不可・エラー等のいかなる例外状況下でも、Codex や DeepSeek へのフォールバック呼出を絶対に実行しない。
   - **Antigravity のみ選択の場合（自己レビュー）**：
     - 外部CLI（Codex CLI、DeepSeek Harness）呼出コマンドを一切生成・実行しない（呼出数厳密に 0）。
     - Antigravity自身が実装・差分・テスト証拠を検査し、自己レビュー所見と合否判定を正本計画書へ記録する。
   - **Codex CLI 単独選択（`codex_only`）の場合**：
     - 固定した資料を外部 Codex CLI のみへ渡す。DeepSeek Harness は一切呼び出さない（呼出数厳密に 0）。
     - 各findingにつき、対象要件、具体的な失敗条件、severity/blocking、必要な最小修正、合否判定可能なtestまたは証拠を報告させる。
     - Synthesis は行わず、Codex 単一のレビュー結果に基づき判定を採用する。
   - **DeepSeek Harness 単独選択（`deepseek_only`）の場合**：
     - 固定した資料を外部 DeepSeek Harness (`dsh headless`) のみへ渡す。Codex CLI は呼び出さない（呼出数厳密に 0）。
     - 各findingにつき、対象要件、具体的な失敗条件、severity/blocking、必要な最小修正、合否判定可能なtestまたは証拠を報告させる。
     - Synthesis は行わず、DeepSeek 単一のレビュー結果に基づき判定を採用する。
     - DeepSeek が利用不可またはエラーの場合、Codex CLI への暗黙フォールバックは絶対に行わず `INCOMPLETE` / `UNAVAILABLE` として停止する。
     - `REQUEST_CHANGES` の場合、指定された是正担当（GUI設定または計画書記載）が是正実装を行う（DeepSeek Reviewer自身は編集しない）。
   - **Antigravity ＋ Codex CLI 選択の場合**：
     - DeepSeek Harness は一切呼び出さない（呼出数厳密に 0）。
     - Antigravity（自己レビュー）および Codex CLI（外部独立レビュー）の両者がレビューを実施し、所見を正本計画書へ記録する。
   - **Antigravity ＋ DeepSeek Harness 選択の場合**：
     - Codex CLI は一切呼び出さない（呼出数厳密に 0）。
     - Antigravity（自己レビュー）および DeepSeek Harness（外部独立レビュー）の両者がレビューを実施し、所見を正本計画書へ記録する。
   - **Codex CLI ＋ DeepSeek Harness 選択（`codex_plus_deepseek`）の場合**：
     - 同一版を Codex CLI と DeepSeek Harness CLI へ別々に渡す。
     - 各Reviewerは、findingごとに次を示す：対象要件、具体的な失敗条件、severity/blocking、必要な最小修正、合否判定可能なtestまたは証拠。
     - Reviewer自身がテストを再実行した場合だけ、そのコマンド・終了コード・結果をReviewer実行証拠として記録する。実装者ログを読んだだけならその旨を明記する。
     - Antigravityはblocking findingが「共通契約」の4条件を満たすことを確認し、同一原因のfindingを統合する。既存findingの解決条件を満たした後、任意改善だけを理由に新たなblocking findingとしてレビューを長期化させない。
     - **判定統合と裁定（Arbitration）契約:**
       - 両Reviewer（Codex CLI と DeepSeek Harness）に同一版を渡し、両方の結果を待つ（実行順序の変更や一方の失敗による早期停止は行わない）。
       - いずれかのReviewerが `INCOMPLETE`、`UNAVAILABLE`、`ERROR`、タイムアウト、未実行、打ち切りの場合、全体を `INCOMPLETE` とする（優先設定で上書きしない。DeepSeek Harness のWindows読み取り専用条件が未達の間は `UNAVAILABLE` となり、Codex単独への暗黙フォールバックは禁止）。
       - 両Reviewerが有効判定（`APPROVED` または `REQUEST_CHANGES`）を返した場合：
         - 判定が一致していれば、その共通判定を採用する。
         - 判定が食い違った場合、保存された `priority_reviewer`（`codex_cli` または `deepseek`）の判定を採用する。
         - **Blocking Finding 例外規定:** いずれかのReviewerが未解決の Blocking Finding、安全性違反、または必須テスト失敗を指摘している場合、優先Reviewerが `APPROVED` であっても全体を `APPROVED` にしてはならず、`REQUEST_CHANGES`（または実行・環境問題なら `INCOMPLETE`）とする。
       - 優先されなかった側の指摘、根拠、severity もすべて最終報告と正本計画書に記録し、破棄しない。
   - **3者すべて（Antigravity ＋ Codex CLI ＋ DeepSeek Harness）選択の場合**：
     - 3者それぞれのレビュー結果を収集する。Antigravityの自己レビュー所見を記録するとともに、Codex CLI と DeepSeek Harness の外部独立レビュー結果に対して上記裁定（Arbitration）契約を適用する。
4. Synthesis（または `codex_only` 時のCodex判定）は`APPROVE`、`APPROVE_WITH_NON_BLOCKING_FINDINGS`、`APPLY_AND_FINALIZE`、`REQUEST_CHANGES`、`BLOCKED`のいずれかを返す。
5. `APPLY_AND_FINALIZE`は最終承認状態ではなく、限定的・機械的な修正指示である。Synthesisは変更箇所、変更内容、指定テスト、差分確認、修正後に到達すべき最終承認状態を明示する。
6. `APPLY_AND_FINALIZE`の場合、Antigravityが指定修正だけを適用し、指定テスト・差分確認を行う。完全一致した場合は再レビューなしでSynthesis指定の最終承認状態を記録できる。一致しない、追加設計判断が必要、または新たなblocking findingが生じた場合は`REQUEST_CHANGES`へ戻す。
7. `REQUEST_CHANGES`の場合、Antigravityが採用済みfindingだけから最小の是正計画を正本計画書へ作る。
8. 是正計画は承認済み範囲を越えてはならない。範囲、公開API、権限、費用、依存関係、外部接続、目的の変更が必要なら停止してユーザーへ戻す。
9. Antigravityが確定した是正だけを実装し、関連テストと必要な回帰テストを同じ最終treeで再実行する。
10. 修正後は、未解決blocking findingが0件であることを確認する。非blocking findingは残してよいが、残存リスクとして記録する。
11. 通常の`REQUEST_CHANGES`是正後は有効な policy に従って独立実装再レビューへ戻る。P2だけでは再レビューしない。
12. グローバルルールの上限内でのみ反復し、上限到達時は停止して残るblocking finding、実施済み修正、テスト結果、未検証事項を報告する。

## 5. 完了判定

1. `APPROVED`：blocking finding 0件、承認済み実装範囲と必須自動検証が完了している。
2. `APPROVED_WITH_NON_BLOCKING_FINDINGS`：blocking finding 0件だが、将来改善や残存リスクがある。非blocking findingを明示する。
3. `REQUEST_CHANGES`：blocking findingが残り、承認済み範囲内で是正可能である。
4. `BLOCKED`：必要な是正が範囲外、検証不能、権限不足、結果不明、または上限到達である。
5. 実機・ステージング・本番受入が未実施なら、実装レビュー承認後も`STAGING_ACCEPTANCE_PENDING`等の未完了ゲートを残す。「本改修完了」「release ready」等、未実施ゲートを含む表現を使わない。
6. コード実装レビューの承認と、実機・ステージング・本番受入の承認を混同しない。
7. 最終結果、証拠の出所、未検証事項、残存リスク、次に必要な明示的許可を正本計画書へ記録する。
8. commit、push、tag、release、deploy、本番変更は行わず停止する。ただしユーザーが別途明示的に許可した場合は、その別操作として扱う。
