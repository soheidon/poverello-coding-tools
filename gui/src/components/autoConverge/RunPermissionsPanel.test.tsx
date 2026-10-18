import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import RunPermissionsPanel from "./RunPermissionsPanel";
import {
  PERM_COMMIT,
  PERM_STAGE,
  type AutoConvergeProfile,
  type ReleasePermissionIntent,
} from "../../types/autoConverge";
import { BUILTIN_FALLBACK_PROFILES } from "./AutoConvergePanel";

const STRICT_PROFILE: AutoConvergeProfile = BUILTIN_FALLBACK_PROFILES[2];

describe("RunPermissionsPanel", () => {
  it("disables all Git/Release/Paid API toggles when deliveryPhase is PhaseALocalConvergenceMvp (§16.5)", () => {
    const onChangePermissions = vi.fn();
    const onChangeReleaseIntent = vi.fn();
    render(
      <RunPermissionsPanel
        profile={STRICT_PROFILE}
        runPermissions={0}
        onChangePermissions={onChangePermissions}
        releaseIntent={{ requested_version_tag: "", release_repository: "" }}
        onChangeReleaseIntent={onChangeReleaseIntent}
        deliveryPhase="PhaseALocalConvergenceMvp"
      />,
    );

    expect(screen.getByTestId("phase-a-deferred-notice")).toBeTruthy();
    expect((screen.getByTestId("perm-stage") as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByTestId("perm-commit") as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByTestId("perm-push-branch") as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByTestId("perm-tag") as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByTestId("perm-push-tag") as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByTestId("perm-release-publish") as HTMLInputElement).disabled).toBe(true);
    expect((screen.getByTestId("perm-paid-live-api") as HTMLInputElement).disabled).toBe(true);
  });

  it("enforces dependency ordering and dual-gate capability checks in PhaseCTagAndRelease", () => {
    let currentPerms = 0;
    const handleChangePermissions = vi.fn((next: number) => {
      currentPerms = next;
    });
    const intent: ReleasePermissionIntent = {
      requested_version_tag: "",
      release_repository: "",
    };

    const { rerender } = render(
      <RunPermissionsPanel
        profile={STRICT_PROFILE}
        runPermissions={currentPerms}
        onChangePermissions={handleChangePermissions}
        releaseIntent={intent}
        onChangeReleaseIntent={vi.fn()}
        deliveryPhase="PhaseCTagAndRelease"
      />,
    );

    expect((screen.getByTestId("perm-stage") as HTMLInputElement).disabled).toBe(false);
    expect((screen.getByTestId("perm-commit") as HTMLInputElement).disabled).toBe(false);

    // Checking commit automatically enables stage prerequisite (§9.1)
    fireEvent.click(screen.getByTestId("perm-commit"));
    expect((currentPerms & PERM_STAGE) !== 0).toBe(true);
    expect((currentPerms & PERM_COMMIT) !== 0).toBe(true);

    rerender(
      <RunPermissionsPanel
        profile={STRICT_PROFILE}
        runPermissions={currentPerms}
        onChangePermissions={handleChangePermissions}
        releaseIntent={intent}
        onChangeReleaseIntent={vi.fn()}
        deliveryPhase="PhaseCTagAndRelease"
      />,
    );

    // Unchecking stage cascades and disables commit
    fireEvent.click(screen.getByTestId("perm-stage"));
    expect((currentPerms & PERM_STAGE) !== 0).toBe(false);
    expect((currentPerms & PERM_COMMIT) !== 0).toBe(false);
  });
});
