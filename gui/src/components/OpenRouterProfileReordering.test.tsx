import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import OpenRouterProviderSection, { displayModelSetName } from "./OpenRouterProviderSection";
import type { ProviderConfig, OpenRouterProfile } from "../types";

function makeProfile(
  id: string,
  display_name: string,
  hidden = false,
): OpenRouterProfile {
  return {
    id,
    display_name,
    hidden,
    model_map: {},
    visible_models: [],
    models: {},
  };
}

const provider: ProviderConfig = {
  display_name: "OpenRouter",
  upstream_url: "https://openrouter.ai/api/v1",
  api_key_env: "OPENROUTER_API_KEY",
  default_model: "openrouter/auto",
  force_anthropic_version: null,
  supports_count_tokens: false,
  supports_vision: false,
  supports_video: false,
  supports_thinking: false,
  model_map: {},
  visible_models: [],
  models: {},
  profiles: [],
};

const profiles: OpenRouterProfile[] = [
  makeProfile("uuid-1", "Model 1"),
  makeProfile("uuid-2", "Model 2"),
  makeProfile("uuid-3", "OpenRouter: DeepSeek"),
];

const noop = async () => {};

describe("OpenRouterProfileReordering", () => {
  it("preserves profile.display_name verbatim as the source of truth without prepending provider name", () => {
    expect(displayModelSetName(makeProfile("1", "Model 1"))).toBe("Model 1");
    expect(displayModelSetName(makeProfile("2", "Model 2"))).toBe("Model 2");
    expect(displayModelSetName(makeProfile("3", "OpenRouter: DeepSeek"))).toBe("OpenRouter: DeepSeek");
    expect(displayModelSetName(makeProfile("4", "Custom Workspace"))).toBe("Custom Workspace");
  });

  it("renders one accessible drag handle per profile and displays clean model set names", async () => {
    render(
      <OpenRouterProviderSection
        providerId="openrouter"
        provider={provider}
        profiles={profiles}
        activeProfileId={null}
        keyStatus={null}
        allKeyStatusLoading={false}
        gatewayRunning={false}
        refreshConfig={noop}
        restartGateway={noop}
        refreshKeyStatus={noop}
        onAddModelSet={noop}
        addError={null}
      />,
    );

    // Expand the accordion by clicking the header row
    const accordionHeader = screen.getByText("OpenRouter");
    await userEvent.click(accordionHeader);

    // One drag handle button per profile, each with the correct aria-label
    expect(
      screen.getAllByRole("button", { name: "Drag to reorder" }),
    ).toHaveLength(profiles.length);

    // Model set headers must display the saved display_name verbatim
    expect(screen.getByText("Model 1")).toBeInTheDocument();
    expect(screen.getByText("Model 2")).toBeInTheDocument();
    expect(screen.getByText("OpenRouter: DeepSeek")).toBeInTheDocument();

    // Must NOT prepend "OpenRouter " to generic "Model N" presets
    expect(screen.queryByText("OpenRouter Model 1")).not.toBeInTheDocument();
    expect(screen.queryByText("OpenRouter モデル 1")).not.toBeInTheDocument();
  });
});
