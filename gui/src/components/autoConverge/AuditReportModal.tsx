import type { AutoConvergeRunState } from "../../types/autoConverge";

interface AuditReportModalProps {
  runState: AutoConvergeRunState | null;
  open: boolean;
  onClose: () => void;
}

export default function AuditReportModal({
  runState,
  open,
  onClose,
}: AuditReportModalProps) {
  if (!open || !runState) return null;

  return (
    <div className="modal-backdrop" data-testid="audit-report-modal">
      <div className="modal-card">
        <div className="modal-header">
          <h3>Auto-Converge Audit Report — {runState.run_id}</h3>
          <button
            type="button"
            className="btn btn-small"
            data-testid="close-audit-modal-btn"
            onClick={onClose}
          >
            Close
          </button>
        </div>
        <div className="modal-body">
          <p>
            <strong>State:</strong> {runState.current_state} |{" "}
            <strong>Review Mode:</strong> {runState.review_mode} |{" "}
            <strong>Sequence:</strong> {runState.state_sequence}
          </p>
          <p>
            <strong>Root Plan Digest:</strong>{" "}
            {runState.plan_lineage.root_plan_digest ?? "N/A"}
          </p>
          <p>
            <strong>Active Plan Digest:</strong>{" "}
            {runState.plan_lineage.active_plan_digest ?? "N/A"}
          </p>
          <p>
            <strong>Candidate Plan Digest:</strong>{" "}
            {runState.plan_lineage.candidate_plan_digest ?? "None"}
          </p>
          <p>
            <strong>Runtime Run Allowlist:</strong>{" "}
            {runState.runtime_allowlist.length > 0
              ? runState.runtime_allowlist.join(", ")
              : "Not frozen yet"}
          </p>
          <p>
            <strong>Completed Release Actions:</strong>{" "}
            {runState.completed_release_actions.length > 0
              ? runState.completed_release_actions.join(" -> ")
              : "None"}
          </p>
        </div>
      </div>
    </div>
  );
}
