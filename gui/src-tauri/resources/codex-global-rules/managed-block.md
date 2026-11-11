<!-- BEGIN POVERELLO MANAGED RULES -->
# Poverello Codex Rules

## Verification & Execution Safety
- Do not bypass safety boundaries, credentials, permissions, or access controls.
- Prefer deterministic tests and verification evidence before claiming completion.
- When executing workflow skills (`pov-plan-only`, `pov-build-review`, `pov-full-cycle`), adhere strictly to their role specifications and stop conditions.
- Store all temporary external review prompt and result files across all reviewers, workflow phases, and naming patterns in the `.plan/reviews/` subdirectory (creating it if absent); never create temporary review artifacts directly under `.plan/`. This applies universally regardless of filename.
- Maximum 3 review/correction cycles for `pov-full-cycle`. Stop and report if unresolved findings remain after 3 cycles.
- Do not commit, push, tag, or publish releases without explicit user authorization.
<!-- END POVERELLO MANAGED RULES -->
