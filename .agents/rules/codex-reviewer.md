---
trigger: model_decision
description: Codex review guidance for completed implementations against approved plans, including risk-proportionate and release-delta review. Do not use for trivial text-only edits.
---

# Codex Reviewer Rule

## Plan-review independence

- For **plan documents only**, a plan may be authored and reviewed by the same Codex agent and still count as the independent plan-review gate for the workflow. A separate agent, model, or human reviewer is not required by default.
- The reviewing pass must be a distinct review step: switch from authoring to reviewer perspective and assess the written plan against its scope, contracts, acceptance criteria, safety constraints, testability, and stop conditions. Do not approve solely because the user requested an approval loop.
- Record that the same Codex agent authored and reviewed the plan, and summarize the review basis. “Independent” here means a distinct, read-only review pass independent of the drafting step; do not imply that a different person, model, or agent performed it.
- This plan-only rule does **not** make implementation/code review, security validation, or release review independent when performed by the implementer. Those reviews retain the independence requirements in the applicable plan and project rules. Honor any explicit user or plan requirement for a different reviewer.

## Review depth and gate deduplication

Use risk-proportionate review. The purpose is to catch material defects, not to maximize review count or force an `Approved` verdict through repeated cycles.

- Review one coherent feature/change unit, not each file or implementation substep separately. Do not repeat a review gate when the reviewed artifact and its evidence have not materially changed.
- For trivial text-only or clearly localized low-risk changes, inspect the exact diff and run only the directly relevant check. Skip a separate external review unless the user, project plan, or a consequential contract explicitly requires it.
- For ordinary feature work, perform one independent review of the complete approved change unit, using targeted tests and the relevant diff. Do not require a full repository suite when targeted checks adequately cover the change and no broad regression risk is introduced.
- Reserve exhaustive path-by-path, broad regression, compatibility, security, migration, packaging, or release review for changes whose risk or explicit acceptance criteria warrant it. Keep release readiness as a separate decision from implementation approval.
- Reuse valid test and inspection evidence when the source snapshot is unchanged. Re-run only checks invalidated by a change or needed to close a concrete finding; do not rerun suites merely to make a report look more complete.
- Treat an approval as bound to the reviewed artifact identity (commit/tree or deterministic manifest), scope, acceptance criteria, and evidence. Carry it forward while those bindings remain unchanged; do not re-review already approved files or subsystems from scratch.
- Treat the designated plan file as the primary project record for review verdicts, reviewed scope/identity, evidence, and unresolved findings. Read its review history before starting; reuse recorded approvals when their artifact and scope bindings still match, and report any missing linkage instead of silently repeating the review.
- Count review work by distinct review gates, not by files, test commands, or report updates. A completed commit-level review recorded in the plan remains valid for that exact commit/tree and scope; do not count or repeat it as a new review during release preparation unless later changes invalidate its approval.
- When all constituent commits already have valid recorded approvals and the release candidate contains exactly those reviewed changes, do not add another independent code review by default. Perform a release-delta review only for unreviewed changes, a concrete cross-component risk not covered by prior reviews, or an explicit release acceptance criterion that remains unverified; state the trigger and limit review to it. If none applies, reuse the recorded approvals and proceed directly to publication preflight.
- Keep publication preflight (branch, tag, version, assets, clean-tree and remote checks) separate from code-review counts. It is a release checklist, not another implementation review.
- Do not require a second generic review just because an approved candidate is committed. Verify that the commit contains exactly the approved tree/path set and expected metadata. Reopen substantive review only if the commit changes content, scope, or relevant evidence.
- A final whole-candidate review should be an integration/release delta review: verify that component approvals cover the exact candidate, examine cross-component interactions and previously unreviewed changes, confirm version/package/source correspondence and release-specific gates, and check that no approval was invalidated. Do not repeat settled component-level analysis without a concrete reason.
- Report findings by severity and distinguish blocking correctness/safety issues from non-blocking suggestions. Do not turn preferences or unrelated cleanup into blockers.
- If a review requests changes, create one bounded corrective unit and review that unit together with the affected acceptance criteria. Do not restart the entire review from scratch unless the correction changes the broader contract or invalidates prior evidence.
- Stop rather than loop indefinitely: if the same blocking finding remains after two corrective cycles, or a gate has three consecutive non-approval verdicts, summarize the evidence and request escalation or human direction. Never claim approval solely because a loop was requested.
- State the review denominator when reporting progress (for example, completed required gates / total currently identified gates). Do not present an approximate percentage as a measured completion metric.

For non-trivial implementation tasks in this repository before requesting commit approval:

1. If the current task is being reviewed through the `/pov-mcp-review` command (or legacy `/anthro-review`), do NOT invoke `poverello/review` (or `anthro-bridge/review`) separately. The command workflow owns the review call.
2. Ensure the implementation plan against which the changes will be evaluated is approved and clearly identified.
3. Collect the working tree status using `git status --short`.
4. Collect all tracked changes relative to HEAD using `git diff HEAD`.
5. For all review-relevant untracked files (source, tests, configs, scripts, resources, docs), read their contents and include them as synthetic diff blocks (`--- UNTRACKED FILE: <path> ---`). For large binaries or generated files, provide path, type, size, and reason for omission. Never silently truncate diff evidence.
6. If contract boundaries, caller/callee invariants, or surrounding repository context were inspected, distill them into `additional_context`.
7. Collect automated test execution results. Local test execution is not subject to the exactly-once rule: run targeted tests, regression tests, full suites, package validations, or domain-specific invariant/reproducibility checks iteratively as needed to assemble complete review evidence.
8. Call canonical `poverello/review` (or legacy `anthro-bridge/review` if `poverello` is not yet configured) to obtain the independent review verdict:
   - The exactly-once rule applies strictly to `poverello/review` (or `anthro-bridge/review`) tool calls, not to local test execution.
   - Duplicate review calls are prohibited once a usable verdict is obtained.
   - If a transport or decoding failure occurs, exactly 1 recovery retry is permitted.
9. Present the verdict to the user verbatim:
   - `Approved` -> `Commit readiness: READY`
   - `Approved with recommendations` -> `Commit readiness: READY`
   - `Not approved` -> `Commit readiness: NOT READY` (remediation required before committing).
10. The reviewer tool is strictly read-only. Never perform file edits or git commits inside the reviewer call.

For release readiness, use the plan file's approval ledger and existing verification evidence first. Report component approvals, any triggered release-delta review, and publication readiness as separate outcomes. Do not create an exact-candidate/integration code-review gate solely because release is approaching: require it only when prior approvals do not cover the candidate or a concrete integration criterion remains unverified. Reuse prior approvals while artifact identity and scope match; do not restart settled component reviews.

## Public Release-Note Content

- Public release notes (`NEWS.md` and GitHub Release notes) describe user-visible behavior and capabilities actually delivered in that release. Keep them concise and focused on shipped changes.
- Do not write about work that was not implemented in the release, including features postponed/deferred to a later version, excluded or unavailable features, or implementation retained only in the repository. Do not use release notes to announce roadmap deferrals or to contrast an unshipped feature with existing workflows.
- Do not write about the release/testing process. In particular, never state that Windows installer lifecycle testing was skipped/not run, whether VM testing or pre-release manual installer testing was performed, or that install/upgrade/uninstall results are unavailable. Do not mention the user's test choice or risk acceptance. Keep all skipped/not-run test and risk records in the internal plan/release verification report.
- Include a limitation only when it materially affects the behavior of a shipped feature and a user needs it to use that feature safely or accurately. Describe the concrete user-facing limitation, not the internal decision, test history, or roadmap state. If no shipped change merits a note, omit the section rather than filling it with non-delivery status.
- Before publication, review both the version section in `NEWS.md` and the derived GitHub Release body against this policy. Preserve skipped-test and deferred-scope evidence in the designated internal plan, not the public release notes.

For a personal-use release where the user explicitly directs that manual Windows smoke testing happen after publication, treat the installer lifecycle checklist as post-release follow-up, not a pre-publication blocker. Before publication, report every checklist item as `PENDING / NOT RUN`; never imply it passed, and disclose the user-accepted risk in the release report. After publication, perform the checklist on a clean or disposable Windows environment when available and record the results. A VM is optional; a dedicated clean physical Windows test machine is acceptable if isolated from the user's existing installation and data. Do not use or alter an existing installation as a test target without a separately approved plan and explicit authorization. A fake-registry harness or static inspection does not substitute for the manual lifecycle checks. If post-release testing finds a defect, report it and prepare a corrective-release plan; do not move, delete, or replace the published tag/release without separate explicit authorization.

## Release Smoke-Test Disposition (supersedes any conflicting default above)

Before each release, obtain and record exactly one user-selected disposition:

1. **VM test before release** — run the full manual installer lifecycle checklist in a disposable virtual machine, using VMware or Windows' built-in virtualization facility (such as Hyper-V/Windows Sandbox when suitable). Record the exact environment and results. Do not touch the user's existing installation or data.
2. **Manual smoke test before release** — run the checklist on a clean, dedicated Windows test environment before publication. Do not touch the user's existing installation or data without separate explicit authorization.
3. **Skip all tests** — omit both VM testing and pre-release manual smoke testing. Mark all lifecycle checks `SKIPPED / NOT RUN`, disclose that there is no lifecycle-test evidence, and record the user's explicit acceptance of that risk. Static inspection or a fake-registry harness is not a substitute for either test option.

Do not choose silently or carry a prior release's choice forward. If option 1 or 2 is selected but a safe environment is unavailable, stop before publication and ask for direction; do not switch options automatically. If option 3 is selected, the omitted tests are not publication blockers, but the omission and accepted risk must be explicit in the release report. If later-authorized testing finds a defect, report it and prepare a corrective-release plan; do not move, delete, or replace a published tag/release without separate explicit authorization.
