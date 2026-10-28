---
name: pov-full-cycle
description: 計画作成、外部Codex/DeepSeek独立計画レビュー、実装・テスト、外部独立実装レビュー、および是正実装のフルサイクルを一貫して進行・管理するSkill。
---

# POV Full Cycle

## 役割

- Antigravity：進行管理、計画作成、内部レビュー、実装、テスト実行、自己レビュー、是正実装、証拠整理を行う。
- Codex：外部`codex exec`（`gpt-6-luna`、`medium`）で独立計画・実装レビューを行う。
- DeepSeek：外部`dsh headless`で独立計画・実装レビューを行う。
- 実装コード、テスト、設定を変更できるのはAntigravityだけである。外部Reviewerはread-onlyとする。
- `pov-mcp-plan`、`pov-mcp-review`、`pov-mcp-revise`はPoverello MCP用の別Skillであり、このSkillから呼ばない。

## 共通契約

- 外部CLIは各グローバルルールに従って実行する。内部エージェントで代替しない。
- 同一案件では正本計画書を1件だけ使う。別版、レビュー依頼書、修正計画書を作らない。
- 計画、レビュー結果、実装報告、是正計画、テスト結果、最終判定を正本計画書へ簡潔に記録する。
- CodexとDeepSeekには同一ハッシュの資料（計画書・コード・差分・テスト証拠）を渡し、互いの回答を見せない。
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

## 1. 計画作成

1. 指示書、リポジトリ、ローカルルール、既存計画、関連コードを確認する。
2. 同じ目的の正本計画書があれば再利用し、なければ1件だけ作る。
3. 目的、確認事実と仮説、範囲、変更対象、不変条件、実装方法、テスト、停止点を記載する。
4. Antigravityが内部レビューし、事実誤認、矛盾、不足を計画書内で修正する。

## 2. 独立計画レビュー

1. 現在の計画書と必要なコードを固定する。
2. 同一版をCodex CLIとDeepSeek Harness CLIへ別々に渡す。両者はread-onlyでレビューする。
3. 各Reviewerは、findingごとに次を示す：対象要件、具体的な失敗条件、severity/blocking、必要な最小修正、合否判定可能なtestまたは証拠。
4. Antigravityはblocking findingが「共通契約」の4条件を満たすことを確認し、重複・同一原因の言い換え・範囲外の任意改善を統合または非blockingへ降格する。
5. 両Reviewerの最終承認状態が同じで、blocking findingがなければSynthesisを行わず、その最終承認状態を採用する。
6. 両Reviewerの判断が異なる、blocking findingの採否が衝突する、または同じ根拠から異なる修正を要求する場合だけ、対象資料と両者の最終結果を外部Codex CLIへ渡してSynthesisする。
7. Synthesisは次のいずれかを返す。
   - `APPROVE`
   - `APPROVE_WITH_NON_BLOCKING_FINDINGS`
   - `APPLY_AND_FINALIZE`
   - `REQUEST_CHANGES`
   - `BLOCKED`
8. `APPLY_AND_FINALIZE`は、承認に必要な修正が限定的・具体的・機械的に適用可能で、新しい設計判断を必要としない場合だけ使用する。Synthesisは変更箇所、置換内容または変更条件、確認方法、修正後に到達すべき最終承認状態を明示する。
9. `APPLY_AND_FINALIZE`の場合、Antigravityは指定された修正だけを正本計画書へ適用し、指定された静的検査・自己整合確認を行う。指定外の設計変更を加えない。確認に合格すれば外部再レビューへ戻らず、Synthesisが指定した最終承認状態を記録する。確認に失敗した場合は`REQUEST_CHANGES`として扱う。
10. `REQUEST_CHANGES`は設計判断を伴う修正、複数の妥当な選択肢がある修正、未確定な安全条件、または修正後の妥当性を外部独立レビューで再確認すべき場合に使用する。Antigravityが計画修正後、同一ハッシュの修正版をCodexとDeepSeekへ再度独立レビューさせる。
11. P2だけでは再レビューせず、グローバルの回数・時間・同一finding上限を超えてはならない。
12. 計画承認（`APPROVED`または`APPROVED_WITH_NON_BLOCKING_FINDINGS`）が得られるまで実装に着手しない。
13. `pov-full-cycle`の呼び出し時点で実装まで許可されているため、計画承認後は同じ依頼範囲内の実装へそのまま進む。ただし共通契約で別ゲートとした操作は実行しない。

## 3. 実装と自己レビュー

1. Antigravityが承認済み計画の範囲だけを実装する。
2. 指定テストと必要な回帰テストを実行する。実行したコマンド、終了コード、要約を記録する。
3. テスト未実行、実行不能、結果不明はPASSにしない。`UNVERIFIED`または具体的な停止理由として記録する。
4. Antigravityが実装を自己レビューし、計画との差分、不変条件、未検証事項、逸脱を確認する。
5. 変更ファイル、実装内容、差分、テスト証拠、未検証事項、逸脱、必要な是正候補を正本計画書へ記録する。
6. 実機・ステージング・本番相当の受入が計画で別ゲートなら、自動テスト完了を「改修全体完了」と呼ばない。`CODE_IMPLEMENTATION_APPROVED`や`STAGING_ACCEPTANCE_PENDING`等、実際の到達点を明記する。

## 4. 独立実装レビューと是正

1. 現在の実装、正確な差分、承認済み計画、実装者のテスト証拠、未検証事項を固定する。
2. 同一版をCodex CLIとDeepSeek Harness CLIへ別々に渡す。両者はread-onlyでレビューする。
3. 各Reviewerは、findingごとに次を示す：対象要件、具体的な失敗条件、severity/blocking、必要な最小修正、合否判定可能なtestまたは証拠。
4. Reviewer自身がテストを再実行した場合だけ、そのコマンド・終了コード・結果をReviewer実行証拠として記録する。実装者ログを読んだだけならその旨を明記する。
5. Antigravityはblocking findingが「共通契約」の4条件を満たすことを確認し、同一原因のfindingを統合する。既存findingの解決条件を満たした後、任意改善だけを理由に新たなblocking findingとしてレビューを長期化させない。
6. 両Reviewerの最終承認状態が同じで、blocking findingがなければSynthesisを行わず、その最終承認状態を採用する。
7. 両Reviewerの判断が異なる、blocking findingの採否が衝突する、または同じ根拠から異なる修正を要求する場合だけ、対象資料と両者の最終結果を外部Codex CLIへ渡してSynthesisする。
8. Synthesisは`APPROVE`、`APPROVE_WITH_NON_BLOCKING_FINDINGS`、`APPLY_AND_FINALIZE`、`REQUEST_CHANGES`、`BLOCKED`のいずれかを返す。
9. `APPLY_AND_FINALIZE`は最終承認状態ではなく、限定的・機械的な修正指示である。Synthesisは変更箇所、変更内容、指定テスト、差分確認、修正後に到達すべき最終承認状態を明示する。
10. `APPLY_AND_FINALIZE`の場合、Antigravityが指定修正だけを適用し、指定テスト・差分確認を行う。完全一致した場合は再レビューなしでSynthesis指定の最終承認状態を記録できる。一致しない、追加設計判断が必要、または新たなblocking findingが生じた場合は`REQUEST_CHANGES`へ戻す。
11. `REQUEST_CHANGES`の場合、Antigravityが採用済みfindingだけから最小の是正計画を正本計画書へ作る。
12. 是正計画は承認済み範囲を越えてはならない。範囲、公開API、権限、費用、依存関係、外部接続、目的の変更が必要なら停止してユーザーへ戻す。
13. Antigravityが確定した是正だけを実装し、関連テストと必要な回帰テストを同じ最終treeで再実行する。
14. 修正後は、未解決blocking findingが0件であることを確認する。非blocking findingは残してよいが、残存リスクとして記録する。
15. 通常の`REQUEST_CHANGES`是正後は独立実装再レビューへ戻る。P2だけでは再レビューしない。
16. グローバルルールの上限内でのみ反復し、上限到達時は停止して残るblocking finding、実施済み修正、テスト結果、未検証事項を報告する。

## 5. 完了判定

1. `APPROVED`：blocking finding 0件、承認済み実装範囲と必須自動検証が完了している。
2. `APPROVED_WITH_NON_BLOCKING_FINDINGS`：blocking finding 0件だが、将来改善や残存リスクがある。非blocking findingを明示する。
3. `REQUEST_CHANGES`：blocking findingが残り、承認済み範囲内で是正可能である。
4. `BLOCKED`：必要な是正が範囲外、検証不能、権限不足、結果不明、または上限到達である。
5. 実機・ステージング・本番受入が未実施なら、実装レビュー承認後も`STAGING_ACCEPTANCE_PENDING`等の未完了ゲートを残す。「本改修完了」「release ready」等、未実施ゲートを含む表現を使わない。
6. コード実装レビューの承認と、実機・ステージング・本番受入の承認を混同しない。
7. 最終結果、証拠の出所、未検証事項、残存リスク、次に必要な明示的許可を正本計画書へ記録する。
8. commit、push、tag、release、deploy、本番変更は行わず停止する。ただしユーザーが別途明示的に許可した場合は、その別操作として扱う。
