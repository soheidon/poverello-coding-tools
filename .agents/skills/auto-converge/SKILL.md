---
name: auto-converge
description: Codex Auto-Converge / Review Orchestrator skill. Enforces mechanical review gates, independent read-only Reviewer sessions, stage-specific composite approval bindings, and bounded Luna -> Sol escalation via the authoritative Rust guard engine.
---

# Codex Auto-Converge / Review Orchestrator (`auto-converge`)

> **Current availability:** The current production desktop build does not run Auto-Converge. Its Reviewer OS-isolation preflight intentionally fails closed before any configured agent process is launched, and the UI disables Start. Do not describe this build as operational, attempt to bypass the preflight, or claim that production isolation is active. The workflow below documents the intended lifecycle for a future build with a verified OS isolation backend.

The intended Poverello Auto-Converge convergence lifecycle is:
`PlanCreate -> PlanReview -> PlanApproved -> PlanFrozen -> Implement -> ImplementationReview -> (Corrective Loop) -> ImplementationApproved -> VerificationRequired -> ReadyForReleaseReview -> ReleaseReadyHalted / ReleaseCompleted / Blocked`.

## Calling Convention

Always delegate execution to `scripts/converge.ps1`, which acts strictly as a client process runner and delegates all state transitions, cryptographic digest validations, and permission checks to the sole authoritative Rust guard binary:

```powershell
pwsh -NoProfile -File .agents/skills/auto-converge/scripts/converge.ps1 -StateFile <path-to-run_state.json> -Action <subcommand>
```

Canonical Rust authoritative CLI transport:
```powershell
anthro-bridge.exe auto-converge-guard <subcommand> --state-file <path> [arguments...]
```

## Role Definitions & Write Boundaries

| Role | Default Model | Default Effort | Write Scope |
|---|---|---|---|
| `Planner` | GPT-6 Luna | High | `.plan/` only (never engine state or source tree) |
| `Plan Reviewer` | GPT-6 Luna | High | Strictly READ-ONLY (OS temporary directory snapshot) |
| `Implementer` | GPT-6 Luna | High | Frozen `runtime_allowlist` only (`Implement` / `CorrectiveImplement`) |
| `Implementation Reviewer` | GPT-6 Luna | High | Strictly READ-ONLY (OS temporary directory snapshot) |
| `Release Reviewer` | GPT-6 Luna | High | Strictly READ-ONLY (OS temporary directory snapshot) |
| `Escalation Planner` | GPT-6 Sol | High | `.plan/` only |
| `Escalation Reviewer` | GPT-6 Sol | High | Strictly READ-ONLY (OS temporary directory snapshot) |

## 14 Unconditional Hard Invariants

1. **Reviewer Independence**: `reviewer_session_id != producer_session_id`.
2. **Reviewer Read-Only (required design contract, not active in the current production build)**: A future supported production backend must enforce the 3-tier boundary (`--sandbox read-only`, external OS temp snapshot checkout, and pre/post `FilesystemManifestHash` comparison) plus a positively verified platform isolation preflight. The current production preflight fails closed before agent launch; tests do not establish production isolation.
3. **No Unapproved Implementation**: Code edits require an active, verified `APPROVE` on the plan; advancing to `ImplementationApproved` requires a verified `implementation_diff` `APPROVE` binding (or verified 6-tuple reuse under `ReviewReusePolicy`).
4. **No Unapproved Corrective Work**: Corrective implementation requires an `APPROVE` on `CorrectivePlanReview`.
5. **Stage-Specific Composite Approval Binding**: Governed by `target_type` (`plan`, `corrective_plan`, `implementation_diff`, `release_candidate`) separating `candidate_plan_digest`, `root_plan_digest`, and `active_plan_digest`.
6. **Stage-Dependent Binding Invalidation**: Plan edits invalidate plan and downstream approvals; code edits during `Implement` do not invalidate Plan Approval; post-approval edits or corrective loops invalidate downstream implementation/release approvals and Phase-2 release authorizations.
7. **Fail-Closed Verdicts**: `REQUEST_CHANGES` is never coerced into `APPROVE`.
8. **Rejection of Malformed Outputs & Bounded Retries**: Malformed JSON or unknown `failure_code` taxonomy entries trigger bounded retries (`max_transport_retries = 2`), then transition to `Blocked`.
9. **Immediate Stop on Blocked**: `Blocked` halts automated progression immediately.
10. **Preservation of Pre-existing Dirty Work & Staged Work Protection**: Pre-existing dirty work outside `runtime_allowlist` is preserved via `baseline_file_manifest`. Pre-existing staged work in `.git/index` blocks Auto-Converge commits fail-closed; no confirmation token can override staged work.
11. **Strict Runtime File Scope**: Implementers may only modify paths in `runtime_allowlist` extracted from the plan's `json auto-converge-scope` block and bound via `allowlist_digest`. Engine state (`run_state.json`) is stored in an engine-owned directory outside agent write scopes and protected by an engine authentication tag + monotonic sequence.
12. **Evidence-Based Progression**: Gates require verifiable command exit codes and output manifests.
13. **Finite Loop Bounds**: All review, verification, release-review, and escalation loops are strictly bounded.
14. **Run-Permission Enforcement**: Dual-gate bitmask (`ProfilePolicyCapabilityBitmask & RunPermissionBitmask`) with two-phase release binding (`RunStartPermissionBinding` -> `CandidateReleaseAuthorization` -> `ConcreteReleaseTarget` + `ReleaseExecutionPermissionBinding`).
