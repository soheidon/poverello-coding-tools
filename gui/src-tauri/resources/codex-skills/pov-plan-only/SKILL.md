---
name: pov-plan-only
description: 計画作成と独立計画レビュー専用のCodex Skill。承認判定後も実装へ進まず計画記録のみで停止する。
---

# POV Plan Only

## 役割

- Codex：作業主体として計画作成、内部自己レビュー、および独立計画レビューの進行・管理を行う。自己レビューは本セッション内で直接実施し、追加の `codex exec` による自己レビューは起動しない。
- 外部独立レビューが必要な場合は、外部Reviewer（DeepSeek Harness、オプション有効時かつ選択時のみ Gemini CLI）が担当する。
- 承認判定（`APPROVED` / `APPROVED_WITH_NON_BLOCKING_FINDINGS`）が得られた場合でも、実装へ進まず計画記録のみで停止する。
- コード、テスト、設定は変更しない。

## 共通契約

- 同一案件では正本計画書を1件だけ作成・更新し、別版や依頼書を作らない。
- 計画、自己レビュー結果、外部レビュー結果、修正内容、最終判定を正本計画書へ簡潔に記録する。
- Blocking finding（P0/P1）は、(1) 反する要件・契約、(2) 具体的失敗条件、(3) 必要な最小修正、(4) 判定可能なテスト/証拠の4点を満たすものに限る。
- Approved判定は実装、commit、push、tag、release、deployの許可ではない。

## レビュー方式と並列レビュー契約

1. **レビュー方式（review_mode）の解決**:
   - レビュー実行前に `CODEX_HOME/pov-review-settings.json`（`schema_version: 4`）を読み込む。ファイル不在時は既定値（`review_mode: codex_self_only`, priority: codex, gemini_cli_enabled: false）とし、設定破損・未知schema時は fail-closed で停止する。
   - `review_mode` の4区分に従ってレビュー工程を実行する：
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
