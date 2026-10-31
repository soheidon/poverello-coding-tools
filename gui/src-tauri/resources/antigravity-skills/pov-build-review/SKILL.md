---
name: pov-build-review
description: 承認済み計画に基づく実装・テスト、外部Codex/DeepSeek独立実装レビュー、必要な是正実装、および証拠境界を管理するSkill。
---

# POV Build Review

## 役割

- Antigravity：進行管理、実装、テスト実行、自己レビュー、是正実装、証拠整理を行う。
- Codex：外部`codex exec`（`gpt-6-luna`、`medium`）で独立実装レビューを行う。
- DeepSeek：外部`dsh headless`で独立実装レビューを行う。
- 外部Reviewer構成は、共通設定ファイル `%USERPROFILE%\.gemini\antigravity-ide\pov-reviewer-policy.json`（他OSでは対応するホームディレクトリ配下）の設定に従う。
- 実装コード、テスト、設定を変更できるのはAntigravityだけである。外部Reviewerはread-onlyとする。
- `pov-mcp-plan`、`pov-mcp-review`、`pov-mcp-revise`はPoverello MCP用の別Skillであり、このSkillから呼ばない。

## 共通契約

- 外部CLIは各グローバルルールに従って実行する。内部エージェントで代替しない。
- 正本計画書は1件だけ使う。別版、レビュー依頼書、修正計画書を作らない。
- 実装報告、レビュー結果、是正計画、テスト結果、最終判定を正本計画書へ簡潔に記録する。
- レビュー実行前に実効ポリシー（評価優先順位: Evaluation Precedence）を決定する:
  1. **実行時明示指定:** コマンドライン引数（`--review none` 等）またはユーザープロンプト内の明示指示（「Codex CLI や DeepSeek Harness によるレビューは不要」「外部レビュー不要」等）が存在する場合は、最優先でその値を実効ポリシーとする。
  2. **GUI設定ファイル:** 実行時指定がない場合、共通設定ファイル `%USERPROFILE%\.gemini\antigravity-ide\pov-reviewer-policy.json` を読み取る。`policy` が `"none"`、`"codex_only"`、`"codex_plus_deepseek"` のいずれかであればその値を採用する（`schema_version: 1` の場合はマイグレーション規約により `none`）。
  3. **安全側既定値 (Fail-Safe):** ファイル不在、構文エラー、未知の policy 値、未対応 schema_version、書込障害等の例外時は、安全側既定値 `none` (`Disabled`) を適用する。
- `codex_only` では Codex CLI のみを使用し、DeepSeek Harness は起動・待機・比較・Synthesis のいずれにも使用しない。DeepSeek Rule の有無も要求しない。Codex CLI 失敗時に他Reviewerへ暗黙フォールバックしない。
- `codex_plus_deepseek` では Codex と DeepSeek に同一ハッシュの実装、差分、承認済み計画、実装者テスト証拠を渡し、互いの回答を見せない。
- Reviewerが自分でテストを再実行していない場合、「テストが通ったことを確認した」と記録せず、「実装者が提示した実行証拠を確認した」と記録する。Reviewer自身が再実行した結果とは明確に分離する。
- 承認状態は`APPROVED`、`APPROVED_WITH_NON_BLOCKING_FINDINGS`、`REQUEST_CHANGES`、`BLOCKED`の4種だけを使う。「条件付き承認」「完了推奨」等の自由文を自動的に`APPROVED`へ丸めない。
- blocking findingは、承認済み計画・安全契約・受入条件への具体的な違反であり、再現可能または具体的な失敗シナリオ、必要な最小修正、合否判定可能なテストまたは証拠を伴う場合に限る。
- 文体、好み、将来強化、計画外の追加機能、一般論だけをblocking findingにしない。同一原因の言い換えを新しいblocking findingとして数えない。
- P2だけで修正・再レビューしない。ただしP2が計画の明示的受入条件を破壊し、具体的な失敗を生む場合はblockingとして扱える。
- 回数、時間、再試行、停止条件はグローバルルールを適用する。
- 実装レビューのApprovedは、commit、push、tag、release、deploy、本番変更、実機受入の承認ではない。

## 1. 実装前確認

1. 承認済みの正本計画書、現在のコード、作業ツリー、ローカルルールを確認する。
2. 計画が未承認、対象不明、現在のコードと実質的に不整合、または実装許可範囲を確定できない場合は実装せず報告する。
3. 計画の変更範囲、不変条件、必要テスト、実機・外部サービスを伴う検証、停止点を確認する。
4. 実装前の作業ツリー状態と既存変更を記録し、今回の差分と既存差分を混同しない。reset、clean、stash等は許可がない限り行わない。

## 2. 実装と自己レビュー

1. Antigravityが承認済み計画の範囲だけを実装する。
2. 指定テストと必要な回帰テストを実行する。実行したコマンド、終了コード、要約を記録する。
3. テスト未実行、実行不能、結果不明はPASSにしない。`UNVERIFIED`または具体的な停止理由として記録する。
4. Antigravityが実装を自己レビューし、計画との差分、不変条件、未検証事項、逸脱を確認する。
5. 変更ファイル、実装内容、差分、テスト証拠、未検証事項、逸脱、必要な是正候補を正本計画書へ記録する。
6. 実機・ステージング・本番相当の受入が計画で別ゲートなら、自動テスト完了を「改修全体完了」と呼ばない。`CODE_IMPLEMENTATION_APPROVED`や`STAGING_ACCEPTANCE_PENDING`等、実際の到達点を明記する。

## 3. 独立実装レビュー

1. 現在の実装、正確な差分、承認済み計画、実装者のテスト証拠、未検証事項を固定する。
2. 外部Reviewerはread-onlyとする。
3. 共通設定の `policy` に従ってレビューを実施する：
   - **`none` の場合（外部レビュー無効）**：
     - 外部レビューア呼出コマンド（`codex exec`, `poverello/review`, DeepSeek Harness 等）を一切生成・実行しない（呼出数厳密に 0）。
     - 独立レビューループを開始しない。
     - 実装・テスト完了後、外部実装レビューを行わずに `BUILD COMPLETE — EXTERNAL REVIEW DISABLED (Awaiting user review)` として停止し、ユーザー確認を待つ。
     - **No-Fallback 原則:** レビューア利用不可・エラー等のいかなる例外状況下でも、Codex や DeepSeek へのフォールバック呼出を絶対に実行しない。
   - **`codex_only` の場合**：
     - 固定した資料を外部 Codex CLI のみへ渡す。
     - 各findingにつき、対象要件、具体的な失敗条件、severity/blocking、必要な最小修正、合否判定可能なテストまたは証拠を報告させる。
     - Synthesis は行わず、Codex 単一のレビュー結果に基づき判定を採用する。
   - **`codex_plus_deepseek` の場合**：
     - 同一版を Codex CLI と DeepSeek Harness CLI へ別々に渡す。
     - 各Reviewerは、findingごとに次を示す：対象要件、具体的な失敗条件、severity/blocking、必要な最小修正、合否判定可能なテストまたは証拠。
     - Reviewer自身がテストを再実行した場合だけ、そのコマンド・終了コード・結果をReviewer実行証拠として記録する。実装者ログを読んだだけならその旨を明記する。
     - 両Reviewerの判定が同じで、blocking findingがなければ統合判断を行わず、その判定を採用する。
     - 両Reviewerの判断が異なる、blocking findingの採否が衝突する、または同じ根拠から異なる修正を要求する場合だけ、対象資料と両者の最終結果を外部 Codex CLI へ渡してSynthesisする。
4. Synthesis（または `codex_only` 時のCodex判定）の出力は`APPROVE`、`APPROVE_WITH_NON_BLOCKING_FINDINGS`、`APPLY_AND_FINALIZE`、`REQUEST_CHANGES`、`BLOCKED`のいずれかとする。
5. `APPLY_AND_FINALIZE`は、修正が限定的・具体的・機械的に検証可能で、新しい設計判断を必要としない場合だけ使用する。Antigravityが指定修正を適用し、指定テストと差分確認が一致した場合は再度の独立レビューなしで実装レビューを確定できる。
6. `REQUEST_CHANGES`は設計判断、複数候補、未確定な安全条件、新しいblocking findingが残る場合に使用し、是正後に独立レビューへ戻る。
7. SynthesisやReviewerは実装コードを変更しない。正本計画書への記録はAntigravityが行い、Reviewer出力を要約して出典を区別する。

## 4. 是正実装と再レビュー

1. `REQUEST_CHANGES`の場合、Antigravityが採用済みfindingだけから最小の是正計画を正本計画書へ作る。
2. 是正計画は承認済み範囲を越えてはならない。範囲、公開API、権限、費用、依存関係、外部接続、目的の変更が必要なら停止してユーザーへ戻す。
3. Antigravityが確定した是正だけを実装し、関連テストと必要な回帰テストを同じ最終treeで再実行する。
4. 修正後は、未解決blocking findingが0件であることを確認する。非blocking findingは残してよいが、残存リスクとして記録する。
5. `APPLY_AND_FINALIZE`の場合は、Synthesis指定修正と指定検証が完全一致したかを自己確認し、一致しなければ通常の`REQUEST_CHANGES`経路へ戻す。
6. グローバルルールの上限内でのみ再レビューを行う。上限到達時は停止し、残るblocking finding、実施済み修正、テスト結果、未検証事項を報告する。

## 5. 完了判定

0. 外部レビュー無効時 (`none`): 必須自動検証完了後、`BUILD COMPLETE — EXTERNAL REVIEW DISABLED (Awaiting user review)` を記録し、ユーザー確認待ちとして安全に停止する。
1. `APPROVED`：blocking finding 0件、承認済み実装範囲と必須自動検証が完了している。
2. `APPROVED_WITH_NON_BLOCKING_FINDINGS`：blocking finding 0件だが、将来改善や残存リスクがある。非blocking findingを明示する。
3. `REQUEST_CHANGES`：blocking findingが残り、承認済み範囲内で是正可能である。
4. `BLOCKED`：必要な是正が範囲外、検証不能、権限不足、結果不明、または上限到達である。
5. 実機・ステージング・本番受入が未実施なら、実装レビュー承認後も`STAGING_ACCEPTANCE_PENDING`等の未完了ゲートを残す。「本改修完了」「release ready」等、未実施ゲートを含む表現を使わない。
6. 最終結果、証拠の出所、未検証事項、次に必要な明示的許可を正本計画書へ記録する。
7. commit、push、tag、release、deploy、本番変更は行わず停止する。ただしユーザーが別途明示的に許可した場合は、その別操作として扱う。
