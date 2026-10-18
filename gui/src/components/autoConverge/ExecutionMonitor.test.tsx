import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import ExecutionMonitor from "./ExecutionMonitor";
import type { AutoConvergeRunState } from "../../types/autoConverge";
import { BUILTIN_FALLBACK_PROFILES } from "./AutoConvergePanel";

const SAMPLE_RUN_STATE: AutoConvergeRunState = {
  schema_version: 1,
  state_sequence: 4,
  run_id: "run-001",
  created_at: "2026-10-18T01:00:00Z",
  updated_at: "2026-10-18T01:05:00Z",
  delivery_phase: "PhaseALocalConvergenceMvp",
  plan_lineage: {
    root_plan_digest: "b".repeat(64),
    active_plan_digest: "b".repeat(64),
    candidate_plan_digest: null,
    approved_plan_chain: ["b".repeat(64)],
    approved_plan_chain_digest: "c".repeat(64),
  },
  runtime_allowlist: ["gui/src/App.tsx"],
  allowlist_digest: "c".repeat(64),
  current_diff_digest: "e".repeat(64),
  current_evidence_manifest_digest: "f".repeat(64),
  current_release_manifest_digest: null,
  profile: BUILTIN_FALLBACK_PROFILES[1],
  policy_profile_digest: "a".repeat(64),
  run_permissions: 0,
  effective_permissions: 0,
  release_authorization: {
    run_start_permission_binding: "1".repeat(64),
    release_intent: null,
    candidate_release_authorization: null,
    concrete_release_target: null,
    release_execution_permission_binding: null,
  },
  current_state: "ImplementationReview",
  is_terminal: false,
  is_paused: false,
  blocked_context: null,
  review_mode: "Normal",
  active_producer_model: "gpt-5.4",
  active_reviewer_model: "gpt-5.4",
  active_reasoning_effort: "xhigh",
  cycle_counts: {
    plan_review_count: 1,
    impl_review_count: 1,
    corrective_review_count: 0,
    verification_retry_count: 0,
    release_review_count: 0,
    escalation_count: 0,
    escalation_review_count: 0,
    transport_retry_count: 0,
  },
  findings: [
    {
      finding_id: "finding-12345678",
      finding: {
        finding_key: {
          category: "correctness",
          file: "gui/src/App.tsx",
          symbol: "AppContent",
          failure_code: "state_transition_violation",
        },
        location: "gui/src/App.tsx:260",
        failure_condition: "Missing tab branch",
        acceptance_condition: "Render AutoConvergePanel when mainTab === autoConverge",
        blocking: true,
      },
      consecutive_cycles: 1,
      resolved: false,
    },
  ],
  completed_release_actions: [],
  execution_log: [
    {
      timestamp: "2026-10-18T01:00:00Z",
      state: "ImplementationReview",
      actor: "engine",
      message: "Entered ImplementationReview",
    },
  ],
};

describe("ExecutionMonitor", () => {
  it("renders idle state when no run is active", () => {
    render(<ExecutionMonitor runState={null} />);
    expect(screen.getByTestId("execution-monitor-idle")).toBeTruthy();
  });

  it("renders live counters, active finding, and log stream during active run", () => {
    render(<ExecutionMonitor runState={SAMPLE_RUN_STATE} />);

    expect(screen.getByTestId("monitor-current-state").textContent).toContain(
      "ImplementationReview",
    );
    expect(screen.getByTestId("monitor-plan-cycles").textContent).toContain("1 / 3");
    expect(screen.getByTestId("monitor-impl-cycles").textContent).toContain("1 / 3");
    expect(screen.getByTestId("monitor-active-finding").textContent).toContain(
      "state_transition_violation",
    );
    expect(screen.getByTestId("monitor-log-stream").textContent).toContain(
      "Entered ImplementationReview",
    );
  });
});
