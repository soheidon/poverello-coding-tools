import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import OpenRouterModelSetCard from "./OpenRouterModelSetCard";
import type { OpenRouterProfile, ProviderConfig } from "../types";

vi.mock("./OpenRouterModelSelector", () => ({
  default: () => null,
}));

const invokeMock = vi.mocked(invoke);
const confirmMock = vi.fn();

const profile: OpenRouterProfile = {
  id: "user-model-set-1",
  display_name: "Model 1",
  model_map: {},
  visible_models: [],
  models: {},
};

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
  profiles: [profile],
};

function renderCard() {
  return render(
    <OpenRouterModelSetCard
      provider={provider}
      profile={profile}
      displayName={profile.display_name}
      profilesCount={2}
      gatewayRunning={false}
      refreshConfig={vi.fn().mockResolvedValue(undefined)}
      restartGateway={vi.fn().mockResolvedValue(undefined)}
    />,
  );
}

describe("OpenRouterModelSetCard profile deletion", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    confirmMock.mockReturnValue(true);
    vi.stubGlobal("confirm", confirmMock);
    invokeMock.mockResolvedValue({
      value: null,
      restartGateway: false,
      restartReason: "profile_deleted",
    });
  });

  it("deletes the profile when Delete is clicked without changing its name", async () => {
    renderCard();

    await userEvent.click(screen.getByRole("button", { name: "apiKeyPanel.edit" }));
    expect(screen.getByPlaceholderText("openRouterProfile.profileNamePlaceholder")).toHaveValue("Model 1");

    const deleteButton = screen.getByRole("button", { name: "openRouterProfile.deleteProfile" });
    fireEvent.blur(screen.getByPlaceholderText("openRouterProfile.profileNamePlaceholder"), {
      relatedTarget: deleteButton,
    });
    expect(deleteButton).toBeInTheDocument();
    await userEvent.click(deleteButton);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledTimes(1);
      expect(invokeMock).toHaveBeenCalledWith("delete_openrouter_profile", {
        profileId: "user-model-set-1",
      });
    });
    expect(invokeMock).not.toHaveBeenCalledWith(
      "rename_openrouter_profile",
      expect.anything(),
    );
  });

  it("does not race a pending rename against a confirmed profile deletion", async () => {
    renderCard();

    await userEvent.click(screen.getByRole("button", { name: "apiKeyPanel.edit" }));
    const nameInput = screen.getByPlaceholderText("openRouterProfile.profileNamePlaceholder");
    await userEvent.clear(nameInput);
    await userEvent.type(nameInput, "Name that should not be saved");

    const deleteButton = screen.getByRole("button", { name: "openRouterProfile.deleteProfile" });
    fireEvent.blur(nameInput, { relatedTarget: deleteButton });
    expect(deleteButton).toBeInTheDocument();
    await userEvent.click(deleteButton);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledTimes(1);
      expect(invokeMock).toHaveBeenCalledWith("delete_openrouter_profile", {
        profileId: "user-model-set-1",
      });
    });
    expect(invokeMock).not.toHaveBeenCalledWith(
      "rename_openrouter_profile",
      expect.anything(),
    );
  });

  it("saves a pending rename if the user cancels deletion after keyboard focus leaves the name field", async () => {
    confirmMock.mockReturnValue(false);
    renderCard();

    await userEvent.click(screen.getByRole("button", { name: "apiKeyPanel.edit" }));
    const nameInput = screen.getByPlaceholderText("openRouterProfile.profileNamePlaceholder");
    await userEvent.clear(nameInput);
    await userEvent.type(nameInput, "Renamed Model Set");

    const deleteButton = screen.getByRole("button", { name: "openRouterProfile.deleteProfile" });
    fireEvent.blur(nameInput, { relatedTarget: deleteButton });
    await userEvent.click(deleteButton);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledTimes(1);
      expect(invokeMock).toHaveBeenCalledWith("rename_openrouter_profile", {
        profileId: "user-model-set-1",
        newName: "Renamed Model Set",
      });
    });
    expect(invokeMock).not.toHaveBeenCalledWith("delete_openrouter_profile", expect.anything());
  });
});
