# Repository Agent Guidance

## Implement, observe, iterate

- Do not require 100% documentary certainty before making progress. When the
  task is authorized and bounded, implement the smallest useful change or
  diagnostic, run the appropriate checks, and use concrete compiler errors,
  test failures, logs, and runtime observations to guide the next iteration.
- A hypothesis is enough to begin a reversible, local implementation or a
  deterministic test. Label it as a hypothesis; do not present it as a proven
  root cause. Prefer adding instrumentation or a narrow test that distinguishes
  plausible causes when the cause is unclear.
- After a failure, identify the exact stage reached and the evidence produced.
  Fix the demonstrated issue or refine the diagnostic, then rerun only checks
  whose side effects are within the approved scope. Do not stop merely because
  documentation does not settle the cause.
- Keep each attempt bounded and reproducible. Record the command, inputs,
  observed output, changed files, and remaining uncertainty. Do not turn one
  failure into an unsupported causal claim.

## Safety and evidence boundaries

- This try-and-learn approach does not grant permission to exceed the user's
  requested scope, modify unrelated data, access credentials, contact external
  services, or make OS/security-policy changes. Follow repository plans and
  explicit authorization requirements for native probes, network activity,
  destructive operations, and other material side effects.
- Use offline/unit tests first when they can answer the question. For live or
  OS-level experiments, state the exact target, mutation, cleanup/recovery
  behavior, and attempt limit; stop at that limit. Do not retry a side-effecting
  experiment without authorization when its plan requires fresh approval.
- Separate **documented**, **observed**, **inferred**, and **unverified** claims.
  A passing test proves only what that test actually exercises. Never claim a
  security boundary, root cause, or release readiness without evidence that
  directly supports it.
- Preserve existing user changes. Do not commit, push, tag, or publish unless
  explicitly requested.

## Planning and implementation

- For substantial repository changes, use the Plan → Build → Review workflow:
  define scope, allowed/forbidden files, invariants, tests, commands, and stop
  conditions; implement only that scope; then review the completed change.
- Keep plans actionable rather than using planning as a substitute for
  implementation. If a plan's assumption fails, capture the evidence and revise
  the smallest affected part instead of restarting the entire plan.
