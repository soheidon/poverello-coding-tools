---
name: pov-plan-only
description: 計画作成と外部Codex/DeepSeek独立計画レビュー専用のSkill。承認（Approved）判定後も実装へ進まず計画記録のみで停止する。
---

# POV Plan Only

## 役割と権能分離 (Separation of Powers)

- **正本計画書（Active Plan File）**: 作業開始前に必ず確認する。GUIでアクティブプランファイルが指定されていない場合、即座に `BLOCKED — NO ACTIVE PLAN FILE SPECIFIED: Poverello GUI の Autocode 画面から作業対象のプランファイルを指定または新規作成してください。` を出力して停止する。計画書ファイルを読み込み、そのSHA-256ダイジェストを記録する。以降のすべてのフェーズ（計画作成、レビュー判定、是正）の更新は、直前フェーズのSHA-256ダイジェストを `expected_sha256` として指定した `append_plan_audit_trail` による CAS（Compare-And-Swap）更新を用いて、正本計画書の `## 5. Execution History & Audit Trail` へ順次追記する。
- 計画candidateは一時領域で作成し、`apply_plan_candidate` に工程開始時の `expected_file_identity` とSHA-256を渡して反映する。監査追記eventとチェックポイント保存の両方にidentityとSHA-256を指定し（いずれも必須）、各信頼済み書込み後に返されたidentity/Hashを次工程へ引き継ぐ。再開時は `get_plan_checkpoint` と `verify_plan_resumption` を実行し、repo、path、revision、file identity、Findingの全検査が成功しなければ停止する。
- **Planner (計画作成担当)**:
  - Plannerは `.plan/.active-plan.json` の `role_assignments.antigravity.planner_id` を用いる。未設定・不正なら停止し、global設定や別roleへのfallbackを禁止する。
  - `planner_role` が `codex`（推奨）の場合: Antigravity によるハルシネーション・代筆を完全に禁止。Antigravity は自ら計画を書かず、`codex exec` を起動して計画書を作成させる。
  - `planner_role` が `antigravity` の場合: Antigravity が計画書を作成する。
- **Reviewer (検査担当)**: 外部 Reviewer（Codex CLI / DeepSeek Harness）は完全な読み取り専用（Read-Only）として検査し、合否判定（`APPROVED` / `REQUEST_CHANGES`）と指摘のみを返す。**Reviewer 自身が正本計画書を直接編集・修正・承認改ざんすることは厳格に禁止される**。
- **Reviser (計画修正担当)**: 指摘に対する修正は、指定された Reviser（`reviser_role` が `codex` の場合は Codex CLI）が行い、修正完了後に再度 Reviewer の検査を仰ぐ。
  - Reviserは `.plan/.active-plan.json` の `role_assignments.antigravity.reviser_id` を用いる。未設定・不正なら停止し、別roleへのfallbackを禁止する。
- **Role assignmentの実行**: 設定値 `antigravity` は現在のAntigravityセッション、`codex` / `codex_cli` は `codex exec` によるCodex CLI実行を意味する。PlannerまたはReviser工程の前に保存値とActive Plan Fileを読み直し、未設定・不正ID・実行失敗時は担当名と理由を報告して停止する。Reviewerが終了した後にのみReviserの別呼び出しを行い、同じIDでもReviewerの指摘をReviserが独立に判断してから変更する。
- 各工程境界では監査イベントをfile identity/HashでCAS追記し、その成功応答を使ってチェックポイントを保存する。チェックポイント保存後のidentity/Hashを次工程の期待状態にする。
- 外部Reviewer構成は、共通設定ファイル `%USERPROFILE%\.gemini\antigravity-ide\pov-reviewer-policy.json`（他OSでは対応するホームディレクトリ配下）の設定に従う。
- コード、テスト、設定は変更しない。
- `pov-mcp-plan`、`pov-mcp-review`、`pov-mcp-revise`はPoverello MCP用の別Skillであり、このSkillから呼ばない。

## 共通契約

- **正本計画書への全ライフサイクル記録 (Audit Trail)**: 計画、自己レビュー結果、外部レビュー結果、是正履歴、最終判定を正本計画書末尾の「Execution History & Audit Trail」セクションへ時系列で漏れなく記録する。
- 外部CLIは各グローバルルールに従って実行する。内部エージェントで代替しない。
- 同一案件では正本計画書を1件だけ使う。別版、レビュー依頼書、修正計画書を作らない。
- 計画、レビュー結果、修正内容、最終判定を正本計画書へ簡潔に記録する。
- レビュー実行前に実効ポリシー（評価優先順位: Evaluation Precedence）を決定する:
  1. **実行時明示指定:** コマンドライン引数（`--review none` 等）またはユーザープロンプト内の明示指示（「Codex CLI や DeepSeek Harness によるレビューは不要」「外部レビュー不要」等）が存在する場合は、最優先でその値を実効ポリシーとする。
  2. **GUI設定ファイル:** 実行時指定がない場合、共通設定ファイル `%USERPROFILE%\.gemini\antigravity-ide\pov-reviewer-policy.json`（`schema_version: 6`）またはリポジトリ内 `.plan/.active-plan.json` を読み取る。計画レビュー設定（`plan_review_policy`）が個別に指定されている場合はその設定を優先する。未指定時または実装レビュー設定継承の場合は、実装レビュー担当（`build_reviewers` または `build_review_policy` / `policy`）の複数選択設定をそのまま計画レビューにも適用する。`"none"` の場合は外部レビュー無効とする（複数Reviewer選択時は有効な `priority_reviewer` ("codex_cli" または "deepseek") が必須。未選択または旧schemaの場合はマイグレーション未完了として dispatch を拒否し、`none` 相当で停止する）。Reviewer実行時のモデルおよび推論強度は同設定内の `codex`（`model`, `reasoning_effort`）および `deepseek`（`model`, `reasoning_effort`）に従う。
  3. **安全側既定値 (Fail-Safe):** ファイル不在、構文エラー、未知の policy 値、未対応 schema_version、未選択 priority、書込障害等の例外時は、安全側既定値 `none` (`Disabled`) を適用する。
- `codex_only` では Codex CLI のみを使用し、DeepSeek Harness は起動・待機・比較・Synthesis のいずれにも使用しない。DeepSeek Rule の有無も要求しない。Codex CLI 失敗時に他Reviewerへ暗黙フォールバックしない。
- `deepseek_only` では DeepSeek Harness (`dsh headless`) のみを使用し、Codex CLI は起動・待機・比較・Synthesis のいずれにも使用しない。Codex CLI への暗黙フォールバックは絶対に行わない。DeepSeek 実行時のモデルおよび推論強度は同設定内の `deepseek`（`model`, `reasoning_effort`）に従う。
- `codex_plus_deepseek` では Codex と DeepSeek に同一ハッシュの資料を渡し、互いの回答を見せない。
- 計画書の修正は Reviewer に行わせず、指定された Reviser が行う。
- P2だけで修正・再レビューしない。回数、時間、再試行、停止条件はグローバルルールを適用する。
- Approvedは実装、commit、push、tag、release、deploy、本番変更の許可ではない。

### Blocking findingの条件

- P0/P1のblocking findingは、次の4点をすべて示せる場合に限る。
  1. 承認済み要件または今回の依頼範囲のどの契約に反するか。
  2. 具体的にどの入力・状態・処理順で失敗するか。
  3. 承認に必要な最小修正は何か。
  4. 修正済みかを判定できる具体的なtestまたは静的検査は何か。
- 将来強化、表記の好み、一般論、未採用の脅威モデル、今回の目的に不要な設計改善はblockingにしない。必要ならP2または非blocking recommendationとして記録する。
- 新たに確認された重大な不具合はblockingにできるが、上記4点を同様に満たすこと。
- 同じ原因を言い換えただけのfindingは新規findingとして数えず、既存findingへ統合する。
- 修正版で既存findingの解決条件を満たした後、別の任意改善だけを理由にレビューを長期化させない。

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

## 1. 計画作成

1. 指示書、リポジトリ、ローカルルール、既存計画、関連コードを確認する。
2. 同じ目的の正本計画書があれば再利用し、なければ1件だけ作る。
3. 目的、確認事実と仮説、範囲、変更対象、不変条件、実装方法、テスト、停止点を記載する。
4. Antigravityが内部レビューし、事実誤認、矛盾、不足を計画書内で修正する。

## 2. 独立計画レビュー

1. 現在の計画書と必要なコードを固定する。
2. 共通設定の `plan_reviewers`（未指定時または実装レビュー継承時は `build_reviewers` または `plan_review_policy` / `policy`）に従ってレビューを実施する：
   - **`none` の場合（外部レビュー無効）**：
     - 外部レビューア呼出コマンド（`codex exec`, `poverello/review` 等）を一切生成・実行しない（呼出数厳密に 0）。
     - 独立レビューループを開始しない。
     - 計画書のドラフト作成および要件反映の完了後、計画書ヘッダーおよびレビュー履歴を `PLAN COMPLETE — EXTERNAL REVIEW DISABLED (Awaiting user approval)` として記録する。
     - 計画概要をチャットに提示して直ちに実行を停止し、ユーザーの明示的な計画承認を待つ。
     - **No-Fallback 原則:** レビューア利用不可・エラー等のいかなる例外状況下でも、Codex や DeepSeek へのフォールバック呼出を絶対に実行しない。
   - **Antigravity のみ選択の場合（自己レビュー）**：
     - 外部CLI（Codex CLI、DeepSeek Harness）呼出コマンドを一切生成・実行しない（呼出数厳密に 0）。
     - Antigravity自身が計画書を検査し、自己レビュー所見と合否判定を正本計画書へ記録する。
   - **Codex CLI 単独選択（`codex_only`）の場合**：
     - 固定した計画書とコードを外部 Codex CLI のみへ渡す。DeepSeek Harness は一切呼び出さない（呼出数厳密に 0）。
     - Codex が計画レビューを行い、指摘に基づく修正を正本計画書へ反映する。
     - Synthesis は行わず、Codex 単一のレビュー結果に基づき合否を判断する。
   - **DeepSeek Harness 単独選択（`deepseek_only`）の場合**：
     - 固定した計画書とコードを外部 DeepSeek Harness (`dsh headless`) のみへ渡す。Codex CLI は呼び出さない（呼出数厳密に 0）。
     - DeepSeek Harness が計画レビューを行い、合否判定および指摘を返す。
     - Synthesis は行わず、DeepSeek 単一のレビュー結果に基づき合否を判断する。
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
     - 両レビュー完了後、Codex と DeepSeek が自分の指摘に必要な修正を正本計画書へ反映する。
     - Antigravityが修正後の計画を確認し、重複、矛盾、範囲逸脱を整理する。blocking findingは「Blocking findingの条件」を満たすものだけ残す。
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
3. 統合判断（または `codex_only` 時の単独判定）は次のいずれかを明示する。
   - `APPROVE`: 現行計画をそのまま承認する。
   - `APPLY_AND_FINALIZE`: 承認に必要な修正が、具体的・限定的・機械的に適用可能で、新たな設計判断を必要としない場合に限る。統合判断は変更箇所、置換内容または変更条件、確認方法を明示する。
   - `REQUEST_CHANGES`: 設計判断を伴う修正、複数の妥当な選択肢がある修正、または修正後の妥当性を外部独立レビューで再確認すべき場合。
   - `BLOCKED`: 必要資料不足、権限不足、検証不能、またはグローバル停止条件に達した場合。
4. `APPLY_AND_FINALIZE`の場合、Antigravityは指定された修正だけを正本計画書へ適用し、指定された静的検査・自己整合確認を行う。指定外の設計変更を加えない。確認に合格すれば、外部再レビューへ戻らずApprovedを記録する。確認に失敗した場合はApprovedにせず`REQUEST_CHANGES`として扱う。
5. `REQUEST_CHANGES`の場合、修正版を作成した後、有効な policy に従って再度レビューさせる。ただし、P2だけでは再レビューせず、グローバルの回数・時間・同一finding上限を超えてはならない。
6. `APPROVE`または正常に完了した`APPLY_AND_FINALIZE`を最終判断とする。最終判断後は計画レビューへ戻らない。

## 3. 完了

- 外部レビュー無効時 (`none`): 計画書の要件反映完了後、`PLAN COMPLETE — EXTERNAL REVIEW DISABLED (Awaiting user approval)` を記録し、ユーザー確認待ちとして安全に停止する。ユーザー承認を受領した時点で `APPROVED BY USER` となる。
- 外部レビュー有効時 (`codex_only` / `deepseek_only` / `codex_plus_deepseek`): Approvedなら計画承認を記録し、実装せず停止する。Not Approvedなら未解決P0・P1と停止理由を記録し、ユーザーへ報告して停止する。
- commit、release、deployを行わない。
