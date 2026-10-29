---
name: pov-plan-only
description: 計画作成と外部Codex/DeepSeek独立計画レビュー専用のSkill。承認（Approved）判定後も実装へ進まず計画記録のみで停止する。
---

# POV Plan Only

## 役割

- Antigravity：進行管理、計画作成、内部レビュー、最終整理を行う。
- Codex：外部`codex exec`（`gpt-6-luna`、`medium`）で計画をレビューし、計画書を修正する。
- DeepSeek：外部`dsh headless`で計画をレビューし、計画書を修正する。
- 外部Reviewer構成は、共通設定ファイル `%USERPROFILE%\.gemini\antigravity-ide\pov-reviewer-policy.json`（他OSでは対応するホームディレクトリ配下）の設定に従う。
- コード、テスト、設定は変更しない。
- `pov-mcp-plan`、`pov-mcp-review`、`pov-mcp-revise`はPoverello MCP用の別Skillであり、このSkillから呼ばない。

## 共通契約

- 外部CLIは各グローバルルールに従って実行する。内部エージェントで代替しない。
- 同一案件では正本計画書を1件だけ使う。別版、レビュー依頼書、修正計画書を作らない。
- 計画、レビュー結果、修正内容、最終判定を正本計画書へ簡潔に記録する。
- レビュー実行前に共通設定ファイル `%USERPROFILE%\.gemini\antigravity-ide\pov-reviewer-policy.json` を読み取る。ファイル不在時は互換既定値 `{"schema_version":1,"policy":"codex_plus_deepseek"}` を適用する。JSON不正、未知version、未知policy値、または読み取り失敗時はReviewerを勝手に選ばず明確なエラーで停止する。
- `codex_only` では Codex CLI のみを使用し、DeepSeek Harness は起動・待機・比較・Synthesis のいずれにも使用しない。DeepSeek Rule の有無も要求しない。Codex CLI 失敗時に他Reviewerへ暗黙フォールバックしない。
- `codex_plus_deepseek` では Codex と DeepSeek に同一ハッシュの資料を渡し、互いの回答を見せない。
- 計画書の修正は競合を避けて順番に行う。
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

## 1. 計画作成

1. 指示書、リポジトリ、ローカルルール、既存計画、関連コードを確認する。
2. 同じ目的の正本計画書があれば再利用し、なければ1件だけ作る。
3. 目的、確認事実と仮説、範囲、変更対象、不変条件、実装方法、テスト、停止点を記載する。
4. Antigravityが内部レビューし、事実誤認、矛盾、不足を計画書内で修正する。

## 2. 独立計画レビュー

1. 現在の計画書と必要なコードを固定する。
2. 共通設定の `policy` に従ってレビューを実施する：
   - **`codex_only` の場合**：
     - 固定した計画書とコードを外部 Codex CLI のみへ渡す。
     - Codex が計画レビューを行い、指摘に基づく修正を正本計画書へ反映する。
     - Synthesis は行わず、Codex 単一のレビュー結果に基づき合否を判断する。
   - **`codex_plus_deepseek` の場合**：
     - 同一版を Codex CLI と DeepSeek Harness CLI へ別々に渡す。
     - 両レビュー完了後、Codex と DeepSeek が自分の指摘に必要な修正を正本計画書へ順番に反映する。
     - Antigravityが修正後の計画を確認し、重複、矛盾、範囲逸脱を整理する。blocking findingは「Blocking findingの条件」を満たすものだけ残す。
     - 判断が一致しない場合だけ、対象資料と両者の最終結果を外部 Codex CLI へ渡し、指摘の根拠、重複、矛盾、採否、必要な修正を統合判断させる。
3. 統合判断（または `codex_only` 時の単独判定）は次のいずれかを明示する。
   - `APPROVE`: 現行計画をそのまま承認する。
   - `APPLY_AND_FINALIZE`: 承認に必要な修正が、具体的・限定的・機械的に適用可能で、新たな設計判断を必要としない場合に限る。統合判断は変更箇所、置換内容または変更条件、確認方法を明示する。
   - `REQUEST_CHANGES`: 設計判断を伴う修正、複数の妥当な選択肢がある修正、または修正後の妥当性を外部独立レビューで再確認すべき場合。
   - `BLOCKED`: 必要資料不足、権限不足、検証不能、またはグローバル停止条件に達した場合。
4. `APPLY_AND_FINALIZE`の場合、Antigravityは指定された修正だけを正本計画書へ適用し、指定された静的検査・自己整合確認を行う。指定外の設計変更を加えない。確認に合格すれば、外部再レビューへ戻らずApprovedを記録する。確認に失敗した場合はApprovedにせず`REQUEST_CHANGES`として扱う。
5. `REQUEST_CHANGES`の場合、修正版を作成した後、有効な policy に従って再度レビューさせる。ただし、P2だけでは再レビューせず、グローバルの回数・時間・同一finding上限を超えてはならない。
6. `APPROVE`または正常に完了した`APPLY_AND_FINALIZE`を最終判断とする。最終判断後は計画レビューへ戻らない。

## 3. 完了

- Approvedなら計画承認を記録し、実装せず停止する。
- Not Approvedなら未解決P0・P1と停止理由を記録し、ユーザーへ報告して停止する。
- commit、release、deployを行わない。
