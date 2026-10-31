import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import AntigravityRulesPanel from "./AntigravityRulesPanel";
import * as i18n from "../i18n";
import { LanguageProvider } from "../i18n";
import { translations as ja } from "../i18n/lang/ja";
import { translations as en } from "../i18n/lang/en";
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
      auto_activation: false,
      web_research: "unrestricted",
      response_verbosity: "standard",
      repository_exploration: "standard",
      verification: "standard",
    },
    preview_text: "Automatic activation: Disabled\nVerification: standard",
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

  it("renders Operation Mode segments, presets, and exact English descriptions", async () => {
    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getByText("Poverello Direct Coding")).toBeInTheDocument();
    });

    // Initial status matches Gemini 3.8 Standard preset
    expect(
      screen.getByText(
        "Disables automatic activation (manual invocation only) and applies standard, unrestricted settings across all four axes."
      )
    ).toBeInTheDocument();

    // Switch to Gemini 3.7 Equivalent (Lightweight)
    const lightweightBtn = screen.getByRole("button", { name: "Gemini 3.7 Equivalent (Lightweight)" });
    await user.click(lightweightBtn);

    expect(
      screen.getByText(
        "A Poverello preset modeled after Gemini 3.7 Flash's lightweight coding behavior. Prioritizes direct implementation over expansive exploration and heavy verification (does not change the underlying model)."
      )
    ).toBeInTheDocument();

    // Click Custom preset
    const customBtn = screen.getByRole("button", { name: "Custom" });
    await user.click(customBtn);

    expect(
      screen.getByText(
        "Customized configuration with individual settings for web search, verbosity, exploration, verification, and auto-activation."
      )
    ).toBeInTheDocument();
  });

  it("toggles Advanced Details accordion and renders state indicators", async () => {
    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getByText("Poverello Direct Coding")).toBeInTheDocument();
    });

    // Accordion is closed by default
    expect(screen.queryByText("Status: Managed and synced")).not.toBeInTheDocument();

    // Click accordion header
    const accordionBtn = screen.getByRole("button", { name: /Advanced Details/i });
    await user.click(accordionBtn);

    // Accordion content is now visible
    expect(screen.getByText("Status: Managed and synced")).toBeInTheDocument();
    // Rule Path is normalized to forward slashes for cross-locale display
    expect(screen.getByText("C:/Users/Sohei/.gemini/config/rules/pov-direct.md")).toBeInTheDocument();
    expect(screen.getByText(/3,450 \/ 24,000 bytes/)).toBeInTheDocument();
    expect(screen.getByText(/Automatic activation: Disabled/)).toBeInTheDocument();
  });

  it("triggers save_pov_direct_settings on Save & Apply click", async () => {
    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getByText("Poverello Direct Coding")).toBeInTheDocument();
    });

    const saveButton = screen.getByRole("button", { name: "Save & Apply" });
    await user.click(saveButton);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "save_pov_direct_settings",
        expect.objectContaining({
          request: expect.objectContaining({
            operation: "apply",
            expected_state: "managed_synced",
            expected_content_hash: "abc123hash",
            settings: expect.objectContaining({
              auto_activation: false,
              web_research: "unrestricted",
            }),
          }),
        })
      );
    });
  });

  it("opens rules folder when Open Rules Folder is clicked", async () => {
    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getByText("Poverello Direct Coding")).toBeInTheDocument();
    });

    const openFolderButton = screen.getByRole("button", { name: "Open Rules Folder" });
    await user.click(openFolderButton);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("open_antigravity_rules_folder");
    });
  });

  it("normalizes structured Rule Path to forward slashes while strictly preserving Rule Preview backslashes", async () => {
    const statusWithEscapes: PovDirectStatus = {
      ...initialStatus,
      rule_path: "C:\\Windows\\System32\\drivers\\etc\\hosts",
      preview_text: "Regex pattern: \\[A-Z\\]\\\\d+ and path C:\\sample\\path",
    };
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === "get_pov_direct_status") return statusWithEscapes;
      return null;
    });

    const user = userEvent.setup();
    render(
      <LanguageProvider>
        <AntigravityRulesPanel />
      </LanguageProvider>
    );

    await waitFor(() => {
      expect(screen.getByText("Poverello Direct Coding")).toBeInTheDocument();
    });

    // Open Advanced Details
    const accordionBtn = screen.getByRole("button", { name: /Advanced Details/i });
    await user.click(accordionBtn);

    // Rule Path: normalized to forward slashes
    expect(screen.getByText("C:/Windows/System32/drivers/etc/hosts")).toBeInTheDocument();

    // Rule Preview: backslashes MUST be preserved as-is without alteration
    expect(screen.getByText("Regex pattern: \\[A-Z\\]\\\\d+ and path C:\\sample\\path")).toBeInTheDocument();
  });

  it("renders Direct Coding panel in Japanese when ja locale is active", async () => {
    vi.spyOn(i18n, "useTranslation").mockImplementation(() => ({
      t: (key: string, params?: Record<string, string | number>) => {
        let text = (ja as Record<string, string>)[key] ?? key;
        if (params) {
          for (const [k, v] of Object.entries(params)) {
            text = text.replace(`{${k}}`, String(v));
          }
        }
        return text;
      },
      lang: "ja",
    }) as ReturnType<typeof i18n.useTranslation>);

    render(<AntigravityRulesPanel />);

    await waitFor(() => {
      expect(screen.getByText("Poverello Direct Coding")).toBeInTheDocument();
    });

    expect(screen.getByText("Antigravityの実装作業を軽量化する条件付きルール")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "適用 / 同期" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "ルールフォルダを開く" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "デフォルトに戻す" })).toBeInTheDocument();
    expect(screen.getByText("動作モード")).toBeInTheDocument();
    expect(screen.getByText("自動適用")).toBeInTheDocument();
    expect(screen.getByText("Web検索")).toBeInTheDocument();
    expect(screen.getByText("応答の冗長性")).toBeInTheDocument();
    expect(screen.getByText("コード探索")).toBeInTheDocument();
    expect(screen.getByText("検証範囲")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /詳細情報/i })).toBeInTheDocument();
  });

  it("renders Direct Coding panel in English when en locale is active", async () => {
    vi.spyOn(i18n, "useTranslation").mockImplementation(() => ({
      t: (key: string, params?: Record<string, string | number>) => {
        let text = (en as Record<string, string>)[key] ?? key;
        if (params) {
          for (const [k, v] of Object.entries(params)) {
            text = text.replace(`{${k}}`, String(v));
          }
        }
        return text;
      },
      lang: "en",
    }) as ReturnType<typeof i18n.useTranslation>);

    render(<AntigravityRulesPanel />);

    await waitFor(() => {
      expect(screen.getByText("Poverello Direct Coding")).toBeInTheDocument();
    });

    expect(screen.getByText("Conditional rules for lightweight Antigravity software implementation")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save & Apply" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Rules Folder" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Reset to Defaults" })).toBeInTheDocument();
    expect(screen.getByText("Operation Mode")).toBeInTheDocument();
    expect(screen.getByText("Auto Activation")).toBeInTheDocument();
    expect(screen.getByText("Web Research")).toBeInTheDocument();
    expect(screen.getByText("Response Verbosity")).toBeInTheDocument();
    expect(screen.getByText("Repository Exploration")).toBeInTheDocument();
    expect(screen.getByText("Verification")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Advanced Details/i })).toBeInTheDocument();
  });
});
