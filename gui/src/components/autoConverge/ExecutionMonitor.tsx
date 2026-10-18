import type { AutoConvergeRunState } from "../../types/autoConverge";

interface ExecutionMonitorProps {
  runState: AutoConvergeRunState | null;
}

export default function ExecutionMonitor({ runState }: ExecutionMonitorProps) {
  if (!runState) {
    return (
      <div className="auto-converge-monitor" data-testid="execution-monitor-idle">
        <p>No active Auto-Converge run. Select a profile and start a run.</p>
      </div>
    );
  }

  const activeFinding = runState.findings.find((f) => !f.resolved);

  return (
    <div className="auto-converge-monitor" data-testid="execution-monitor">
      <div className="monitor-status-row">
        <span data-testid="monitor-current-state">
          Current State: <strong>{runState.current_state}</strong>
        </span>
        <span data-testid="monitor-review-mode">
          Review Mode: <strong>{runState.review_mode}</strong>
        </span>
        <span data-testid="monitor-models">
          Producer: <strong>{runState.active_producer_model}</strong> / Reviewer:{" "}
          <strong>{runState.active_reviewer_model}</strong> (
          {runState.active_reasoning_effort})
        </span>
      </div>

      <div className="monitor-counters-row">
        <span data-testid="monitor-plan-cycles">
          Plan Reviews: {runState.cycle_counts.plan_review_count} /{" "}
          {runState.profile.convergence_limits.max_plan_reviews}
        </span>
        <span data-testid="monitor-impl-cycles">
          Impl Reviews: {runState.cycle_counts.impl_review_count} /{" "}
          {runState.profile.convergence_limits.max_impl_reviews}
        </span>
        <span data-testid="monitor-escalations">
          Escalations: {runState.cycle_counts.escalation_count} /{" "}
          {runState.profile.convergence_limits.max_escalations}
        </span>
      </div>

      {runState.blocked_context && (
        <div className="monitor-blocked-banner" data-testid="monitor-blocked-banner">
          <strong>BLOCKED ({runState.blocked_context.kind}):</strong>{" "}
          {runState.blocked_context.reason}
        </div>
      )}

      {activeFinding && (
        <div className="monitor-active-finding" data-testid="monitor-active-finding">
          <strong>Active Finding [{activeFinding.finding_id.slice(0, 8)}]:</strong>{" "}
          [{activeFinding.finding.finding_key.failure_code}]{" "}
          {activeFinding.finding.failure_condition} ({activeFinding.finding.location})
        </div>
      )}

      <div className="monitor-log-stream" data-testid="monitor-log-stream">
        {runState.execution_log.map((entry, idx) => (
          <div key={idx} className="monitor-log-line">
            [{entry.timestamp}] [{entry.actor}] ({entry.state}) {entry.message}
          </div>
        ))}
      </div>
    </div>
  );
}
