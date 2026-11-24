---
name: pov-plan-only
description: 計画作成と独立計画レビュー専用のCodex Skill。承認判定後も実装へ進まず計画記録のみで停止する。
---

# POV Plan Only

## 役割と権能分離 (Separation of Powers)

- **正本計画書（Active Plan File）の解決順序**: 作業開始前に必ず確認する。
  1. **ファイルベース解決（明示指定またはリポジトリ内ミラー＋Stable正本レジストリ照合）**:
     本Skill同梱の `scripts/plan_cas_helper.py` を用いて対象プランを解決する（Tauri IPC や MCP は必須としない）。
     - ユーザーが明示的なプランパスを指定した場合は `python "<skill_dir>/scripts/plan_cas_helper.py" resolve --surface codex --plan "<explicit_path>"` を実行する。
     - 明示指定がない場合は `python "<skill_dir>/scripts/plan_cas_helper.py" resolve --surface codex` を実行し、カレントGitリポジトリの `.plan/.active-plan.json`（`surfaces.codex.active_plan_path`）を候補として取得したうえで、必ず Poverello 正本レジストリ（`%APPDATA%\Poverello\project-registry.json`、または隔離検証時の `POVERELLO_REGISTRY_PATH`）と照合する。
     - `plan_cas_helper.py` は、(a) 明示パスとレジストリ登録値の一致、(b) `.active-plan.json` ミラーとレジストリ登録値の一致、(c) `surfaces.codex` が明示解除（`null`）されていないこと、(d) リポジトリ境界・`.md`拡張子・symlink/reparse point 不在、(e) アプリと同じ規則で解決したCodex homeの `pov-review-settings.json` にある `workflow_role_assignments.codex` のPlanner/Reviser割当（`codex_session` / `codex`）を検証する。`CODEX_HOME` が空ならWindowsは `%USERPROFILE%\.codex`、それ以外は `$HOME/.codex` を使う。ミラー内の旧role fieldsは実行時の正本として使わない。
  2. **未解決時の安全な停止 (Fail Closed)**:
     レジストリが読めない、リポジトリ未登録、明示パスやミラーがレジストリと不一致、複数候補、symlink/reparse point によるリポジトリ外参照、または解除済み surface の場合は、別プロジェクトや旧slotへフォールバックせず、`BLOCKED` メッセージを出力して即座に停止する。
- **Planner (計画作成担当)**: 指定された正本計画書（`.plan/<name>.md` 等）に初期計画を作成する。
- **Reviewer (検査担当)**: 計画書を完全な読み取り専用（Read-Only）として検査し、合否判定（`APPROVED` / `REQUEST_CHANGES`）と具体的根拠付き指摘（P0/P1/P2）のみを返す。**Reviewer 自身が正本計画書を直接編集・修正・承認改ざんすることは厳格に禁止される**。
- **Reviser (計画修正担当)**: Reviewer の指摘を受けて計画書を修正する担当。修正は必ず Reviser（Codex作業セッション等）が行い、修正完了後に正本計画書を固定して再度 Reviewer の検査を仰ぐ。
- **Role assignmentの実行**: `plan_cas_helper.py resolve` が確認する担当割当および解決済みCodex homeの `pov-review-settings.json` に従い、`codex_session`（または後方互換の `codex`）に割り当てられた工程のみを担当する。Codex homeを決定できない、設定が欠落・不正・競合中の場合は、repo mirrorの旧role値へfallbackせず停止する。Reviewer設定は既存のglobal reviewer設定から独立して解決する。
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
  正本計画書の `## 5. Execution History & Audit Trail`（または該当監査セクション）直前に、専用のチェックポイント領域 `## Poverello Execution Checkpoint` を設けて記録する。人間が記述した計画本文や既存の監査ログを一切削除・要約・破壊しない。
- **利用可否診断・新規開始・再開の厳密な区別**:
  1. **利用可否の診断 (`resolve`)**: `python "<skill_dir>/scripts/plan_cas_helper.py" resolve --surface codex [--plan "<explicit_path>"]` を実行し、登録一致・役割割当・現在の `checkpoint_state`（`NONE`, `DIFFERENT_WORKFLOW`, `MISSING_PLAN_REVISION`, `MISSING_BINDING`, `TARGET_MISMATCH`, `RESUMABLE`, `REVISION_MISMATCH`, `HALTED`, `MALFORMED_OR_DUPLICATE`）を確認する。新規開始と再開を曖昧に推定してはならない。
  2. **新しい `pov-plan-only` ワークフローの開始 (`start`)**:
     - `checkpoint_state == "NONE"` の場合は、`python "<skill_dir>/scripts/plan_cas_helper.py" start --surface codex --expected-sha256 "<sha256>" --actor codex_session --notes "Start pov-plan-only" --workflow-run-id "<run_id>" --operation-id "<op_id>"` で基準チェックポイントを作成する。
     - `checkpoint_state` が `DIFFERENT_WORKFLOW`（例: `pov-build-review`）または `MISSING_PLAN_REVISION`（旧形式）の場合は、旧checkpointのRepository/Active Plan/Workflow Run IDを新規対象の証明に使わず、**`--archive-prior`** を付けて `start` を実行する。欠落・不一致のbinding情報は監査行に記録し、旧checkpoint原文はフェンス付きアーカイブ（`### Archived Prior Checkpoint`）へ退避する。新しいcheckpointは現在解決されたregistry planと新run IDにbindingし、過去の整合性を検証済みとは扱わない。
     - `MALFORMED_OR_DUPLICATE`・`HALTED`・`REVISION_MISMATCH`・`RESUMABLE` に対して `start` を実行してはならない（自動修復せず停止する）。
  3. **既存の `pov-plan-only` ワークフローの再開 (`verify`)**:
     `python "<skill_dir>/scripts/plan_cas_helper.py" verify --surface codex` を実行し、`RESUMABLE`（`Workflow == pov-plan-only`・記録された `Repository` と `Active Plan` が現在の正本に一致・`Workflow Run ID` が存在・`Plan Revision` 一致・`Status != HALTED`）のみを許可する。commitでは `--workflow-run-id` に同じrun IDを渡す。欠落・別workflow・対象binding不一致・revision不一致・HALTED・不正/重複マーカー時は自動修復せず停止する。
  4. **各工程の確定 (`commit`)**:
     `start` または `verify` の完了後、各工程境界で `python "<skill_dir>/scripts/plan_cas_helper.py" commit --surface codex --expected-sha256 "<pre_sha256>" --phase "<phase>" --actor "<actor>" --result "<result>" --notes "<notes>" --status "<status>" --last-completed "<last_completed>" --next-action "<next_action>" --workflow-run-id "<run_id>" --operation-id "<op_id>" [--body-file "<tmp_body>"]` を実行し、共有 `plan-locks` のプロセス間ロックを保持したまま正本再解決・CAS（`expected_sha256`）検証・監査行追記・チェックポイント除外リビジョン計算・置換直前パス再検証・同一ディレクトリ一時ファイルからの原子的置換（`os.replace`）を一括実行する。返却された最新の `sha256_hash` を次工程の `--expected-sha256` へ引き継ぐ。
- **ファイルベースCASの限界（非MCP境界）**: 本スクリプトはPoverello GUIと同じStable `plan-locks` を使うが、外部実行ジャーナル（`plan-run-journals`）には接続しない。共有ロックを使わない外部writerは直列化されないため、最終CAS検査と置換の間に競合する可能性があり、そのwriterとの同時書込み安全性は保証しない。
- **保存タイミング**:
  1. ワークフロー新規開始時（`start`）
  2. 計画作成完了時（`commit`）
  3. 計画レビュー判定確定時（`commit`）
  4. 計画修正完了時（`commit`）
  5. 完了（COMPLETED）または停止時（`commit`）
- **記録内容**:
  `Checkpoint Version: 1`、`Workflow: pov-plan-only`、`Status`、`Last Completed`、`Next Action`、`Repository`、`Active Plan`、`Plan Revision`、`Workflow Run ID`、`Operation ID`、`Timestamp`。
- **作業再開手順 (Resumption)**:
  計画作成・レビュー再開時、`python "<skill_dir>/scripts/plan_cas_helper.py" verify --surface codex` を実行して確認する：
  1. レジストリ＋ミラー一致・リポジトリ同一性・リビジョン整合性（`Plan Revision` と現在本文の一致）・`Status != HALTED` を確認する。
  2. 直前完了工程（`Last Completed`）に基づき、確定済みの計画を活用して `Next Action` から安全に再開する。
  3. 不一致や外部変更検出時は推測でやり直さず安全に停止する。
- **コンテキスト読込削減 (Context Optimization)**:
  1. 過去の会話履歴全文や長大な監査ログを毎回再読込しない。
  2. Active Plan Fileのチェックポイントおよび現在の工程に必要なセクションのみを取得して作業を進める。

## レビュー方式と並列レビュー契約

1. **レビュー方式（review_mode）の解決**:
   - 作業開始前に `plan_cas_helper.py resolve --surface codex [--plan <explicit_path>]` を実行する。選択結果はStable registryとrepo mirrorの一致を検証し、Codexから利用できないTauri commandやMCP toolを呼び出し要件にしない。
   - 再開時は `plan_cas_helper.py verify --surface codex` を実行し、registry/mirror、Plan Revision、HALTED状態を検証する。失敗時は別planや旧slotへfallbackせず停止する。
   - 計画candidateを適用する場合は、`plan_cas_helper.py commit --body-file <candidate>` を用いる。helperは既存の監査履歴とcheckpointを正本から保持し、共有ロック内でCAS更新する。監査またはcheckpointの保持・更新に失敗した場合は完了扱いにしない。
   - 各工程境界の更新はhelperが返す最新 `sha256_hash` を次回の `--expected-sha256` に渡す。書込み応答が不明な場合は `resolve` と `verify` で現在状態を読み直し、状態が判定できなければ停止する。
   - 選択planを明示パスから解決できず、registryとmirrorの同一repo/pathを確認できない場合は `BLOCKED` として停止する。
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
