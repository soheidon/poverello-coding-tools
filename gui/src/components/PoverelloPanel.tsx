import { useState } from "react";
import ExternalReviewCard from "./ExternalReviewCard";
import AntigravityRulesPanel from "./AntigravityRulesPanel";
import type { ActivePlanContext } from "../types";

export default function PoverelloPanel({ onOpenAgentOptSettings }: { onOpenAgentOptSettings?: () => void }) {
  const [activePlanContext, setActivePlanContext] = useState<ActivePlanContext | null>(null);
  return (
    <div className="poverello-panel" data-testid="poverello-panel">
      <ExternalReviewCard onActivePlanContextChanged={setActivePlanContext} />
      <AntigravityRulesPanel
        activePlanContext={activePlanContext}
        onOpenAgentOptSettings={onOpenAgentOptSettings}
      />
    </div>
  );
}
