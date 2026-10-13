import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import AntigravityRulesPanel from "./AntigravityRulesPanel";
import { LanguageProvider } from "../i18n";
import type { PovDirectStatus } from "../types";

const mockInvoke = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

describe("AntigravityRulesPanel Component", () => {
  const initialStatus: PovDirectStatus = {
    rule_path: "C:\\Users\\Sohei\\.gemini\\config\\rules\\pov-direct.md",
    skill_path: "C:\\Users\\Sohei\\.gemini\\config\\skills\\pov-direct\\SKILL.md",
    skill_present: true,
    state: "managed_synced",
    settings_state: "embedded_valid",
    settings: {
      auto_activation: true,
      web_research: "requested_or_necessary",
      response_verbosity: "concise",
      repository_exploration: "minimal",
      verification: "focused",
    },
    preview_text: "Automatic activation: Enabled",
    expanded_byte_size: 3450,
    byte_limit: 24000,
    can_apply: true,
    available_operations: ["apply"],
    block_reason: null,
    content_hash: "abc123hash",
    legacy_sidecar_present: false,
  };

  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_pov_direct_status") {
        return initialStatus;
      }
      if (cmd === "save_pov_direct_settings") {
        return initialStatus;
      }
      if (cmd === "open_antigravity_rules_folder") {
        return undefined;
      }
      throw new Error(`Unhandled mock command: ${cmd}`);
    });
  });

  it("renders status, badges, settings and preview", async () => {
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getAllByText("Poverello Direct Coding").length).toBeGreaterThan(0);
    });

    // Check status badge
    expect(screen.getByText("Synced")).toBeInTheDocument();

    // Check file path display
    expect(screen.getByText(initialStatus.rule_path)).toBeInTheDocument();

    // Check byte counter
    expect(screen.getByText(/3,450 \/ 24,000 bytes/)).toBeInTheDocument();
  });

  it("triggers save_pov_direct_settings on Save & Apply click", async () => {
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getAllByText("Poverello Direct Coding").length).toBeGreaterThan(0);
    });

    const saveButton = screen.getByRole("button", { name: "Save & Apply" });
    fireEvent.click(saveButton);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "save_pov_direct_settings",
        expect.objectContaining({
          request: expect.objectContaining({
            operation: "apply",
            expected_state: "managed_synced",
            expected_content_hash: "abc123hash",
            settings: expect.objectContaining({
              auto_activation: true,
            }),
          }),
        })
      );
    });
  });

  it("opens rules folder when Open Rules Folder is clicked", async () => {
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getAllByText("Poverello Direct Coding").length).toBeGreaterThan(0);
    });

    const openFolderButton = screen.getByRole("button", { name: "Open Rules Folder" });
    fireEvent.click(openFolderButton);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("open_antigravity_rules_folder");
    });
  });
});
