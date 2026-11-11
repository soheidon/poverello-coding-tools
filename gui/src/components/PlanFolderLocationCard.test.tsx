import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import PlanFolderLocationCard from "./PlanFolderLocationCard";
import * as tauriBridge from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("PlanFolderLocationCard Component", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders Antigravity plan folder location card and fetches location into input box", async () => {
    vi.mocked(tauriBridge.invoke).mockImplementation(async (cmd, args: any) => {
      if (cmd === "get_plan_manager_location" && args?.surface === "antigravity") {
        return "C:/dev/my-antigravity-project";
      }
      return null;
    });

    render(<PlanFolderLocationCard surface="antigravity" />);

    expect(screen.getByTestId("plan-folder-location-card-antigravity")).toBeInTheDocument();
    await waitFor(() => {
      const input = screen.getByTestId<HTMLInputElement>("input-plan-folder-location");
      expect(input.value).toBe("C:/dev/my-antigravity-project");
    });
    expect(screen.getByTestId("btn-select-plan-root")).toBeInTheDocument();
    expect(screen.getByTestId("btn-reset-plan-root")).toBeInTheDocument();

    const controlsRow = screen.getByTestId("plan-folder-controls-row");
    expect(controlsRow).toHaveStyle({ display: "flex", flexWrap: "wrap" });
  });

  it("renders Codex plan folder location card with unconfigured state", async () => {
    vi.mocked(tauriBridge.invoke).mockImplementation(async (cmd, args: any) => {
      if (cmd === "get_plan_manager_location" && args?.surface === "codex") {
        return null;
      }
      return null;
    });

    render(<PlanFolderLocationCard surface="codex" />);

    expect(screen.getByTestId("plan-folder-location-card-codex")).toBeInTheDocument();
    await waitFor(() => {
      const input = screen.getByTestId<HTMLInputElement>("input-plan-folder-location");
      expect(input.value).toBe("");
      expect(input.placeholder).toMatch(/planFolder\.inputPlaceholder|未設定|Unconfigured/i);
    });
    expect(screen.queryByTestId("btn-reset-plan-root")).not.toBeInTheDocument();
  });

  it("selects a folder via picker button and routes through set_plan_manager_location to persist", async () => {
    const user = userEvent.setup();
    vi.mocked(tauriBridge.invoke).mockImplementation(async (cmd, args: any) => {
      if (cmd === "get_plan_manager_location") return null;
      if (cmd === "select_plan_root_dialog" && args?.surface === "antigravity") {
        return "C:/dev/new-selected-project";
      }
      if (cmd === "set_plan_manager_location" && args?.path === "C:/dev/new-selected-project") {
        return "C:/dev/new-selected-project";
      }
      return null;
    });

    render(<PlanFolderLocationCard surface="antigravity" />);

    await waitFor(() => {
      expect(screen.getByTestId<HTMLInputElement>("input-plan-folder-location").value).toBe("");
    });

    const selectBtn = screen.getByTestId("btn-select-plan-root");
    await user.click(selectBtn);

    await waitFor(() => {
      expect(tauriBridge.invoke).toHaveBeenCalledWith("select_plan_root_dialog", { surface: "antigravity" });
      expect(tauriBridge.invoke).toHaveBeenCalledWith("set_plan_manager_location", {
        surface: "antigravity",
        path: "C:/dev/new-selected-project",
      });
      expect(screen.getByTestId<HTMLInputElement>("input-plan-folder-location").value).toBe("C:/dev/new-selected-project");
      const statusEl = screen.getByRole("status");
      expect(statusEl).toHaveTextContent(/設定を保存しました|savedMessage/i);
    });
  });

  it("picker cancel preserves existing value and does not invoke save", async () => {
    const user = userEvent.setup();
    vi.mocked(tauriBridge.invoke).mockImplementation(async (cmd) => {
      if (cmd === "get_plan_manager_location") return "C:/dev/existing-folder";
      if (cmd === "select_plan_root_dialog") return null; // User cancelled
      return null;
    });

    render(<PlanFolderLocationCard surface="antigravity" />);

    await waitFor(() => {
      expect(screen.getByTestId<HTMLInputElement>("input-plan-folder-location").value).toBe("C:/dev/existing-folder");
    });

    const selectBtn = screen.getByTestId("btn-select-plan-root");
    await user.click(selectBtn);

    expect(screen.getByTestId<HTMLInputElement>("input-plan-folder-location").value).toBe("C:/dev/existing-folder");
    expect(tauriBridge.invoke).not.toHaveBeenCalledWith("set_plan_manager_location", expect.anything());
    expect(screen.queryByTestId("saved-message")).not.toBeInTheDocument();
  });

  it("displays error and maintains previous value if saving picked folder fails", async () => {
    const user = userEvent.setup();
    vi.mocked(tauriBridge.invoke).mockImplementation(async (cmd, args: any) => {
      if (cmd === "get_plan_manager_location") return "C:/dev/existing-folder";
      if (cmd === "select_plan_root_dialog") return "C:/dev/corrupt-target";
      if (cmd === "set_plan_manager_location") {
        throw new Error("Target folder is invalid");
      }
      return null;
    });

    render(<PlanFolderLocationCard surface="codex" />);

    await waitFor(() => {
      expect(screen.getByTestId<HTMLInputElement>("input-plan-folder-location").value).toBe("C:/dev/existing-folder");
    });

    const selectBtn = screen.getByTestId("btn-select-plan-root");
    await user.click(selectBtn);

    await waitFor(() => {
      const alertEl = screen.getByRole("alert");
      expect(alertEl).toHaveTextContent("Target folder is invalid");
      // Reverted to previous valid persisted value
      expect(screen.getByTestId<HTMLInputElement>("input-plan-folder-location").value).toBe("C:/dev/existing-folder");
    });
  });

  it("allows direct manual typing and saves on Enter key", async () => {
    const user = userEvent.setup();
    vi.mocked(tauriBridge.invoke).mockImplementation(async (cmd, args: any) => {
      if (cmd === "get_plan_manager_location") return null;
      if (cmd === "set_plan_manager_location" && args?.path === "D:/my/typed/repo") {
        return "D:/my/typed/repo";
      }
      return null;
    });

    render(<PlanFolderLocationCard surface="antigravity" />);

    await waitFor(() => {
      expect(screen.getByTestId<HTMLInputElement>("input-plan-folder-location").value).toBe("");
    });

    const input = screen.getByTestId<HTMLInputElement>("input-plan-folder-location");
    await user.type(input, "D:/my/typed/repo{Enter}");

    await waitFor(() => {
      expect(tauriBridge.invoke).toHaveBeenCalledWith("set_plan_manager_location", {
        surface: "antigravity",
        path: "D:/my/typed/repo",
      });
      expect(screen.getByRole("status")).toBeInTheDocument();
      expect(input.value).toBe("D:/my/typed/repo");
    });
  });

  it("saves on blur when input value changes", async () => {
    const user = userEvent.setup();
    vi.mocked(tauriBridge.invoke).mockImplementation(async (cmd, args: any) => {
      if (cmd === "get_plan_manager_location") return "C:/initial/path";
      if (cmd === "set_plan_manager_location" && args?.path === "C:/updated/path") {
        return "C:/updated/path";
      }
      return null;
    });

    render(<PlanFolderLocationCard surface="codex" />);

    await waitFor(() => {
      expect(screen.getByTestId<HTMLInputElement>("input-plan-folder-location").value).toBe("C:/initial/path");
    });

    const input = screen.getByTestId<HTMLInputElement>("input-plan-folder-location");
    await user.clear(input);
    await user.type(input, "C:/updated/path");
    await user.tab();

    await waitFor(() => {
      expect(tauriBridge.invoke).toHaveBeenCalledWith("set_plan_manager_location", {
        surface: "codex",
        path: "C:/updated/path",
      });
      expect(screen.getByRole("status")).toBeInTheDocument();
      expect(input.value).toBe("C:/updated/path");
    });
  });

  it("clearing input to empty saves null (reset)", async () => {
    const user = userEvent.setup();
    vi.mocked(tauriBridge.invoke).mockImplementation(async (cmd, args: any) => {
      if (cmd === "get_plan_manager_location") return "C:/old/path";
      if (cmd === "set_plan_manager_location" && args?.path === null) {
        return null;
      }
      return null;
    });

    render(<PlanFolderLocationCard surface="antigravity" />);

    await waitFor(() => {
      expect(screen.getByTestId<HTMLInputElement>("input-plan-folder-location").value).toBe("C:/old/path");
    });

    const input = screen.getByTestId<HTMLInputElement>("input-plan-folder-location");
    await user.clear(input);
    await user.keyboard("{Enter}");

    await waitFor(() => {
      expect(tauriBridge.invoke).toHaveBeenCalledWith("set_plan_manager_location", {
        surface: "antigravity",
        path: null,
      });
      expect(input.value).toBe("");
    });
  });

  it("reverts input and displays accessible alert when backend validation fails", async () => {
    const user = userEvent.setup();
    vi.mocked(tauriBridge.invoke).mockImplementation(async (cmd, args: any) => {
      if (cmd === "get_plan_manager_location") return "C:/valid/existing";
      if (cmd === "set_plan_manager_location") {
        throw new Error("Target folder does not exist: 'Z:/invalid/path'");
      }
      return null;
    });

    render(<PlanFolderLocationCard surface="antigravity" />);

    await waitFor(() => {
      expect(screen.getByTestId<HTMLInputElement>("input-plan-folder-location").value).toBe("C:/valid/existing");
    });

    const input = screen.getByTestId<HTMLInputElement>("input-plan-folder-location");
    await user.clear(input);
    await user.type(input, "Z:/invalid/path{Enter}");

    await waitFor(() => {
      const alertEl = screen.getByRole("alert");
      expect(alertEl).toHaveTextContent("Target folder does not exist");
      expect(input.value).toBe("C:/valid/existing");
    });
  });

  it("resets location when reset button is clicked", async () => {
    const user = userEvent.setup();
    vi.mocked(tauriBridge.invoke).mockImplementation(async (cmd, args: any) => {
      if (cmd === "get_plan_manager_location") return "C:/dev/current-project";
      if (cmd === "set_plan_manager_location" && args?.path === null) {
        return null;
      }
      return null;
    });

    render(<PlanFolderLocationCard surface="codex" />);

    await waitFor(() => {
      expect(screen.getByTestId<HTMLInputElement>("input-plan-folder-location").value).toBe("C:/dev/current-project");
    });

    const resetBtn = screen.getByTestId("btn-reset-plan-root");
    await user.click(resetBtn);

    await waitFor(() => {
      expect(screen.getByTestId<HTMLInputElement>("input-plan-folder-location").value).toBe("");
    });
  });
});
