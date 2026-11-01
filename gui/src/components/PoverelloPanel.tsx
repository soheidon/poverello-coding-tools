import ExternalReviewCard from "./ExternalReviewCard";
import AntigravityRulesPanel from "./AntigravityRulesPanel";

export default function PoverelloPanel() {
  return (
    <div className="poverello-panel" data-testid="poverello-panel">
      <ExternalReviewCard />
      <AntigravityRulesPanel />
    </div>
  );
}
