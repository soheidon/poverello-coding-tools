import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within, act, cleanup } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import ApiKeyPanel, { ModelSelector, type ModelSelectorProps } from "./ApiKeyPanel";
import { MODEL_CAPABILITIES } from "../modelCapabilities";
import type { GatewayConfig } from "../types";

// ── Setup ─────────────────────────────────────────────────────────

const invokeMock = invoke as unknown as ReturnType<typeof vi.fn>;

function baseProps(overrides: Partial<ModelSelectorProps> = {}): ModelSelectorProps {
  return {
    providerId: "deepseek",
    modelKey: "claude-sonnet-5",
    gatewayModelLabel: "Sonnet 5 →",
    currentUpstream: "deepseek-flash",
    thinkingModePolicy: "toggleable",
    currentThinkingMode: "thinking",
    currentReasoningEffort: "",
    onSaved: vi.fn().mockResolvedValue(undefined),
    gatewayRunning: false,
    restartGateway: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

function resetInvoke() {
  invokeMock.mockReset();
  invokeMock.mockImplementation(async (cmd: string) => {
    if (cmd === "set_model_upstream") return { restartGateway: false, restartReason: "" };
    if (cmd === "get_config") return null;
    return null;
  });
}

beforeEach(() => {
  resetInvoke();
});

// The current reason-effort value the Flash model's selector would save.
function lastUpstreamSave(): Record<string, unknown> | undefined {
  const calls = invokeMock.mock.calls.filter((c: unknown[]) => c[0] === "set_model_upstream");
  const last = calls[calls.length - 1];
  return (last?.[1] as Record<string, unknown>) ?? undefined;
}

async function setReasoningEffortSelect(value: string) {
  const flashRow = screen.getByText("Sonnet 5 →").closest("div") as HTMLElement;
  const effortSelect = within(flashRow).getByLabelText(/reasoning effort/i);
  await act(async () => {
    await userEvent.selectOptions(effortSelect, value);
  });
}

// ── Tests ─────────────────────────────────────────────────────────

describe("DeepSeek V4 Flash reasoning effort", () => {
  it("hides the effort selector in Normal mode", () => {
    render(<ModelSelector {...baseProps({ currentThinkingMode: "normal", currentReasoningEffort: "" })} />);
    const flashRow = screen.getByText("Sonnet 5 →").closest("div") as HTMLElement;
    expect(within(flashRow).queryByLabelText(/reasoning effort/i)).not.toBeInTheDocument();
  });

  it("shows only Low / High / Max options in Thinking mode", () => {
    render(<ModelSelector {...baseProps({ currentThinkingMode: "thinking" })} />);
    const flashRow = screen.getByText("Sonnet 5 →").closest("div") as HTMLElement;
    const effortSelect = within(flashRow).getByLabelText(/reasoning effort/i);
    const options = Array.from(effortSelect.querySelectorAll("option")).map((o) => o.textContent);
    expect(options).toEqual(["Low", "High", "Max"]);
    expect(options).not.toContain("Medium");
    expect(options).not.toContain("XHigh");
    expect(options).not.toContain("Not set");
  });

  it("saves reasoning_effort=\"max\" when Max is selected", async () => {
    render(<ModelSelector {...baseProps({ currentThinkingMode: "thinking" })} />);
    await setReasoningEffortSelect("Max");
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "set_model_upstream",
        expect.objectContaining({ reasoningEffort: "max" }),
      );
    });
  });

  it("saves reasoning_effort=\"low\" when Low is selected", async () => {
    render(<ModelSelector {...baseProps({ currentThinkingMode: "thinking" })} />);
    await setReasoningEffortSelect("Low");
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "set_model_upstream",
        expect.objectContaining({ reasoningEffort: "low" }),
      );
    });
  });

  it("clears the effort to null when switching from Thinking to Normal", async () => {
    render(<ModelSelector {...baseProps({ currentThinkingMode: "thinking" })} />);
    // Start with Max selected.
    await setReasoningEffortSelect("Max");
    await waitFor(() => {
      expect(lastUpstreamSave()?.reasoningEffort).toBe("max");
    });

    // Switch Mode → Normal (mode select is the second combobox in the row).
    const flashRow = screen.getByText("Sonnet 5 →").closest("div") as HTMLElement;
    const modeSelect = within(flashRow).getAllByRole("combobox")[1] as HTMLElement;
    await act(async () => {
      await userEvent.selectOptions(modeSelect, "normal");
    });

    await waitFor(() => {
      expect(lastUpstreamSave()?.reasoningEffort).toBeNull();
      expect(lastUpstreamSave()?.thinkingMode).toBe("normal");
    });
  });

  it("defaults to High when enabling Thinking with no prior effort", async () => {
    render(<ModelSelector {...baseProps({ currentThinkingMode: "normal" })} />);
    const flashRow = screen.getByText("Sonnet 5 →").closest("div") as HTMLElement;
    const modeSelect = within(flashRow).getAllByRole("combobox")[1] as HTMLElement;
    await act(async () => {
      await userEvent.selectOptions(modeSelect, "thinking");
    });
    const effortSelect = within(flashRow).getByLabelText(/reasoning effort/i);
    expect((effortSelect as HTMLSelectElement).value).toBe("high");
  });

  it("normalizes a legacy medium to high when switching from Pro to Flash", async () => {
    render(
      <ModelSelector
        {...baseProps({
          currentUpstream: "deepseek-v4-pro",
          currentThinkingMode: "thinking",
          currentReasoningEffort: "medium",
        })}
      />,
    );
    const flashRow = screen.getByText("Sonnet 5 →").closest("div") as HTMLElement;
    const modelSelect = within(flashRow).getAllByRole("combobox")[0];
    await act(async () => {
      await userEvent.selectOptions(modelSelect, "deepseek-flash");
    });
    await waitFor(() => {
      expect(lastUpstreamSave()?.reasoningEffort).toBe("high");
    });
  });

  it("preserves low when switching from Pro to Flash", async () => {
    render(
      <ModelSelector
        {...baseProps({
          currentUpstream: "deepseek-v4-pro",
          currentThinkingMode: "thinking",
          currentReasoningEffort: "low",
        })}
      />,
    );
    const flashRow = screen.getByText("Sonnet 5 →").closest("div") as HTMLElement;
    const modelSelect = within(flashRow).getAllByRole("combobox")[0];
    await act(async () => {
      await userEvent.selectOptions(modelSelect, "deepseek-flash");
    });
    await waitFor(() => {
      expect(lastUpstreamSave()?.reasoningEffort).toBe("low");
    });
  });
});

describe("DeepSeek V4 Pro reasoning effort", () => {
  const proProps = (overrides: Partial<ModelSelectorProps> = {}) =>
    baseProps({
      currentUpstream: "deepseek-v4-pro",
      thinkingModePolicy: "toggleable",
      ...overrides,
    });

  function getEffortSelect() {
    const proRow = screen.getByText("Sonnet 5 →").closest("div") as HTMLElement;
    return within(proRow).getByLabelText(/reasoning effort/i) as HTMLSelectElement;
  }

  it("hides the effort selector in Normal mode", () => {
    render(<ModelSelector {...proProps({ currentThinkingMode: "normal" })} />);
    const proRow = screen.getByText("Sonnet 5 →").closest("div") as HTMLElement;
    expect(queryOptions(within(proRow).queryByLabelText(/reasoning effort/i))).toBeNull();
  });

  it("shows Low / High / Max options in Thinking mode", () => {
    render(<ModelSelector {...proProps({ currentThinkingMode: "thinking" })} />);
    const options = Array.from(getEffortSelect().querySelectorAll("option")).map((o) => o.textContent);
    expect(options).toEqual(["Low", "High", "Max"]);
    expect(options).not.toContain("Medium");
    expect(options).not.toContain("Not set");
  });

  it("saves reasoning_effort=\"high\" when High is selected", async () => {
    render(<ModelSelector {...proProps({ currentThinkingMode: "thinking" })} />);
    await act(async () => {
      await userEvent.selectOptions(getEffortSelect(), "high");
    });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "set_model_upstream",
        expect.objectContaining({ reasoningEffort: "high" }),
      );
    });
  });

  it("saves reasoning_effort=\"max\" when Max is selected", async () => {
    render(<ModelSelector {...proProps({ currentThinkingMode: "thinking" })} />);
    await act(async () => {
      await userEvent.selectOptions(getEffortSelect(), "max");
    });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "set_model_upstream",
        expect.objectContaining({ reasoningEffort: "max" }),
      );
    });
  });

  it("clears the effort to null when switching from Thinking to Normal", async () => {
    render(<ModelSelector {...proProps({ currentThinkingMode: "thinking" })} />);
    await act(async () => {
      await userEvent.selectOptions(getEffortSelect(), "max");
    });
    await waitFor(() => {
      expect(lastUpstreamSave()?.reasoningEffort).toBe("max");
    });

    const proRow = screen.getByText("Sonnet 5 →").closest("div") as HTMLElement;
    const modeSelect = within(proRow).getAllByRole("combobox")[1] as HTMLElement;
    await act(async () => {
      await userEvent.selectOptions(modeSelect, "normal");
    });

    await waitFor(() => {
      expect(lastUpstreamSave()?.reasoningEffort).toBeNull();
      expect(lastUpstreamSave()?.thinkingMode).toBe("normal");
    });
  });

  it("defaults to High when enabling Thinking with no prior effort", async () => {
    render(<ModelSelector {...proProps({ currentThinkingMode: "normal" })} />);
    const proRow = screen.getByText("Sonnet 5 →").closest("div") as HTMLElement;
    const modeSelect = within(proRow).getAllByRole("combobox")[1] as HTMLElement;
    await act(async () => {
      await userEvent.selectOptions(modeSelect, "thinking");
    });
    expect(getEffortSelect().value).toBe("high");
  });

  it("normalizes a legacy medium to high on display", () => {
    render(<ModelSelector {...proProps({ currentThinkingMode: "thinking", currentReasoningEffort: "medium" })} />);
    expect(getEffortSelect().value).toBe("high");
  });

  it("keeps low as-is on display", () => {
    render(<ModelSelector {...proProps({ currentThinkingMode: "thinking", currentReasoningEffort: "low" })} />);
    expect(getEffortSelect().value).toBe("low");
  });

  it("keeps an already-valid high / max value as-is on display", () => {
    const { unmount } = render(
      <ModelSelector {...proProps({ currentThinkingMode: "thinking", currentReasoningEffort: "max" })} />,
    );
    expect(getEffortSelect().value).toBe("max");
    unmount();
    render(<ModelSelector {...proProps({ currentThinkingMode: "thinking", currentReasoningEffort: "high" })} />);
    expect(getEffortSelect().value).toBe("high");
  });

  it("normalizes a legacy xhigh to high on display", () => {
    render(<ModelSelector {...proProps({ currentThinkingMode: "thinking", currentReasoningEffort: "xhigh" })} />);
    expect(getEffortSelect().value).toBe("high");
  });

  it("saves reasoning_effort=\"low\" when Low is selected", async () => {
    render(<ModelSelector {...proProps({ currentThinkingMode: "thinking" })} />);
    await act(async () => {
      await userEvent.selectOptions(getEffortSelect(), "low");
    });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "set_model_upstream",
        expect.objectContaining({ reasoningEffort: "low" }),
      );
    });
  });
});

describe("DeepSeek V4.1 Flash reasoning effort", () => {
  const flashProps = (overrides: Partial<ModelSelectorProps> = {}) =>
    baseProps({
      currentUpstream: "deepseek-flash",
      thinkingModePolicy: "toggleable",
      ...overrides,
    });

  function getEffortSelect() {
    const flashRow = screen.getByText("Sonnet 5 →").closest("div") as HTMLElement;
    return within(flashRow).getByLabelText(/reasoning effort/i) as HTMLSelectElement;
  }

  it("renders Low, High, Max options for deepseek-flash", () => {
    render(<ModelSelector {...flashProps({ currentThinkingMode: "thinking" })} />);
    const options = Array.from(getEffortSelect().querySelectorAll("option")).map((o) => o.textContent);
    expect(options).toEqual(["Low", "High", "Max"]);
  });

  it("saves reasoning_effort=\"low\" when Low is selected", async () => {
    render(<ModelSelector {...flashProps({ currentThinkingMode: "thinking" })} />);
    await act(async () => {
      await userEvent.selectOptions(getEffortSelect(), "low");
    });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "set_model_upstream",
        expect.objectContaining({ reasoningEffort: "low" }),
      );
    });
  });

  it("saves reasoning_effort=\"max\" when Max is selected", async () => {
    render(<ModelSelector {...flashProps({ currentThinkingMode: "thinking" })} />);
    await act(async () => {
      await userEvent.selectOptions(getEffortSelect(), "max");
    });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "set_model_upstream",
        expect.objectContaining({ reasoningEffort: "max" }),
      );
    });
  });

  it("clears the effort to null when switching from Thinking to Normal", async () => {
    render(<ModelSelector {...flashProps({ currentThinkingMode: "thinking" })} />);
    await act(async () => {
      await userEvent.selectOptions(getEffortSelect(), "max");
    });
    await waitFor(() => {
      expect(lastUpstreamSave()?.reasoningEffort).toBe("max");
    });

    const flashRow = screen.getByText("Sonnet 5 →").closest("div") as HTMLElement;
    const modeSelect = within(flashRow).getAllByRole("combobox")[1] as HTMLElement;
    await act(async () => {
      await userEvent.selectOptions(modeSelect, "normal");
    });

    await waitFor(() => {
      expect(lastUpstreamSave()?.reasoningEffort).toBeNull();
      expect(lastUpstreamSave()?.thinkingMode).toBe("normal");
    });
  });

  it("defaults to High when enabling Thinking with no prior effort", async () => {
    render(<ModelSelector {...flashProps({ currentThinkingMode: "normal", currentReasoningEffort: "" })} />);
    const flashRow = screen.getByText("Sonnet 5 →").closest("div") as HTMLElement;
    const modeSelect = within(flashRow).getAllByRole("combobox")[1] as HTMLElement;
    await act(async () => {
      await userEvent.selectOptions(modeSelect, "thinking");
    });

    await waitFor(() => {
      expect(lastUpstreamSave()?.reasoningEffort).toBe("high");
      expect(lastUpstreamSave()?.thinkingMode).toBe("thinking");
    });
  });

  it("preserves low when switching from Pro to Flash", () => {
    const { rerender } = render(
      <ModelSelector
        {...flashProps({
          currentUpstream: "deepseek-v4-pro",
          currentThinkingMode: "thinking",
          currentReasoningEffort: "low",
        })}
      />,
    );
    expect(getEffortSelect().value).toBe("low");

    rerender(
      <ModelSelector
        {...flashProps({
          currentUpstream: "deepseek-flash",
          currentThinkingMode: "thinking",
          currentReasoningEffort: "low",
        })}
      />,
    );
    expect(getEffortSelect().value).toBe("low");
  });

  it("normalizes a legacy medium to high when switching from Pro to Flash", () => {
    render(
      <ModelSelector
        {...flashProps({
          currentUpstream: "deepseek-flash",
          currentThinkingMode: "thinking",
          currentReasoningEffort: "medium",
        })}
      />,
    );
    expect(getEffortSelect().value).toBe("high");
  });
});

describe("Direct DeepSeek model dropdown options and legacy compatibility", () => {
  it("renders only DeepSeek V4.1 Flash and DeepSeek V4 Pro 0813 in normal dropdown", () => {
    const props = baseProps({
      providerId: "deepseek",
      modelKey: "claude-sonnet-5",
      currentUpstream: "deepseek-flash",
      thinkingModePolicy: "toggleable",
      currentThinkingMode: "thinking",
    });
    render(<ModelSelector {...props} />);
    const modelSelect = screen.getAllByRole("combobox")[0];
    const options = Array.from(modelSelect.querySelectorAll("option")).map((o) => ({
      value: o.value,
      label: o.textContent,
    }));
    expect(options).toEqual([
      { value: "deepseek-flash", label: "DeepSeek V4.1 Flash" },
      { value: "deepseek-v4-pro", label: "DeepSeek V4 Pro 0813" },
      { value: "__custom__", label: "apiKeyPanel.customModel" },
    ]);
    expect(options.some((o) => o.value === "deepseek-v4-flash")).toBe(false);
    expect(options.some((o) => o.value === "deepseek-v4-flash-vision-exp")).toBe(false);
  });

  it("loads legacy deepseek-v4-flash: preserves model ID, keeps toggleable thinking policy, and preserves reasoning effort", () => {
    const props = baseProps({
      providerId: "deepseek",
      modelKey: "claude-sonnet-5",
      currentUpstream: "deepseek-v4-flash",
      thinkingModePolicy: "toggleable",
      currentThinkingMode: "thinking",
      currentReasoningEffort: "high",
    });
    render(<ModelSelector {...props} />);
    const comboboxes = screen.getAllByRole("combobox");
    const modelSelect = comboboxes[0] as HTMLSelectElement;
    expect(modelSelect.value).toBe("deepseek-v4-flash");
    expect(screen.getByRole("option", { name: "deepseek-v4-flash (Legacy / saved)" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();

    // Thinking mode selector must be present (policy is toggleable, not unknown)
    const modeSelect = comboboxes[1] as HTMLSelectElement;
    expect(modeSelect).toBeInTheDocument();
    expect(modeSelect.value).toBe("thinking");

    // Reasoning effort selector must be present with Low/High/Max options from MODEL_CAPABILITIES
    const effortSelect = screen.getByLabelText(/reasoning effort/i) as HTMLSelectElement;
    expect(effortSelect).toBeInTheDocument();
    const effortOptions = Array.from(effortSelect.querySelectorAll("option")).map((o) => o.textContent);
    expect(effortOptions).toEqual(["Low", "High", "Max"]);
    expect(effortSelect.value).toBe("high");
  });

  it("loads legacy deepseek-v4-flash-vision-exp: preserves model ID, toggleable thinking policy, and reasoning effort", () => {
    const props = baseProps({
      providerId: "deepseek",
      modelKey: "claude-sonnet-5",
      currentUpstream: "deepseek-v4-flash-vision-exp",
      thinkingModePolicy: "toggleable",
      currentThinkingMode: "thinking",
      currentReasoningEffort: "max",
    });
    render(<ModelSelector {...props} />);
    const comboboxes = screen.getAllByRole("combobox");
    const modelSelect = comboboxes[0] as HTMLSelectElement;
    expect(modelSelect.value).toBe("deepseek-v4-flash-vision-exp");
    expect(screen.getByRole("option", { name: "deepseek-v4-flash-vision-exp (Legacy / saved)" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();

    // Thinking mode selector must be present (toggleable, not unknown)
    const modeSelect = comboboxes[1] as HTMLSelectElement;
    expect(modeSelect).toBeInTheDocument();

    // Reasoning effort selector must be present with Low/High/Max
    const effortSelect = screen.getByLabelText(/reasoning effort/i) as HTMLSelectElement;
    expect(effortSelect).toBeInTheDocument();
    expect(effortSelect.value).toBe("max");
  });

  it("handles genuine unknown custom model: uses unknown policy (no thinking/effort controls)", () => {
    const props = baseProps({
      providerId: "deepseek",
      modelKey: "claude-sonnet-5",
      currentUpstream: "my-custom-unregistered-model",
      thinkingModePolicy: "toggleable",
      currentThinkingMode: "normal",
    });
    render(<ModelSelector {...props} />);
    const comboboxes = screen.getAllByRole("combobox");
    expect(comboboxes).toHaveLength(1); // Only the model selector combobox, no thinking mode or effort select
    const customInput = screen.getByRole("textbox") as HTMLInputElement;
    expect(customInput.value).toBe("my-custom-unregistered-model");
    expect(screen.queryByLabelText(/reasoning effort/i)).not.toBeInTheDocument();
  });

  it("does not overwrite legacy model ID when saved without edits", async () => {
    const props = baseProps({
      providerId: "deepseek",
      modelKey: "claude-sonnet-5",
      currentUpstream: "deepseek-v4-flash",
      thinkingModePolicy: "toggleable",
      currentThinkingMode: "thinking",
      currentReasoningEffort: "low",
    });
    render(<ModelSelector {...props} />);
    const effortSelect = screen.getByLabelText(/reasoning effort/i) as HTMLSelectElement;
    await act(async () => {
      await userEvent.selectOptions(effortSelect, "max");
    });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "set_model_upstream",
        expect.objectContaining({
          upstreamModel: "deepseek-v4-flash",
          reasoningEffort: "max",
        }),
      );
    });
  });
});

describe("Direct MiMo ModelSelector and Legacy Compatibility", () => {
  it("renders saved legacy MiMo V2.5 Pro as explicit (Legacy / saved) option without custom textbox", () => {
    const props = baseProps({
      providerId: "mimo",
      modelKey: "claude-opus-5",
      currentUpstream: "mimo-v2.5-pro",
      thinkingModePolicy: "toggleable",
      currentThinkingMode: "thinking",
    });
    render(<ModelSelector {...props} />);
    const comboboxes = screen.getAllByRole("combobox");
    const modelSelect = comboboxes[0] as HTMLSelectElement;
    expect(modelSelect.value).toBe("mimo-v2.5-pro");
    expect(screen.getByRole("option", { name: "MiMo-V2.5-Pro (Legacy / saved)" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("renders saved legacy MiMo V2.5 as explicit (Legacy / saved) option without custom textbox", () => {
    const props = baseProps({
      providerId: "mimo",
      modelKey: "claude-haiku-4-5",
      currentUpstream: "mimo-v2.5",
      thinkingModePolicy: "toggleable",
      currentThinkingMode: "normal",
    });
    render(<ModelSelector {...props} />);
    const comboboxes = screen.getAllByRole("combobox");
    const modelSelect = comboboxes[0] as HTMLSelectElement;
    expect(modelSelect.value).toBe("mimo-v2.5");
    expect(screen.getByRole("option", { name: "MiMo-V2.5 (Legacy / saved)" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("renders saved legacy MiMo V2.5 Pro UltraSpeed as explicit (Legacy / saved) option without custom textbox", () => {
    const props = baseProps({
      providerId: "mimo",
      modelKey: "claude-sonnet-5",
      currentUpstream: "mimo-v2.5-pro-ultraspeed",
      thinkingModePolicy: "toggleable",
      currentThinkingMode: "thinking",
    });
    render(<ModelSelector {...props} />);
    const comboboxes = screen.getAllByRole("combobox");
    const modelSelect = comboboxes[0] as HTMLSelectElement;
    expect(modelSelect.value).toBe("mimo-v2.5-pro-ultraspeed");
    expect(screen.getByRole("option", { name: "MiMo-V2.5-Pro-UltraSpeed (Legacy / saved)" })).toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("renders genuinely unknown custom MiMo model with Custom... and custom text input", () => {
    const props = baseProps({
      providerId: "mimo",
      modelKey: "claude-sonnet-5",
      currentUpstream: "mimo-custom-experimental",
      thinkingModePolicy: "toggleable",
      currentThinkingMode: "normal",
    });
    render(<ModelSelector {...props} />);
    const comboboxes = screen.getAllByRole("combobox");
    const modelSelect = comboboxes[0] as HTMLSelectElement;
    expect(modelSelect.value).toBe("__custom__");
    const customInput = screen.getByRole("textbox") as HTMLInputElement;
    expect(customInput.value).toBe("mimo-custom-experimental");
  });
});

describe("Kimi Code ModelSelector", () => {
  it("renders only kimi-for-coding and kimi-for-coding-highspeed options", () => {
    const props = baseProps({
      providerId: "kimi-code",
      modelKey: "claude-sonnet-5",
      currentUpstream: "kimi-for-coding",
      thinkingModePolicy: "thinking_only",
      currentThinkingMode: "thinking_only",
      currentReasoningEffort: "",
    });
    render(<ModelSelector {...props} />);
    const modelSelect = screen.getByRole("combobox");
    const options = Array.from(modelSelect.querySelectorAll("option")).map((o) => o.value);
    expect(options).toEqual(["k3.1-pro", "k3.1-pro-highspeed", "kimi-for-coding", "kimi-for-coding-highspeed", "__custom__"]);
  });

  it("displays subscription and quota note in Kimi Code provider panel when expanded", async () => {
    const mockConfig: GatewayConfig = {
      port: 8080,
      host: "127.0.0.1",
      cors_origins: [],
      active_provider: "kimi-code",
      auto_compact_threshold_kb: 50,
      claude_desktop: { enabled: true },
      providers: {
        "kimi-code": {
          display_name: "Kimi Code",
          api_key_env: "KIMI_CODE_API_KEY",
          messages_url: "https://api.kimi.com/coding/v1/messages",
          supports_count_tokens: false,
          models: {
            "claude-opus-5": { upstream_model: "kimi-for-coding", thinking_mode: "thinking_only" },
            "claude-sonnet-5": { upstream_model: "kimi-for-coding", thinking_mode: "thinking_only" },
            "claude-haiku-4-5": { upstream_model: "kimi-for-coding-highspeed", thinking_mode: "thinking_only" },
          },
        },
      },
    } as unknown as GatewayConfig;

    render(
      <ApiKeyPanel
        config={mockConfig}
        refreshConfig={vi.fn().mockResolvedValue(undefined)}
        gatewayRunning={false}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    const kimiCodeHeader = screen.getByText("Kimi Code");
    await act(async () => {
      kimiCodeHeader.click();
    });

    expect(screen.getByText(/Kimi Code subscription/i)).toBeInTheDocument();
    expect(screen.getByText(/membership/i)).toBeInTheDocument();
  });

  it("renders providers in the order: DeepSeek, MiMo, GLM, MiniMax, Kimi, Kimi Code, OpenRouter regardless of object key order", () => {
    const mockConfig: GatewayConfig = {
      port: 8080,
      host: "127.0.0.1",
      cors_origins: [],
      active_provider: "deepseek",
      auto_compact_threshold_kb: 50,
      claude_desktop: { enabled: true },
      providers: {
        openrouter: { display_name: "OpenRouter", profiles: [] },
        "kimi-code": { display_name: "Kimi Code", models: {} },
        mimo: { display_name: "MiMo", models: {} },
        kimi: { display_name: "Kimi", models: {} },
        zai: { display_name: "GLM", models: {} },
        minimax: { display_name: "MiniMax", models: {} },
        deepseek: { display_name: "DeepSeek", models: {} },
      },
    } as unknown as GatewayConfig;

    render(
      <ApiKeyPanel
        config={mockConfig}
        refreshConfig={vi.fn().mockResolvedValue(undefined)}
        gatewayRunning={false}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    const apiKeysSection = screen.getByText("apiKeyPanel.header").closest(".settings-tile") as HTMLElement;
    const providerRowButtons = within(apiKeysSection).getAllByRole("button").filter((btn) => btn.getAttribute("aria-expanded") !== null);
    const providerNames = providerRowButtons.map((btn) => btn.querySelector("div:nth-child(2)")?.textContent?.trim());
    expect(providerNames).toEqual(["DeepSeek", "MiMo", "GLM", "MiniMax", "Kimi", "Kimi Code", "OpenRouter"]);
  });

  it("renders MiMo V2.6 models in correct order with friendly names and no effort selector", async () => {
    const mockConfig: GatewayConfig = {
      port: 8080,
      host: "127.0.0.1",
      cors_origins: [],
      active_provider: "mimo",
      auto_compact_threshold_kb: 50,
      claude_desktop: { enabled: true },
      providers: {
        mimo: {
          display_name: "MiMo",
          api_key_env: "XIAOMI_API_KEY",
          default_model: "mimo-v2.6-flash",
          models: {
            "claude-opus-5": {
              upstream_model: "mimo-v2.6-pro",
              thinking_mode: "thinking",
              visible: true,
            },
            "claude-sonnet-5": {
              upstream_model: "mimo-v2.6-flash",
              thinking_mode: "thinking",
              visible: true,
            },
            "claude-haiku-4-5": {
              upstream_model: "mimo-v2.6-flash",
              thinking_mode: "normal",
              visible: true,
            },
          },
        },
      },
    } as unknown as GatewayConfig;

    render(
      <ApiKeyPanel
        config={mockConfig}
        refreshConfig={vi.fn().mockResolvedValue(undefined)}
        gatewayRunning={false}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    // Expand MiMo provider accordion
    const mimoBtn = screen.getByRole("button", { name: /mimo/i });
    await userEvent.click(mimoBtn);

    // Verify model selectors have the 3 V2.6 models with friendly names
    const selects = screen.getAllByRole("combobox");
    const modelSelect = selects.find((s) => {
      const opts = Array.from(s.querySelectorAll("option")).map((o) => o.textContent);
      return opts.includes("MiMo-V2.6-Flash");
    });
    expect(modelSelect).toBeDefined();
    const values = Array.from(modelSelect!.querySelectorAll("option")).map((o) => o.value);
    expect(values).toEqual([
      "mimo-v2.6-flash",
      "mimo-v2.6-pro",
      "mimo-v2.6-pro-ultraspeed",
      "__custom__",
    ]);
    const labels = Array.from(modelSelect!.querySelectorAll("option")).map((o) => o.textContent);
    expect(labels).toEqual([
      "MiMo-V2.6-Flash",
      "MiMo-V2.6-Pro",
      "MiMo-V2.6-Pro-UltraSpeed",
      "apiKeyPanel.customModel",
    ]);

    // Verify no reasoning effort level selector (low/medium/high/max) is shown for MiMo
    expect(screen.queryByLabelText(/reasoning effort/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: /effort/i })).not.toBeInTheDocument();
  });

  it("renders Ollama Local as its own standalone settings card below API Keys", async () => {
    const mockConfig = {
      active_provider: "deepseek",
      providers: {
        deepseek: {
          display_name: "DeepSeek",
          api_key_env: "DEEPSEEK_API_KEY",
          hidden: false,
          models: {},
        },
      },
      claude_code: {
        third_party_provider: {
          show_on_dashboard: true,
          provider: "ollama",
          base_url: "http://127.0.0.1:11434",
          model: "mimo-v2.6-distill-qwen-9b",
          thinking_mode: "normal",
          supports_vision: false,
          context_window: 131072,
        },
      },
    } as unknown as GatewayConfig;

    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "list_ollama_models") return ["gemma4:latest", "llama3.3:70b"];
      if (cmd === "update_claude_code_third_party_settings") return { restartGateway: false, restartReason: "" };
      return null;
    });

    render(
      <ApiKeyPanel
        config={mockConfig}
        refreshConfig={vi.fn().mockResolvedValue(undefined)}
        gatewayRunning={false}
        restartGateway={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    // 1. Verify two separate section headings exist: API Keys and Ollama Local
    expect(screen.getByText("apiKeyPanel.header")).toBeInTheDocument();
    expect(screen.getByText("apiKeyPanel.ollamaLocal.title")).toBeInTheDocument();

    const headings = screen.getAllByRole("heading", { level: 3 });
    expect(headings).toHaveLength(2);
    expect(headings[0]).toHaveTextContent("apiKeyPanel.header");
    expect(headings[1]).toHaveTextContent("apiKeyPanel.ollamaLocal.title");

    const apiKeysSection = headings[0].closest(".settings-tile") as HTMLElement;
    expect(within(apiKeysSection).getByText("DeepSeek")).toBeInTheDocument();

    const ollamaCard = headings[1].closest(".settings-tile") as HTMLElement;

    // 2. Verify Show on Dashboard checkbox is present on the top-right of Ollama Local header
    const dashboardCheckbox = within(ollamaCard).getByRole("checkbox");
    expect(dashboardCheckbox).toBeInTheDocument();
    expect(dashboardCheckbox).toBeChecked();

    // 3. Verify Model selector, Refresh button, and Thinking dropdown are directly visible on one row
    const refreshBtn = within(ollamaCard).getByRole("button", { name: /apiKeyPanel\.ollamaLocal\.refresh/i });
    expect(refreshBtn).toBeInTheDocument();
    expect(within(ollamaCard).getByText("apiKeyPanel.ollamaLocal.modelTag")).toBeInTheDocument();
    expect(within(ollamaCard).getByText("apiKeyPanel.ollamaLocal.thinking")).toBeInTheDocument();

    // Total checkboxes on page: 1 in DeepSeek row, 1 in Ollama header = 2
    expect(screen.getAllByRole("checkbox")).toHaveLength(2);

    // 4. Verify model discovery refresh button works
    await userEvent.click(refreshBtn);
    expect(invokeMock).toHaveBeenCalledWith("list_ollama_models", { endpoint: "http://127.0.0.1:11434" });

    // 5. Expand Advanced settings accordion
    const advBtn = within(ollamaCard).getByRole("button", { name: /apiKeyPanel\.ollamaLocal\.advancedSettings/i });
    await userEvent.click(advBtn);

    // Verify endpoint, vision, context window
    expect(within(ollamaCard).getByDisplayValue("http://127.0.0.1:11434")).toBeInTheDocument();
    expect(within(ollamaCard).getByDisplayValue("131072")).toBeInTheDocument();
  });
});

// ── Direct GLM (zai): model and reasoning-effort controls ──────────

function glmConfig(
  models: Record<string, unknown>,
  zai?: Record<string, unknown>,
): GatewayConfig {
  return {
    port: 8080,
    host: "127.0.0.1",
    cors_origins: [],
    active_provider: "zai",
    auto_compact_threshold_kb: 50,
    claude_desktop: { enabled: true },
    providers: {
      zai: {
        display_name: "GLM",
        api_key_env: "ZAI_API_KEY",
        default_model: "glm-5.3",
        models,
        ...(zai ? { zai } : {}),
      },
    },
  } as unknown as GatewayConfig;
}

const GLM_ROUTE_MODELS = {
  "claude-opus-5": { upstream_model: "glm-5.3", thinking_mode: "thinking", reasoning_effort: "max", visible: true },
  "claude-sonnet-5": { upstream_model: "glm-5.3", thinking_mode: "thinking", reasoning_effort: "max", visible: true },
  "claude-haiku-4-5": { upstream_model: "glm-5.3", thinking_mode: "thinking", reasoning_effort: "max", visible: true },
};

async function renderExpandedGlm(
  models: Record<string, unknown>,
  zai?: Record<string, unknown>,
) {
  render(
    <ApiKeyPanel
      config={glmConfig(models, zai)}
      refreshConfig={vi.fn().mockResolvedValue(undefined)}
      gatewayRunning={false}
      restartGateway={vi.fn().mockResolvedValue(undefined)}
    />,
  );
  await userEvent.click(screen.getByRole("button", { name: /GLM/ }));
}

describe("Direct GLM (zai) provider controls", () => {
  it("shows the GLM provider name, the verified models, and no custom entry", async () => {
    await renderExpandedGlm(GLM_ROUTE_MODELS);

    // The provider row uses the config display name; the pre-rename label is gone.
    expect(screen.getByRole("button", { name: /GLM/ })).toBeInTheDocument();
    expect(screen.queryByText("Z.ai")).not.toBeInTheDocument();

    // With PAYG as default, the 4 documented models are offered — and no free-text entry.
    const modelSelects = screen
      .getAllByRole("combobox")
      .filter((select) => queryOptions(select)?.includes("GLM-5.3"));
    expect(modelSelects).toHaveLength(3);
    for (const select of modelSelects) {
      const options = select.querySelectorAll("option");
      expect(Array.from(options).map((o) => o.value)).toEqual([
        "glm-5.3",
        "glm-5.3-flash",
        "glm-5.3-flashx",
        "glm-5.2",
      ]);
      expect(Array.from(options).map((o) => o.textContent)).toEqual([
        "GLM-5.3",
        "GLM-5.3 Flash",
        "GLM-5.3 FlashX",
        "GLM-5.2",
      ]);
      expect(Array.from(options).some((o) => o.value === "__custom__")).toBe(false);
    }
    // No custom text input anywhere in the GLM panel.
    expect(screen.queryByPlaceholderText(/model name/i)).not.toBeInTheDocument();
  });

  it("shows a fixed Thinking mode and a selectable reasoning effort", async () => {
    await renderExpandedGlm(GLM_ROUTE_MODELS);

    // GLM-5.3 always thinks: the panel states it and offers no Normal/Thinking toggle.
    expect(screen.getAllByText("Thinking only")).toHaveLength(3);
    expect(screen.queryByText("Normal")).not.toBeInTheDocument();

    // The effort is selectable and saves per route.
    const effortSelects = screen
      .getAllByRole("combobox")
      .filter((select) => select.getAttribute("aria-label") === "Reasoning effort");
    expect(effortSelects).toHaveLength(3);
    for (const select of effortSelects) {
      expect(select).not.toBeDisabled();
      expect(Array.from(select.querySelectorAll("option")).map((o) => o.textContent)).toEqual([
        "Low",
        "High",
        "Max",
      ]);
      expect((select as HTMLSelectElement).value).toBe("max");
    }
    expect(screen.queryByText("apiKeyPanel.reasoningEffortNotSentYet")).toBeNull();
  });

  it("offers the 4 documented GLM API model candidates", async () => {
    await renderExpandedGlm(GLM_ROUTE_MODELS);
    const selects = screen
      .getAllByRole("combobox")
      .filter((select) => queryOptions(select)?.includes("GLM-5.3"));
    expect(selects).toHaveLength(3);
    for (const select of selects) {
      expect(Array.from(select.querySelectorAll("option")).map((o) => o.value)).toEqual([
        "glm-5.3",
        "glm-5.3-flash",
        "glm-5.3-flashx",
        "glm-5.2",
      ]);
    }
  });

  it("treats a saved FlashX as a normal candidate on the pay-as-you-go contract", async () => {
    await renderExpandedGlm(
      {
        "claude-opus-5": { upstream_model: "glm-5.3-flashx", thinking_mode: "thinking", reasoning_effort: "max", visible: true },
        "claude-sonnet-5": { upstream_model: "glm-5.3", thinking_mode: "thinking", reasoning_effort: "max", visible: true },
        "claude-haiku-4-5": { upstream_model: "glm-5.3", thinking_mode: "thinking", reasoning_effort: "max", visible: true },
      },
      { contract_product: "pay_as_you_go", protocol_mode: "openai_chat", contract_state: "unverified" },
    );

    const opusSelect = screen
      .getAllByRole("combobox")
      .filter((select) => queryOptions(select)?.some((text) => text.includes("GLM-5.3 FlashX")))[0] as HTMLSelectElement;
    expect(opusSelect).toBeDefined();
    expect(opusSelect.value).toBe("glm-5.3-flashx");
    // A documented candidate carries no legacy marker or reason note.
    const options = Array.from(opusSelect.querySelectorAll("option")).map((o) => o.textContent ?? "");
    expect(options.some((text) => text.includes("(Legacy / saved"))).toBe(false);
    expect(screen.queryByText("apiKeyPanel.glmModelUnavailableOnContract")).not.toBeInTheDocument();
    expect(
      invokeMock.mock.calls.filter((c: unknown[]) => c[0] === "set_model_upstream"),
    ).toHaveLength(0);
  });

  it("keeps an unknown saved model id verbatim on a GLM route", async () => {
    await renderExpandedGlm(
      {
        "claude-opus-5": { upstream_model: "glm-9.9-experimental", thinking_mode: "thinking", reasoning_effort: "max", visible: true },
        "claude-sonnet-5": { upstream_model: "glm-5.3", thinking_mode: "thinking", reasoning_effort: "max", visible: true },
        "claude-haiku-4-5": { upstream_model: "glm-5.3", thinking_mode: "thinking", reasoning_effort: "max", visible: true },
      },
      { contract_product: "pay_as_you_go", protocol_mode: "openai_chat", contract_state: "unverified" },
    );

    const opusSelect = screen
      .getAllByRole("combobox")
      .filter((select) => queryOptions(select)?.some((text) => text.includes("glm-9.9-experimental")))[0] as HTMLSelectElement;
    expect(opusSelect).toBeDefined();
    expect(opusSelect.value).toBe("glm-9.9-experimental");
    const options = Array.from(opusSelect.querySelectorAll("option")).map((o) => o.textContent ?? "");
    // An unknown id is a saved value, not a contract-incompatible documented
    // model: it keeps the plain legacy marker and is never rewritten.
    expect(options.some((text) => text.includes("(Legacy / saved)"))).toBe(true);
    expect(screen.queryByPlaceholderText(/model name/i)).not.toBeInTheDocument();
    expect(
      invokeMock.mock.calls.filter((c: unknown[]) => c[0] === "set_model_upstream"),
    ).toHaveLength(0);
  });

  it("offers the pay-as-you-go candidates by default on routes and does not display technical connection settings", async () => {
    await renderExpandedGlm(GLM_ROUTE_MODELS, { contract_product: "pay_as_you_go" });
    const modelSelects = screen
      .getAllByRole("combobox")
      .filter((select) => queryOptions(select)?.includes("GLM-5.3"));
    for (const select of modelSelects) {
      expect(Array.from(select.querySelectorAll("option")).map((o) => o.value)).toEqual([
        "glm-5.3",
        "glm-5.3-flash",
        "glm-5.3-flashx",
        "glm-5.2",
      ]);
    }

    // Normal view does not clutter with technical connection mode, notices, or accordions
    expect(screen.queryByText("apiKeyPanel.glmConnectionMode:")).not.toBeInTheDocument();
    expect(screen.queryByText("apiKeyPanel.glmPayAsYouGoOpenAi")).not.toBeInTheDocument();
    expect(screen.queryByText("apiKeyPanel.glmDispatchImplementedNotice")).not.toBeInTheDocument();
    expect(screen.queryByRole("checkbox", { name: /apiKeyPanel\.glmWarningAck/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /apiKeyPanel\.glmAdvancedSettingsShow/ })).not.toBeInTheDocument();
    expect(screen.queryByText("apiKeyPanel.glmCodingPlanNotSupportedNotice")).not.toBeInTheDocument();
  });

  it("offers a thinking toggle for GLM-5.2 and hides its effort while thinking is off", async () => {
    await renderExpandedGlm(
      {
        "claude-opus-5": { upstream_model: "glm-5.2", thinking_mode: "normal", reasoning_effort: "max", visible: true },
        "claude-sonnet-5": { upstream_model: "glm-5.3", thinking_mode: "thinking", reasoning_effort: "max", visible: true },
        "claude-haiku-4-5": { upstream_model: "glm-5.3", thinking_mode: "thinking", reasoning_effort: "max", visible: true },
      },
      { contract_product: "pay_as_you_go" },
    );

    // GLM-5.2 is the one documented model whose thinking may be turned off, so
    // it — and only it — renders the Normal/Thinking toggle instead of the
    // fixed "Thinking only" label the 5.3 routes show.
    const isThinkingToggle = (select: HTMLElement) =>
      Array.from(select.querySelectorAll("option"), (o) => o.textContent).join("|") ===
      "Normal|Thinking";
    const toggles = screen.getAllByRole("combobox").filter(isThinkingToggle);
    expect(toggles).toHaveLength(1);
    expect((toggles[0] as HTMLSelectElement).value).toBe("normal");

    // Thinking off means no effort axis: only the two 5.3 routes keep a selector.
    const effortSelects = () =>
      screen
        .getAllByRole("combobox")
        .filter((select) => select.getAttribute("aria-label") === "Reasoning effort");
    expect(effortSelects()).toHaveLength(2);

    // Turning thinking on keeps the stored effort and saves the canonical pair.
    await act(async () => {
      await userEvent.selectOptions(toggles[0], "thinking");
    });
    const save = invokeMock.mock.calls.find((c: unknown[]) => c[0] === "set_model_upstream");
    expect(save?.[1]).toMatchObject({
      modelKey: "claude-opus-5",
      upstreamModel: "glm-5.2",
      thinkingMode: "thinking",
      reasoningEffort: "max",
    });
    expect(effortSelects()).toHaveLength(3);
  });

  it("saves the canonical thinking value for a GLM route model change", async () => {
    await renderExpandedGlm(GLM_ROUTE_MODELS, { contract_product: "pay_as_you_go" });

    const modelSelects = screen
      .getAllByRole("combobox")
      .filter((select) => queryOptions(select)?.includes("GLM-5.3"));
    await act(async () => {
      await userEvent.selectOptions(modelSelects[0], "glm-5.3-flash");
    });

    const save = invokeMock.mock.calls.find((c: unknown[]) => c[0] === "set_model_upstream");
    expect(save?.[1]).toMatchObject({
      upstreamModel: "glm-5.3-flash",
      thinkingMode: "thinking",
    });
    // The catalog capability expression is never persisted.
    expect((save?.[1] as Record<string, unknown>).thinkingMode).not.toBe("thinking_only");
  });

  it("uses the localized GLM key placeholder and saves the API key using provider env_var fallback", async () => {
    await renderExpandedGlm(GLM_ROUTE_MODELS);

    const input = screen.getByPlaceholderText("apiKeyPanel.glmKeyPlaceholder");
    expect(input).toBeInTheDocument();

    await act(async () => {
      await userEvent.type(input, "test-glm-key-1234");
    });

    const saveBtn = screen.getByRole("button", { name: "apiKeyPanel.saveKey" });
    await act(async () => {
      await userEvent.click(saveBtn);
    });

    const saveCall = invokeMock.mock.calls.find((c: unknown[]) => c[0] === "set_env_api_key");
    expect(saveCall).toBeDefined();
    expect(saveCall?.[1]).toEqual({
      key: "test-glm-key-1234",
      envVarName: "ZAI_API_KEY",
    });
  });
});

const ANTHROPIC_ROUTE_MODELS = {
  "claude-opus-5": { upstream_model: "claude-opus-5-5", thinking_mode: "thinking", reasoning_effort: "high", visible: true },
  "claude-sonnet-5": { upstream_model: "claude-sonnet-5-5", thinking_mode: "thinking", reasoning_effort: "high", visible: true },
  "claude-haiku-4-5": { upstream_model: "claude-haiku-5-5", thinking_mode: "thinking", reasoning_effort: "high", visible: true },
};

function anthropicConfig(models: Record<string, unknown>): GatewayConfig {
  return {
    config_version: "1.0",
    active_provider: "anthropic",
    active_openrouter_profile_id: null,
    providers: {
      anthropic: {
        display_name: "Anthropic",
        upstream_url: "https://api.anthropic.com",
        api_key_env: "ANTHROPIC_API_KEY",
        default_model: "claude-opus-5-5",
        force_anthropic_version: null,
        supports_count_tokens: true,
        supports_vision: true,
        supports_video: false,
        supports_thinking: true,
        visible_models: ["claude-opus-5", "claude-sonnet-5", "claude-haiku-4-5"],
        models,
      },
    },
    server: { host: "127.0.0.1", port: 4000, enable_cors: false },
    non_vision_image_policy: "replace",
    normalize_response_model_identity: true,
  } as unknown as GatewayConfig;
}

async function renderExpandedAnthropic(models: Record<string, unknown>) {
  render(
    <ApiKeyPanel
      config={anthropicConfig(models)}
      refreshConfig={vi.fn().mockResolvedValue(undefined)}
      gatewayRunning={false}
      restartGateway={vi.fn().mockResolvedValue(undefined)}
    />,
  );
  await userEvent.click(screen.getByRole("button", { name: /Anthropic/ }));
}

describe("Direct Anthropic provider controls", () => {
  it("shows the Anthropic provider name, allowlisted models, and no custom entry", async () => {
    await renderExpandedAnthropic(ANTHROPIC_ROUTE_MODELS);

    expect(screen.getByRole("button", { name: /Anthropic/ })).toBeInTheDocument();

    const modelSelects = screen
      .getAllByRole("combobox")
      .filter((select) => queryOptions(select)?.includes("Claude Opus 5.5"));
    expect(modelSelects).toHaveLength(3);
    for (const select of modelSelects) {
      const options = select.querySelectorAll("option");
      expect(Array.from(options).map((o) => o.value)).toEqual([
        "claude-opus-5-5",
        "claude-sonnet-5-5",
        "claude-haiku-5-5",
      ]);
      expect(Array.from(options).some((o) => o.value === "__custom__")).toBe(false);
    }
    expect(screen.queryByPlaceholderText(/model name/i)).not.toBeInTheDocument();
  });

  it("saves the API key using ANTHROPIC_API_KEY env_var", async () => {
    await renderExpandedAnthropic(ANTHROPIC_ROUTE_MODELS);

    const input = screen.getByPlaceholderText("sk-...");
    expect(input).toBeInTheDocument();

    await act(async () => {
      await userEvent.type(input, "sk-ant-test-key-9999");
    });

    const saveBtn = screen.getByRole("button", { name: "apiKeyPanel.saveKey" });
    await act(async () => {
      await userEvent.click(saveBtn);
    });

    const saveCall = invokeMock.mock.calls.find((c: unknown[]) => c[0] === "set_env_api_key");
    expect(saveCall).toBeDefined();
    expect(saveCall?.[1]).toEqual({
      key: "sk-ant-test-key-9999",
      envVarName: "ANTHROPIC_API_KEY",
    });
  });
});

// ── Independent fixture for OpenAI reasoning models (§13 & §26) ────
// Declared explicitly without deriving from MODEL_CAPABILITIES to ensure
// independent verification of UI rendered options, defaults, and persistence.
interface ExpectedReasoningModelFixture {
  readonly id: string;
  readonly expectedOptions: readonly string[];
  readonly defaultEffort: string;
  readonly supportsNone: boolean;
}

const INDEPENDENT_OPENAI_REASONING_FIXTURE: readonly ExpectedReasoningModelFixture[] = [
  // Group 1: 5 choices, no "none"
  {
    id: "openai/gpt-6.1-sol",
    expectedOptions: ["low", "medium", "high", "xhigh", "max"],
    defaultEffort: "medium",
    supportsNone: false,
  },
  {
    id: "openai/gpt-6.1-sol-pro",
    expectedOptions: ["low", "medium", "high", "xhigh", "max"],
    defaultEffort: "medium",
    supportsNone: false,
  },
  {
    id: "openai/gpt-6-astra",
    expectedOptions: ["low", "medium", "high", "xhigh", "max"],
    defaultEffort: "medium",
    supportsNone: false,
  },
  {
    id: "openai/gpt-6-astra-pro",
    expectedOptions: ["low", "medium", "high", "xhigh", "max"],
    defaultEffort: "medium",
    supportsNone: false,
  },
  {
    id: "openai/gpt-astra-latest",
    expectedOptions: ["low", "medium", "high", "xhigh", "max"],
    defaultEffort: "medium",
    supportsNone: false,
  },
  // Group 2: 6 choices, includes "none"
  {
    id: "openai/gpt-6-sol",
    expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
    defaultEffort: "medium",
    supportsNone: true,
  },
  {
    id: "openai/gpt-6-sol-pro",
    expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
    defaultEffort: "medium",
    supportsNone: true,
  },
  {
    id: "openai/gpt-6-luna",
    expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
    defaultEffort: "medium",
    supportsNone: true,
  },
  {
    id: "openai/gpt-6-luna-pro",
    expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
    defaultEffort: "medium",
    supportsNone: true,
  },
  {
    id: "openai/gpt-5.6-sol",
    expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
    defaultEffort: "medium",
    supportsNone: true,
  },
  {
    id: "openai/gpt-5.6-sol-pro",
    expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
    defaultEffort: "medium",
    supportsNone: true,
  },
  {
    id: "openai/gpt-5.6-terra",
    expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
    defaultEffort: "medium",
    supportsNone: true,
  },
  {
    id: "openai/gpt-5.6-terra-pro",
    expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
    defaultEffort: "medium",
    supportsNone: true,
  },
  {
    id: "openai/gpt-5.6-luna",
    expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
    defaultEffort: "medium",
    supportsNone: true,
  },
  {
    id: "openai/gpt-5.6-luna-pro",
    expectedOptions: ["none", "low", "medium", "high", "xhigh", "max"],
    defaultEffort: "medium",
    supportsNone: true,
  },
] as const;

describe("OpenAI GPT-6.1 Sol and Sol Pro reasoning controls (§11 & §13)", () => {
  it("shared catalog declares reasoning metadata for all 15 OpenAI models (§13)", () => {
    const all15Ids = [
      "openai/gpt-6.1-sol",
      "openai/gpt-6.1-sol-pro",
      "openai/gpt-6-sol",
      "openai/gpt-6-sol-pro",
      "openai/gpt-6-luna",
      "openai/gpt-6-luna-pro",
      "openai/gpt-6-astra",
      "openai/gpt-6-astra-pro",
      "openai/gpt-astra-latest",
      "openai/gpt-5.6-sol",
      "openai/gpt-5.6-sol-pro",
      "openai/gpt-5.6-terra",
      "openai/gpt-5.6-terra-pro",
      "openai/gpt-5.6-luna",
      "openai/gpt-5.6-luna-pro",
    ];
    expect(all15Ids).toHaveLength(15);

    for (const id of all15Ids) {
      const caps = MODEL_CAPABILITIES[id];
      expect(caps).toBeDefined();
      expect(caps.supportsReasoningEffort).toBe(true);
      expect(caps.reasoning).toBeDefined();
      expect(caps.reasoning?.defaultEffort).toBe("medium");
      expect(caps.reasoningEffortOptions).toEqual(caps.reasoning?.supportedEfforts);

      if (id.includes("gpt-6.1") || id.includes("astra")) {
        expect(caps.reasoningEffortOptions).toEqual(["low", "medium", "high", "xhigh", "max"]);
        expect(caps.reasoningEffortOptions).not.toContain("none");
      } else {
        expect(caps.reasoningEffortOptions).toEqual(["none", "low", "medium", "high", "xhigh", "max"]);
        expect(caps.reasoningEffortOptions).toContain("none");
      }
    }
  });

  it("renders 5 reasoning effort choices and thinking only (no normal toggle) for GPT-6.1 Sol, defaulting to medium", async () => {
    const props = baseProps({
      providerId: "openrouter",
      modelKey: "claude-opus-5",
      gatewayModelLabel: "Opus 5 →",
      currentUpstream: "openai/gpt-6.1-sol",
      thinkingModePolicy: "thinking_only",
      currentThinkingMode: "thinking",
      currentReasoningEffort: "",
    });
    render(<ModelSelector {...props} />);

    // Thinking only indicator is shown
    expect(screen.getByText("Thinking only")).toBeInTheDocument();

    // No Normal/Thinking toggle combobox exists
    const comboboxes = screen.getAllByRole("combobox");
    // Only the model selector and the reasoning effort selector should exist
    expect(comboboxes).toHaveLength(2);

    const effortSelect = screen.getByLabelText(/reasoning effort/i) as HTMLSelectElement;
    expect(effortSelect).toBeInTheDocument();
    expect(effortSelect.value).toBe("medium");

    const optionValues = Array.from(effortSelect.querySelectorAll("option")).map((o) => o.value);
    expect(optionValues).toEqual(["low", "medium", "high", "xhigh", "max"]);
    expect(optionValues).not.toContain("normal");
    expect(optionValues).not.toContain("off");
    expect(optionValues).not.toContain("none");
    expect(optionValues).not.toContain("minimal");
  });

  it("handles legacy normal or invalid effort by falling back to medium on display without overwriting", () => {
    const props = baseProps({
      providerId: "openrouter",
      modelKey: "claude-opus-5",
      gatewayModelLabel: "Opus 5 →",
      currentUpstream: "openai/gpt-6.1-sol",
      thinkingModePolicy: "thinking_only",
      currentThinkingMode: "normal",
      currentReasoningEffort: "none",
    });
    render(<ModelSelector {...props} />);

    const effortSelect = screen.getByLabelText(/reasoning effort/i) as HTMLSelectElement;
    expect(effortSelect.value).toBe("medium");
    expect(invokeMock).not.toHaveBeenCalledWith("set_model_upstream", expect.anything());
  });

  it("reloads and saves reasoning effort for GPT-6.1 Sol regular and Sol Pro", async () => {
    const props = baseProps({
      providerId: "openrouter",
      modelKey: "claude-opus-5",
      gatewayModelLabel: "Opus 5 →",
      currentUpstream: "openai/gpt-6.1-sol",
      thinkingModePolicy: "thinking_only",
      currentThinkingMode: "thinking",
      currentReasoningEffort: "high",
    });
    const { unmount } = render(<ModelSelector {...props} />);

    let effortSelect = screen.getByLabelText(/reasoning effort/i) as HTMLSelectElement;
    expect(effortSelect.value).toBe("high");

    await act(async () => {
      await userEvent.selectOptions(effortSelect, "xhigh");
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "set_model_upstream",
        expect.objectContaining({
          upstreamModel: "openai/gpt-6.1-sol",
          thinkingMode: "thinking",
          reasoningEffort: "xhigh",
        }),
      );
    });

    unmount();

    // Now test GPT-6.1 Sol Pro preserves model ID and saves correctly
    const proProps = baseProps({
      providerId: "openrouter",
      modelKey: "claude-opus-5",
      gatewayModelLabel: "Opus 5 →",
      currentUpstream: "openai/gpt-6.1-sol-pro",
      thinkingModePolicy: "thinking_only",
      currentThinkingMode: "thinking",
      currentReasoningEffort: "max",
    });
    render(<ModelSelector {...proProps} />);

    effortSelect = screen.getByLabelText(/reasoning effort/i) as HTMLSelectElement;
    expect(effortSelect.value).toBe("max");

    await act(async () => {
      await userEvent.selectOptions(effortSelect, "low");
    });

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith(
        "set_model_upstream",
        expect.objectContaining({
          upstreamModel: "openai/gpt-6.1-sol-pro",
          thinkingMode: "thinking",
          reasoningEffort: "low",
        }),
      );
    });
  });

  describe("Table-driven actual UI rendering and persistence across all 15 models (§26)", () => {
    it.each(INDEPENDENT_OPENAI_REASONING_FIXTURE)(
      "renders actual effort selector with independent options, medium default, and persists valid effort for $id (§26)",
      async (fixture) => {
        const props = baseProps({
          providerId: "openrouter",
          modelKey: "claude-opus-5",
          gatewayModelLabel: "Opus 5 →",
          currentUpstream: fixture.id,
          thinkingModePolicy: "thinking_only",
          currentThinkingMode: "thinking",
          currentReasoningEffort: "",
        });
        const { unmount } = render(<ModelSelector {...props} />);

        // 1. The selected model is that exact ID and its effort selector is actually present.
        const modelSelect = screen.getAllByRole("combobox")[0] as HTMLSelectElement;
        expect(modelSelect.value).toBe(fixture.id);

        const effortSelect = screen.getByLabelText(/reasoning effort/i) as HTMLSelectElement;
        expect(effortSelect).toBeInTheDocument();

        // 2. The rendered option values exactly equal the independent expected matrix in §13.2,
        // including none only for models whose matrix includes it; no normal, off, or minimal option leaks.
        const optionValues = Array.from(effortSelect.querySelectorAll("option")).map((o) => o.value);
        expect(optionValues).toEqual(fixture.expectedOptions);
        expect(optionValues).not.toContain("normal");
        expect(optionValues).not.toContain("off");
        expect(optionValues).not.toContain("minimal");
        if (fixture.supportsNone) {
          expect(optionValues).toContain("none");
        } else {
          expect(optionValues).not.toContain("none");
        }

        // 3. With no usable persisted effort, the effective selected default is medium.
        expect(effortSelect.value).toBe(fixture.defaultEffort);

        // 4. Changing to a valid effort persists the chosen value while retaining the exact model ID.
        const targetEffort = "high";
        await act(async () => {
          await userEvent.selectOptions(effortSelect, targetEffort);
        });

        await waitFor(() => {
          expect(invokeMock).toHaveBeenCalledWith(
            "set_model_upstream",
            expect.objectContaining({
              upstreamModel: fixture.id,
              thinkingMode: "thinking",
              reasoningEffort: targetEffort,
            }),
          );
        });

        unmount();
      },
    );

    it.each(INDEPENDENT_OPENAI_REASONING_FIXTURE)(
      "handles legacy normal/off boundary on render without persisting for $id (§26 & §27)",
      (fixture) => {
        for (const legacyEffort of ["normal", "off"]) {
          const props = baseProps({
            providerId: "openrouter",
            modelKey: "claude-opus-5",
            gatewayModelLabel: "Opus 5 →",
            currentUpstream: fixture.id,
            thinkingModePolicy: "thinking_only",
            currentThinkingMode: "normal",
            currentReasoningEffort: legacyEffort,
          });
          const { unmount } = render(<ModelSelector {...props} />);

          const effortSelect = screen.getByLabelText(/reasoning effort/i) as HTMLSelectElement;
          const expectedBoundaryEffort = fixture.supportsNone ? "none" : "medium";
          expect(effortSelect.value).toBe(expectedBoundaryEffort);
          expect(invokeMock).not.toHaveBeenCalledWith("set_model_upstream", expect.anything());

          unmount();
        }
      },
    );
  });

  describe("None-supporting OpenAI models persisted load and explicit selection (§27)", () => {
    const NONE_SUPPORTING_FIXTURES = INDEPENDENT_OPENAI_REASONING_FIXTURE.filter((f) => f.supportsNone);

    it.each(NONE_SUPPORTING_FIXTURES)(
      "preserves persisted 'none' on load without writing and persists 'none' when selected for $id (§27)",
      async (fixture) => {
        // 1. Persisted 'none' is selected on load/re-render without an automatic write
        const props = baseProps({
          providerId: "openrouter",
          modelKey: "claude-opus-5",
          gatewayModelLabel: "Opus 5 →",
          currentUpstream: fixture.id,
          thinkingModePolicy: "thinking_only",
          currentThinkingMode: "thinking",
          currentReasoningEffort: "none",
        });
        const { unmount } = render(<ModelSelector {...props} />);

        const effortSelect = screen.getByLabelText(/reasoning effort/i) as HTMLSelectElement;
        expect(effortSelect.value).toBe("none");
        expect(invokeMock).not.toHaveBeenCalledWith("set_model_upstream", expect.anything());

        unmount();

        // 2. Selecting 'none' from a different effort persists the exact model ID plus 'none'
        const initialProps = baseProps({
          providerId: "openrouter",
          modelKey: "claude-opus-5",
          gatewayModelLabel: "Opus 5 →",
          currentUpstream: fixture.id,
          thinkingModePolicy: "thinking_only",
          currentThinkingMode: "thinking",
          currentReasoningEffort: "high",
        });
        render(<ModelSelector {...initialProps} />);

        const selectToChange = screen.getByLabelText(/reasoning effort/i) as HTMLSelectElement;
        expect(selectToChange.value).toBe("high");

        await act(async () => {
          await userEvent.selectOptions(selectToChange, "none");
        });

        await waitFor(() => {
          expect(invokeMock).toHaveBeenCalledWith(
            "set_model_upstream",
            expect.objectContaining({
              upstreamModel: fixture.id,
              thinkingMode: "thinking",
              reasoningEffort: "none",
            }),
          );
        });
      },
    );
  });

  describe("Table-driven model-switch coverage for regular/-pro pairs and fallback (§27)", () => {
    interface RegularProPair {
      readonly regularId: string;
      readonly proId: string;
    }

    const REGULAR_PRO_NONE_PAIRS: readonly RegularProPair[] = [
      { regularId: "openai/gpt-6-sol", proId: "openai/gpt-6-sol-pro" },
      { regularId: "openai/gpt-6-luna", proId: "openai/gpt-6-luna-pro" },
      { regularId: "openai/gpt-5.6-sol", proId: "openai/gpt-5.6-sol-pro" },
      { regularId: "openai/gpt-5.6-terra", proId: "openai/gpt-5.6-terra-pro" },
      { regularId: "openai/gpt-5.6-luna", proId: "openai/gpt-5.6-luna-pro" },
    ];

    it.each(REGULAR_PRO_NONE_PAIRS)(
      "preserves 'none' when switching regular -> pro and pro -> regular for $regularId <-> $proId (§27)",
      async ({ regularId, proId }) => {
        // regular -> pro
        {
          const props = baseProps({
            providerId: "openrouter",
            modelKey: "claude-opus-5",
            gatewayModelLabel: "Opus 5 →",
            currentUpstream: regularId,
            thinkingModePolicy: "thinking_only",
            currentThinkingMode: "thinking",
            currentReasoningEffort: "none",
          });
          const { unmount } = render(<ModelSelector {...props} />);

          const modelSelect = screen.getAllByRole("combobox")[0] as HTMLSelectElement;
          expect(modelSelect.value).toBe(regularId);

          await act(async () => {
            await userEvent.selectOptions(modelSelect, proId);
          });

          await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith(
              "set_model_upstream",
              expect.objectContaining({
                upstreamModel: proId,
                thinkingMode: "thinking",
                reasoningEffort: "none",
              }),
            );
          });

          const effortSelect = screen.getByLabelText(/reasoning effort/i) as HTMLSelectElement;
          expect(effortSelect.value).toBe("none");

          unmount();
        }

        // pro -> regular
        {
          invokeMock.mockReset();
          resetInvoke();
          const props = baseProps({
            providerId: "openrouter",
            modelKey: "claude-opus-5",
            gatewayModelLabel: "Opus 5 →",
            currentUpstream: proId,
            thinkingModePolicy: "thinking_only",
            currentThinkingMode: "thinking",
            currentReasoningEffort: "none",
          });
          const { unmount } = render(<ModelSelector {...props} />);

          const modelSelect = screen.getAllByRole("combobox")[0] as HTMLSelectElement;
          expect(modelSelect.value).toBe(proId);

          await act(async () => {
            await userEvent.selectOptions(modelSelect, regularId);
          });

          await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith(
              "set_model_upstream",
              expect.objectContaining({
                upstreamModel: regularId,
                thinkingMode: "thinking",
                reasoningEffort: "none",
              }),
            );
          });

          const effortSelect = screen.getByLabelText(/reasoning effort/i) as HTMLSelectElement;
          expect(effortSelect.value).toBe("none");

          unmount();
        }
      },
    );

    const NON_NONE_DESTINATIONS = [
      "openai/gpt-6.1-sol",
      "openai/gpt-6.1-sol-pro",
      "openai/gpt-6-astra",
      "openai/gpt-6-astra-pro",
      "openai/gpt-astra-latest",
    ];

    it.each(REGULAR_PRO_NONE_PAIRS)(
      "falls back to medium and never serializes none when switching from $regularId to non-none destinations (§27)",
      async ({ regularId }) => {
        for (const destId of NON_NONE_DESTINATIONS) {
          invokeMock.mockReset();
          resetInvoke();
          const props = baseProps({
            providerId: "openrouter",
            modelKey: "claude-opus-5",
            gatewayModelLabel: "Opus 5 →",
            currentUpstream: regularId,
            thinkingModePolicy: "thinking_only",
            currentThinkingMode: "thinking",
            currentReasoningEffort: "none",
          });
          const { unmount } = render(<ModelSelector {...props} />);

          const modelSelect = screen.getAllByRole("combobox")[0] as HTMLSelectElement;
          expect(modelSelect.value).toBe(regularId);

          await act(async () => {
            await userEvent.selectOptions(modelSelect, destId);
          });

          await waitFor(() => {
            expect(invokeMock).toHaveBeenCalledWith(
              "set_model_upstream",
              expect.objectContaining({
                upstreamModel: destId,
                thinkingMode: "thinking",
                reasoningEffort: "medium",
              }),
            );
          });

          const effortSelect = screen.getByLabelText(/reasoning effort/i) as HTMLSelectElement;
          expect(effortSelect.value).toBe("medium");
          expect(effortSelect.value).not.toBe("none");

          unmount();
        }
      },
    );
  });
});

// Returns option texts from a select element, or null when absent.
function queryOptions(select: HTMLElement | null): string[] | null {
  if (!select) return null;
  return Array.from(select.querySelectorAll("option")).map((o) => o.textContent ?? "");
}
