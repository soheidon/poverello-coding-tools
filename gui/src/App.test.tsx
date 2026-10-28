import type { ReactNode } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import App from "./App";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === "is_first_run") return false;
    if (command === "read_config") return {};
    if (command === "resolve_claude_code_auto_compact") return null;
    return undefined;
  }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  LogicalSize: class LogicalSize {
    constructor(public width: number, public height: number) {}
  },
  currentMonitor: vi.fn(async () => null),
  getCurrentWindow: () => ({
    innerSize: vi.fn(async () => ({ toLogical: () => ({ width: 900, height: 700 }) })),
    outerPosition: vi.fn(async () => ({ y: 0 })),
    outerSize: vi.fn(async () => ({ height: 720 })),
    scaleFactor: vi.fn(async () => 1),
    setSize: vi.fn(async () => undefined),
  }),
}));

vi.mock("./i18n", () => ({
  LanguageProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("./hooks/useHealthCheck", () => ({
  useHealthCheck: () => ({ data: null, error: null, loading: false, refresh: vi.fn() }),
}));

vi.mock("./hooks/useProxyToggle", () => ({
  useProxyToggle: () => ({
    managedRunning: false,
    loading: false,
    error: null,
    diag: null,
    successMessage: null,
    start: vi.fn(),
    stop: vi.fn(),
    clearDiag: vi.fn(),
  }),
}));

vi.mock("./dashboardTiles", () => ({ calculateDashboardCardCount: () => 0 }));
vi.mock("./windowSizing", () => ({
  DASHBOARD_GRID_COLUMNS: 3,
  DASHBOARD_GRID_ROW_GAP: 8,
  DASHBOARD_TILE_MIN_HEIGHT: 180,
  calculateAvailableLogicalHeight: vi.fn(),
  calculateDashboardGridRows: () => 1,
  calculateInitialWindowHeight: () => 720,
}));

vi.mock("./components/TitleBar", () => ({
  default: ({ onTabChange }: {
    onTabChange: (tab: "gateway" | "mcp" | "orchestrator" | "autoConverge" | "settings") => void;
  }) => (
    <button data-testid="title-bar" onClick={() => onTabChange("autoConverge")}>
      legacy auto route
    </button>
  ),
}));
vi.mock("./components/Header", () => ({ default: () => <div data-testid="header" /> }));
vi.mock("./components/ProviderTiles", () => ({ default: () => <div data-testid="gateway-content" /> }));
vi.mock("./components/StatusPanel", () => ({ default: () => null }));
vi.mock("./components/LogPanel", () => ({ default: () => null }));

describe("App navigation", () => {
  it("renders the normal app and does not expose the retained Auto-Converge panel", async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => expect(screen.getByTestId("gateway-content")).toBeInTheDocument());
    expect(screen.getByTestId("title-bar")).toBeInTheDocument();
    expect(screen.queryByTestId("auto-converge-panel")).not.toBeInTheDocument();
    await user.click(screen.getByTestId("title-bar"));
    expect(screen.getByTestId("gateway-content")).toBeInTheDocument();
    expect(screen.queryByTestId("auto-converge-panel")).not.toBeInTheDocument();
  });
});
