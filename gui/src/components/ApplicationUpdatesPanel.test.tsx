import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import ApplicationUpdatesPanel, { formatLastChecked, ReleaseNotesViewer } from "./ApplicationUpdatesPanel";

const { invokeMock, checkMock } = vi.hoisted(() => ({ invokeMock: vi.fn(), checkMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: checkMock }));

describe("formatLastChecked helper", () => {
  it("formats valid ISO timestamp into zero-padded local YYYY-MM-DD HH:mm", () => {
    const fixedIso = "2026-11-09T05:07:00.000Z";
    const date = new Date(fixedIso);
    const pad = (n: number) => String(n).padStart(2, "0");
    const expected = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;

    expect(formatLastChecked(fixedIso, "未確認")).toBe(expected);
  });

  it("falls back gracefully when input is null, undefined, or invalid", () => {
    expect(formatLastChecked(null, "未確認")).toBe("未確認");
    expect(formatLastChecked(undefined, "未確認")).toBe("未確認");
    expect(formatLastChecked("not-a-date", "未確認")).toBe("未確認");
  });
});

describe("ReleaseNotesViewer", () => {
  it("renders headings and lists without raw markers", () => {
    const markdown = "### Application Updates\n- Item 1\n* Item 2\n## Secondary\nParagraph text";
    const { container } = render(<ReleaseNotesViewer markdown={markdown} />);

    expect(screen.getByRole("heading", { level: 4, name: "Application Updates" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 3, name: "Secondary" })).toBeInTheDocument();
    expect(screen.getByText("Item 1")).toBeInTheDocument();
    expect(screen.getByText("Item 2")).toBeInTheDocument();
    expect(screen.getByText("Paragraph text")).toBeInTheDocument();

    // Ensure raw symbols are not in text
    expect(container.textContent).not.toContain("###");
    expect(container.textContent).not.toContain("##");
  });

  it("preserves unsupported Markdown syntax safely as literal text", () => {
    const markdown = "> Quote block\n1. Numbered item\n**Bold text**\n`inline code`";
    render(<ReleaseNotesViewer markdown={markdown} />);

    expect(screen.getByText("> Quote block")).toBeInTheDocument();
    expect(screen.getByText("1. Numbered item")).toBeInTheDocument();
    expect(screen.getByText("**Bold text**")).toBeInTheDocument();
    expect(screen.getByText("`inline code`")).toBeInTheDocument();
  });

  it("escapes HTML tags as plain text without creating DOM nodes", () => {
    const malicious = "<script>alert('xss')</script>\n<img src='x' onerror='alert(1)' />";
    const { container } = render(<ReleaseNotesViewer markdown={malicious} />);

    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByText("<script>alert('xss')</script>")).toBeInTheDocument();
    expect(screen.getByText("<img src='x' onerror='alert(1)' />")).toBeInTheDocument();
  });

  it("returns null for empty or whitespace markdown", () => {
    const { container } = render(<ReleaseNotesViewer markdown={"   \n  "} />);
    expect(container.firstChild).toBeNull();
  });
});

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

  it("dismisses update and returns to idle when 'Later' is clicked", async () => {
    const update = {
      version: "0.28.0",
      body: "Release notes",
      close: vi.fn().mockResolvedValue(undefined),
      downloadAndInstall: vi.fn().mockResolvedValue(undefined),
    };
    checkMock.mockResolvedValueOnce(update);
    render(<ApplicationUpdatesPanel visible />);
    expect(await screen.findByText("updater.available")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "updater.later" }));
    await waitFor(() => {
      expect(update.close).toHaveBeenCalledTimes(1);
      expect(screen.queryByText("updater.available")).not.toBeInTheDocument();
    });
  });

  it("disables checking button and displays checking state while check is pending", async () => {
    let resolveCheck: (val: any) => void = () => {};
    const checkPromise = new Promise((resolve) => { resolveCheck = resolve; });
    checkMock.mockImplementationOnce(() => checkPromise);

    render(<ApplicationUpdatesPanel visible />);

    await waitFor(() => {
      expect(screen.getByRole("status")).toHaveTextContent("updater.checking");
      expect(screen.getByRole("button", { name: "updater.checking" })).toBeDisabled();
    });

    resolveCheck(null);
    expect(await screen.findByText("updater.current")).toBeInTheDocument();
  });

  it("disables actions and displays progress status during installation", async () => {
    let finishInstall: () => void = () => {};
    const installPromise = new Promise<void>((resolve) => { finishInstall = resolve; });
    const update = {
      version: "0.28.0",
      body: "Notes",
      close: vi.fn(),
      downloadAndInstall: vi.fn().mockImplementation(() => installPromise),
    };
    checkMock.mockResolvedValueOnce(update);

    render(<ApplicationUpdatesPanel visible />);
    expect(await screen.findByText("updater.available")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "updater.downloadInstall" }));

    await waitFor(() => {
      expect(screen.getByRole("status")).toHaveTextContent("updater.installing");
      expect(screen.getByRole("button", { name: "updater.checking" })).toBeDisabled();
    });

    finishInstall();
  });

  it("dynamically attaches to portal slot created after initial render", async () => {
    // Start with empty DOM
    document.body.innerHTML = "";
    render(<ApplicationUpdatesPanel visible />);

    // Add slot later
    const slot = document.createElement("div");
    slot.id = "application-updates-slot";
    document.body.appendChild(slot);

    await waitFor(() => {
      expect(screen.getByTestId("application-updates-panel")).toBeInTheDocument();
    });
  });
});
