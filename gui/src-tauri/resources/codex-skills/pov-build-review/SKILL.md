---
name: pov-build-review
description: 承認済み計画に基づく実装・テスト、独立実装レビュー、および是正実装を管理するCodex Skill。
---

# POV Build Review

## 役割

- Codex：承認済み計画に基づく実装、テスト実行、自己レビュー、独立実装レビュー、および必要な是正実装を行う。
- レビュー完了後、commit、push、tag、release等は行わずに停止する。

## 共通契約

- 実装前に承認済み正本計画書を確認し、その範囲内のみを変更する。
- テスト未実行・実行不能・結果不明の項目をPASSとみなさない。
- 実装レビューで指摘されたblocking findingに対して最小限の是正を行い、回帰テストを実施する。
- レビュー承認はcommit、push、tag、release、deployの許可ではない。

## 外部並列レビュー契約

1. **レビュー設定の解決**:
   - レビュー実行前に `CODEX_HOME/pov-review-settings.json` を読み込む。ファイル不在時は Codex 単独（priority: codex, gemini_cli_enabled: false）を既定値とし、設定破損・未知schema時は fail-closed で停止する。
   - schema version 1 を読み込んだ場合は、旧 `reviewers.gemini == true` であれば `gemini_cli_enabled = true` として移行処理し、旧設定の選択状態を保持する。schema version 2 では `gemini_cli_enabled` フィールドを必須として検証する。
   - `gemini_cli_enabled == false` の場合は、設定ファイル内に `reviewers.gemini: true` が残存していても実効Reviewer候補（effective reviewer set）から Gemini を除外し、ディスパッチャーへ Gemini を渡さない。
   - 実効 Reviewer（`codex`, `deepseek`, オプション有効時かつ選択時のみ `gemini`）を確認する。0件選択時は外部 Reviewer を起動せず、結果を `REVIEW_SKIPPED` として実装完了ゲートで停止する（APPROVEDと主張しない）。

2. **送信前同意（Pre-send Consent）**:
   - レビュー対象ファイル一覧、変更差分、ファイル数、合計サイズ、SHA-256ダイジェスト、送信先CLI、read-only制約を提示し、送信前の明示的同意を得る。
   - `.git`、認証・秘密情報、`.env`、巨大生成物、symlink/reparse はスナップショットから除外する。
   - 同意が得られない場合は Reviewer を起動せず `REVIEW_SKIPPED` として停止する。

3. **安全なディスパッチ**:
   - 同梱の `scripts/codex-review-dispatcher.ps1` を通じて固定CLI・固定引数・read-onlyスナップショットで最大3件並列実行する。
   - スナップショットおよびその全配下ファイルの symlink/reparse point を再帰検証し、外部参照や脱出経路が存在する場合は即座に実行拒否する。
   - Windows環境ではプロセスを `CREATE_SUSPENDED` で生成し、親パイプハンドルの継承制御を行ったうえで Windows Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) に割り当てた後に `ResumeThread` で実行する。タイムアウト・キャンセル・異常終了時は `TerminateJobObject` により子孫プロセスを含めて確実に強制終了し、回収を確認する（終了未確認時は自動再試行せず結果不明として停止）。
   - Gemini CLI は、実行環境において書き込み制限およびPlan Mode逸脱（YOLO遷移）防止を担保する公式sandboxが実効的に検証されない限り起動せず、即座に `UNAVAILABLE` として報告する。
   - DeepSeek Harness は公式headless仕様に従い、`--permission-mode read-only --approval-policy never` を指定し、タスク指示は `-p` ではなく位置引数で渡す。
   - stdoutは各1 MiBでキャップし、stderr本文（推論・トークン等）は保存・返却しない。

4. **結果の統合と判定**:
   - 選択されたReviewerが1件でも失敗・timeout・unavailable・truncated（1 MiB超過）の場合、バッチ全体を `INCOMPLETE` とし、APPROVEDにしない。
   - いずれかのReviewerに未解決の具体的Blocking Findingがある場合、全体判定を `REQUEST_CHANGES` または `CONFLICT_UNRESOLVED` とし、priority reviewer がAPPROVEしていても承認としない。
   - 全選択Reviewerが正常完了し未解決Blocking Findingがない場合のみ、priority reviewerの判定を最終判定として記録する。CodexがReviewerに含まれない場合、driverの自己評価を独立レビュー結果に数えない。

## 進行手順

1. **実装とテスト**: 承認済み計画に従って実装し、テストを実行・記録する。
2. **実装レビュー**:
   - レビュー設定を読み取り、実装差分スナップショットを合成して送信前同意を得る。
   - ディスパッチャー経由で選択されたReviewerを並列実行し、判定基準に従って結果を統合する。
3. **是正**: 必要に応じて是正計画を立て、最小限の修正と再テスト・再レビューを行う。
4. **停止**: 完了判定を記録して停止する。
