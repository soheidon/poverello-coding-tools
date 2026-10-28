---
name: pov-mcp-review
description: Poverello MCP経由で、設定されたモデルへ処理を依頼するSkill
---

# Poverello MCP Review

Review an implementation against its approved implementation plan using the Poverello external MCP review tool.

## Instructions
1. Identify the approved implementation plan using the following priority:
   - Priority 1: An approved implementation plan present in the active conversation context.
   - Priority 2: An active implementation plan artifact (e.g. `implementation_plan.md`).
   - If neither is found, STOP immediately and ask the user to provide or specify the plan to review. Do not guess from historical transcripts or older plan files (review tool calls = 0).
2. Collect working tree changes and status:
   - Inspect git status: run `git status --short`.
   - Collect tracked changes: run `git diff HEAD` to capture all staged and unstaged modifications from HEAD to the working tree.
   - Collect untracked changes: for all review-relevant untracked files (source, tests, configs, scripts, resources, docs) appearing as `??` in `git status --short`, read their content and append as synthetic diff blocks:
     ```text
     --- UNTRACKED FILE: path/to/file ---
     <content>
     ```
   - For large binary, vendor, or generated files that cannot be transmitted in full, provide explicit metadata instead (path, type, size, omission reason).
   - Do not silently truncate the review payload. If the complete relevant diff cannot fit within context, STOP and report that the review cannot be performed reliably rather than requesting an approval from incomplete evidence.
3. Inspect relevant code and bridge context:
   - Read adjacent source files, contract boundaries, caller/callee invariants, or surrounding repository context when required to understand invariants.
   - Distill the relevant findings into `additional_context`. Never assume repository files inspected by the host agent are automatically visible to the external reviewer model.
4. Collect test results:
   - Local test execution is not subject to the exactly-once rule.
   - Run targeted tests, regression tests, full test suites, package/build validation (`R CMD check`, `cargo test`, `npm test/build`), statistical-invariance checks, reproducibility checks, RNG/deterministic-seed tests, serial/parallel equivalence checks, or other domain-appropriate verification as many times as necessary to collect sufficient review evidence.
   - Tests may be run iteratively: results from one test may justify running additional focused checks before the final review evidence is assembled.
   - For documentation-only changes, testing may be omitted if code logic and packaging invariants are unaffected.
5. Call `poverello/review`:
   - The exactly-once rule applies strictly to `poverello/review` tool calls, not to local test execution or evidence collection.
   - Call `poverello/review` exactly once upon receipt of a usable verdict.
   - A usable verdict must contain one of the following consistent pairs:
     - `Decision: Approved` with `Commit readiness: READY`
     - `Decision: Approved with recommendations` with `Commit readiness: READY`
     - `Decision: Not approved` with `Commit readiness: NOT READY`
   - Any response missing these sections, containing an unsupported decision, or presenting contradictory readiness (e.g. `Approved` + `NOT READY`) is unusable.
   - Duplicate review calls are prohibited once a usable verdict is obtained.
   - If the tool call itself fails or returns an unusable response (e.g. transport, decoding error, or inconsistent verdict), exactly 1 recovery retry is permitted.
   - Supply:
     - `task`: Concise statement of what was implemented.
     - `approved_plan`: The exact text of the approved plan.
     - `git_diff`: Tracked diff (`git diff HEAD`) plus review-relevant untracked files.
     - `git_status`: Output of `git status --short`.
     - `test_results`: Test summary (if applicable).
     - `additional_context`: Surrounding contracts or constraints inspected by the host agent.
6. Present the returned review verdict verbatim (Approved / Approved with recommendations / Not approved) along with the Plan Compliance Matrix and Blocking Issues.
7. Do not edit files.
8. Do not create git commits, tags, or releases.
9. Stop after presenting the review verdict.
