---
trigger: model_decision
description: Use for reviewing completed implementations against approved implementation plans before requesting commit approval. Do not use for trivial text-only edits.
---

# DeepSeek Reviewer Rule

For non-trivial implementation tasks in this repository before requesting commit approval:

1. If the current task is being reviewed through the `/pov-review` command (or legacy `/anthro-review`), do NOT invoke `poverello/review` (or `anthro-bridge/review`) separately. The command workflow owns the review call.
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
