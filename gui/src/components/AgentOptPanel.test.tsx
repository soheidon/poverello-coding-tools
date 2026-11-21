import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import AgentOptPanel from "./AgentOptPanel";
import type { NerfEngineOverview } from "../types";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

function renderDiagnostic(state: string, event: string | null = null) {
  const overview = {
    global_hook_diagnostic: {
      state,
      plugin_version: state === "risk_pattern_detected" ? "0.1.2" : null,
      event,
    },
    workspace_targets: [],
    target_resolution_errors: [],
  } as unknown as NerfEngineOverview;

  return render(
    <AgentOptPanel
      overview={overview}
      busy={false}
      onRefreshOverview={async () => undefined}
      onError={() => undefined}
      onSuccess={() => undefined}
    />,
  );
}

describe("AgentOptPanel global hook diagnostic", () => {
  it("explains the global risk and offers only manual remediation guidance", () => {
    renderDiagnostic("risk_pattern_detected", "PreToolUse");

    const card = screen.getByTestId("agentopt-global-hook-diagnostic");
    expect(card).toHaveTextContent("agentOpt.globalHooks.scope");
    expect(card).toHaveTextContent("agentOpt.globalHooks.status.risk_pattern_detected");
    expect(card).toHaveTextContent("agentOpt.globalHooks.event");
    expect(screen.getByText("agy plugin disable wt-agent-hooks")).toBeInTheDocument();
    expect(card).toHaveTextContent("agentOpt.globalHooks.diagnosticReadOnly");
    expect(invoke).not.toHaveBeenCalled();
  });

  it("does not show a disable command when the global plugin is disabled", () => {
    renderDiagnostic("disabled");

    const card = screen.getByTestId("agentopt-global-hook-diagnostic");
    expect(card).toHaveTextContent("agentOpt.globalHooks.status.disabled");
    expect(screen.queryByText("agy plugin disable wt-agent-hooks")).not.toBeInTheDocument();
  });

  it("distinguishes an absent global plugin from project AgentOpt state", () => {
    renderDiagnostic("plugin_absent");

    const card = screen.getByTestId("agentopt-global-hook-diagnostic");
    expect(card).toHaveTextContent("agentOpt.globalHooks.title");
    expect(card).toHaveTextContent("agentOpt.globalHooks.status.plugin_absent");
    expect(card).toHaveTextContent("agentOpt.globalHooks.scope");
  });

  it("canceling global remediation does not call the backend", () => {
    renderDiagnostic("risk_pattern_detected", "PreToolUse");

    fireEvent.click(screen.getByTestId("btn-disable-wt-agent-hooks"));
    expect(screen.getByTestId("modal-disable-wt-agent-hooks")).toBeInTheDocument();
    fireEvent.click(screen.getByTestId("btn-cancel-disable-wt-agent-hooks"));

    expect(screen.queryByTestId("modal-disable-wt-agent-hooks")).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalled();
  });

  it("requires confirmation, invokes the dedicated command, and refreshes status", async () => {
    const refresh = vi.fn().mockResolvedValue(undefined);
    const onError = vi.fn();
    const onSuccess = vi.fn();
    vi.mocked(invoke).mockResolvedValue({ state: "disabled" });
    render(
      <AgentOptPanel
        overview={{ global_hook_diagnostic: { state: "risk_pattern_detected", plugin_version: "0.1.2", event: "PreToolUse" }, workspace_targets: [], target_resolution_errors: [] } as unknown as NerfEngineOverview}
        busy={false}
        onRefreshOverview={refresh}
        onError={onError}
        onSuccess={onSuccess}
      />,
    );

    fireEvent.click(screen.getByTestId("btn-disable-wt-agent-hooks"));
    expect(invoke).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTestId("btn-confirm-disable-wt-agent-hooks"));

    await waitFor(() => expect(invoke).toHaveBeenCalledWith("disable_wt_agent_hooks_global"));
    await waitFor(() => expect(refresh).toHaveBeenCalledTimes(1));
    expect(onError).toHaveBeenCalledWith(null);
    expect(onSuccess).toHaveBeenCalledWith("agentOpt.globalHooks.disableSuccess");
  });
});
