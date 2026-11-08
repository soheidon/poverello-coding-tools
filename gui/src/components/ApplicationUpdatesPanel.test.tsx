import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import ApplicationUpdatesPanel from "./ApplicationUpdatesPanel";

const { invokeMock, checkMock } = vi.hoisted(() => ({ invokeMock: vi.fn(), checkMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: checkMock }));

describe("ApplicationUpdatesPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    document.body.innerHTML = '<div id="application-updates-slot"></div><div id="application-update-notice-slot"></div>';
    invokeMock.mockResolvedValue({ configured: true, currentVersion: "0.27.0", channel: "stable", buildMode: "stable", reason: "ready" });
    checkMock.mockResolvedValue(null);
  });

  afterEach(() => cleanup());

  it("checks once on startup and reports current version only after a successful check", async () => {
    render(<ApplicationUpdatesPanel visible />);
    expect(await screen.findByText("updater.current")).toBeInTheDocument();
    expect(checkMock).toHaveBeenCalledTimes(1);
    expect(screen.getByText("updater.currentVersion")).toBeInTheDocument();
  });

  it("keeps failures distinct from the latest-version state", async () => {
    checkMock.mockRejectedValueOnce(new Error("network unavailable"));
    render(<ApplicationUpdatesPanel visible />);
    expect(await screen.findByRole("alert")).toHaveTextContent("updater.checkFailed");
    expect(screen.queryByText("updater.current")).not.toBeInTheDocument();
  });

  it("does not invoke the updater when this build is not configured", async () => {
    invokeMock.mockResolvedValue({ configured: false, currentVersion: "0.27.0", channel: "dev", buildMode: "dev", reason: "development_disabled" });
    render(<ApplicationUpdatesPanel visible />);
    expect(await screen.findByText("updater.developmentDisabled")).toBeInTheDocument();
    expect(checkMock).not.toHaveBeenCalled();
  });

  it.each([
    ["staging", "staging_misconfigured", "updater.stagingMisconfigured"],
    ["stable", "stable_misconfigured", "updater.stableMisconfigured"],
  ])("explains unavailable %s updater configuration", async (buildMode, reason, messageKey) => {
    invokeMock.mockResolvedValue({ configured: false, currentVersion: "0.27.0", channel: buildMode === "stable" ? "stable" : "dev", buildMode, reason });
    render(<ApplicationUpdatesPanel visible />);
    expect(await screen.findByText(messageKey)).toBeInTheDocument();
    expect(checkMock).not.toHaveBeenCalled();
  });

  it("reports status IPC failures separately from an unavailable updater", async () => {
    invokeMock.mockRejectedValueOnce(new Error("status IPC failed"));
    render(<ApplicationUpdatesPanel visible />);
    expect(await screen.findByRole("alert")).toHaveTextContent("updater.statusUnavailable");
    expect(screen.queryByText("updater.configurationUnavailable")).not.toBeInTheDocument();
  });

  it("starts installation only after an explicit user action", async () => {
    const update = {
      version: "0.28.0",
      body: "Release notes",
      close: vi.fn().mockResolvedValue(undefined),
      downloadAndInstall: vi.fn().mockResolvedValue(undefined),
    };
    checkMock.mockResolvedValueOnce(update);
    render(<ApplicationUpdatesPanel visible />);
    expect(await screen.findByText("updater.available")).toBeInTheDocument();
    expect(update.downloadAndInstall).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "updater.downloadInstall" }));
    await waitFor(() => expect(update.downloadAndInstall).toHaveBeenCalledTimes(1));
    expect(update.downloadAndInstall.mock.calls[0]?.[1]).toEqual({ restartAfterInstall: true });
  });

  it("shows a non-modal startup notice and opens update details on request", async () => {
    checkMock.mockResolvedValueOnce({ version: "0.28.0", body: "Notes", close: vi.fn(), downloadAndInstall: vi.fn() });
    const onOpenSettings = vi.fn();
    render(<ApplicationUpdatesPanel visible={false} onOpenSettings={onOpenSettings} />);
    expect(await screen.findByTestId("application-update-notice")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "updater.viewDetails" }));
    expect(onOpenSettings).toHaveBeenCalledTimes(1);
  });
});
