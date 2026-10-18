# Auto-Converge Formal Workflow & Guard Contract Reference

## 1. 19 Discrete FSM States (`AutoConvergeState`)

1. `PlanCreate`
2. `PlanReview`
3. `PlanRevise`
4. `PlanApproved`
5. `PlanFrozen`
6. `Implement`
7. `ImplementationReview`
8. `CorrectivePlanCreate`
9. `CorrectivePlanReview`
10. `CorrectivePlanRevise`
11. `CorrectiveImplement`
12. `ImplementationApproved`
13. `VerificationRequired`
14. `ReadyForReleaseReview`
15. `ReleaseReview`
16. `ReleaseActionsPending`
17. `ReleaseCompleted` (Terminal)
18. `ReleaseReadyHalted` (Terminal)
19. `Blocked` (Terminal safety sink)

## 2. Stage-Differentiated Approval Bindings (`target_type`)

- **`plan` (`PlanReview`)**:
  - `root_plan_digest`: `null`
  - `active_plan_digest`: `null`
  - `candidate_plan_digest`: non-null SHA-256 of initial plan
  - `allowlist_digest`: non-null SHA-256 of canonical `json auto-converge-scope` allowlist
  - `baseline_workspace_digest`: non-null SHA-256 of `baseline_file_manifest`
  - `policy_profile_digest`: non-null SHA-256 of sealed profile snapshot
- **`corrective_plan` (`CorrectivePlanReview`)**:
  - `root_plan_digest`: non-null SHA-256 of locked root plan
  - `active_plan_digest`: non-null SHA-256 of currently approved active plan
  - `candidate_plan_digest`: non-null SHA-256 of corrective plan under review
  - `allowlist_digest`: non-null SHA-256 of corrective allowlist (newly added files must be `clean` in `baseline_file_manifest`)
  - `baseline_workspace_digest`: non-null SHA-256
  - `diff_digest`: non-null SHA-256 of `RunDeltaManifest`
  - `policy_profile_digest`: non-null SHA-256
- **`implementation_diff` (`ImplementationReview`)**:
  - `root_plan_digest`: non-null SHA-256
  - `active_plan_digest`: non-null SHA-256
  - `candidate_plan_digest`: `null`
  - `allowlist_digest`: non-null SHA-256
  - `diff_digest`: non-null SHA-256 of `RunDeltaManifest`
  - `evidence_manifest_digest`: non-null SHA-256
  - `policy_profile_digest`: non-null SHA-256
- **`release_candidate` (`ReleaseReview`)**:
  - `root_plan_digest`: non-null SHA-256
  - `active_plan_digest`: non-null SHA-256
  - `candidate_plan_digest`: `null`
  - `allowlist_digest`: non-null SHA-256
  - `diff_digest`: non-null SHA-256 of `RunDeltaManifest`
  - `evidence_manifest_digest`: non-null SHA-256
  - `release_manifest_digest`: non-null SHA-256
  - `policy_profile_digest`: non-null SHA-256

## 3. Finding Identity & Failure Code Taxonomy

`FindingID = SHA256(category + "|" + normalized_file + "|" + normalized_symbol + "|" + normalized_failure_code)`

Allowed canonical `failure_code` values:
- `INV-001` .. `INV-014`
- `SEC-001` .. `SEC-004`
- `COR-001` .. `COR-004`
- `CON-001` .. `CON-003`
- `PERF-001` .. `PERF-003`
- `SCOPE-001` .. `SCOPE-002`

## 4. Escalation Policy Matrix (`EscalationPolicy`)

- `Enabled`: Escalates on `ForcedRunStart`, `ArchitecturalDefectOrRecommended`, `SameBlockerPersistence`, and `StageReviewBudgetExhausted`.
- `BlockingOnly`: Escalates on `ForcedRunStart`, `ArchitecturalDefectOrRecommended`, and `SameBlockerPersistence`; transitions directly to `Blocked` on `StageReviewBudgetExhausted`.
- `Disabled`: Rejects `ForcedRunStart` at `RunStart`; transitions directly to `Blocked` on all runtime triggers.
