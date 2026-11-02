import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import ReviewerDiagnosticsModal from "./ReviewerDiagnosticsModal";
import { LanguageContext } from "../i18n";
import type { ReviewerDiagnosticEvent } from "../types";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

const mockDiagnosticEvents: ReviewerDiagnosticEvent[] = [
  {
    run_id: "test-run-1111",
    timestamp: "2026-11-02T16:00:00Z",
    reviewer: "deepseek",
    model: "deepseek-flash",
    reasoning_effort: "high",
    snapshot_files_count: 12,
    snapshot_total_bytes: 34567,
    snapshot_sha256: "abcdef1234567890",
    sandbox_mechanism: "windows_job_object",
    sandbox_verified: false,
    execution_phase: "preflight_sandbox_check",
    process_spawned: false,
    exit_code: null,
    timed_out: false,
    cancelled: false,
    children_reaped: false,
    verdict: "UNAVAILABLE",
    error_category: "sandbox_unverified_fail_closed",
    external_dispatch_occurred: false,
    duration_ms: 15,
  },
];

describe("ReviewerDiagnosticsModal Component (§18)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("does not render when isOpen is false", () => {
    const { container } = render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ReviewerDiagnosticsModal isOpen={false} onClose={vi.fn()} />
      </LanguageContext.Provider>
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders diagnostic event list, details, and fail-closed UNAVAILABLE state", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_reviewer_diagnostic_events") {
        return Promise.resolve(mockDiagnosticEvents);
      }
      return Promise.resolve();
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ReviewerDiagnosticsModal isOpen={true} onClose={vi.fn()} />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("reviewer-diagnostics-modal")).toBeInTheDocument();
      expect(screen.getByText("DEEPSEEK — deepseek-flash")).toBeInTheDocument();
      expect(screen.getByText("NO (Fail-Closed UNAVAILABLE)")).toBeInTheDocument();
      expect(screen.getByText("sandbox_unverified_fail_closed")).toBeInTheDocument();
    });
  });

  it("handles report export and copies details without triggering external commands", async () => {
    const user = userEvent.setup();
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "get_reviewer_diagnostic_events") {
        return Promise.resolve(mockDiagnosticEvents);
      }
      if (cmd === "export_reviewer_diagnostics_report") {
        return Promise.resolve(JSON.stringify(mockDiagnosticEvents, null, 2));
      }
      return Promise.resolve();
    });

    // Mock clipboard API
    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, "clipboard", {
      value: {
        writeText: writeTextMock,
      },
      configurable: true,
    });

    render(
      <LanguageContext.Provider value={{ lang: "en", setLang: vi.fn() }}>
        <ReviewerDiagnosticsModal isOpen={true} onClose={vi.fn()} />
      </LanguageContext.Provider>
    );

    await waitFor(() => {
      expect(screen.getByTestId("btn-export-diagnostic-report")).toBeInTheDocument();
    });

    // Click Export
    await user.click(screen.getByTestId("btn-export-diagnostic-report"));
    await waitFor(() => {
      expect(screen.getByTestId("diagnostic-export-textarea")).toBeInTheDocument();
      expect(screen.getByTestId("diagnostic-export-textarea")).toHaveValue(
        JSON.stringify(mockDiagnosticEvents, null, 2)
      );
    });

    // Click Copy JSON
    await user.click(screen.getByTestId("btn-copy-diagnostic-detail"));
    expect(writeTextMock).toHaveBeenCalledTimes(1);
  });
});
