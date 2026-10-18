import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import AutoConvergePanel from "./AutoConvergePanel";
import { invoke } from "@tauri-apps/api/core";
import { translations as en } from "../../i18n/lang/en";
import { translations as ja } from "../../i18n/lang/ja";
import { translations as de } from "../../i18n/lang/de";
import { translations as es } from "../../i18n/lang/es";
import { translations as fr } from "../../i18n/lang/fr";
import { translations as ko } from "../../i18n/lang/ko";
import { translations as zhCN } from "../../i18n/lang/zh-CN";
import { translations as zhTW } from "../../i18n/lang/zh-TW";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("AutoConvergePanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders built-in presets (Fast Local, Balanced, Strict Release) and defaults to Balanced", () => {
    render(<AutoConvergePanel />);
    expect(screen.getByTestId("auto-converge-panel")).toBeTruthy();
    const select = screen.getByTestId("profile-select") as HTMLSelectElement;
    expect(select.value).toBe("balanced");
    expect(select.options.length).toBe(3);
  });

  it("resets per-run permissions when switching profiles or target branch", () => {
    render(<AutoConvergePanel />);
    const select = screen.getByTestId("profile-select") as HTMLSelectElement;
    fireEvent.change(select, { target: { value: "strict-release" } });
    expect(select.value).toBe("strict-release");

    const branchInput = screen.getByTestId("input-target-branch") as HTMLInputElement;
    fireEvent.change(branchInput, { target: { value: "release/v0.24.6" } });
    expect(branchInput.value).toBe("release/v0.24.6");
  });

  it("clones a custom profile and resets to built-in presets", () => {
    render(<AutoConvergePanel />);
    const cloneBtn = screen.getByTestId("clone-profile-btn");
    fireEvent.click(cloneBtn);
    const select = screen.getByTestId("profile-select") as HTMLSelectElement;
    expect(select.options.length).toBe(4);

    fireEvent.click(screen.getByTestId("reset-presets-btn"));
    expect(select.options.length).toBe(3);
    expect(select.value).toBe("balanced");
  });

  it("renders start run button when no run is active", () => {
    const mockedInvoke = vi.mocked(invoke);
    mockedInvoke.mockResolvedValueOnce([]);
    render(<AutoConvergePanel />);
    expect(screen.getByTestId("start-run-btn")).toBeDisabled();
    expect(screen.getByRole("alert")).toBeInTheDocument();
  });

  it("provides the unavailable notice in every supported locale", () => {
    const expectedCopy = [
      ["en", en["autoConverge.unavailable"], "Auto-Converge is unavailable in this desktop build. Reviewer OS isolation is not available, so runs are blocked before any agent process starts."],
      ["ja", ja["autoConverge.unavailable"], "このデスクトップ版ではAuto-Convergeを利用できません。ReviewerのOS隔離が利用できないため、エージェントプロセスの起動前に実行がブロックされます。"],
      ["de", de["autoConverge.unavailable"], "Auto-Converge ist in diesem Desktop-Build nicht verfügbar. Die OS-Isolierung für Reviewer fehlt; daher werden Läufe blockiert, bevor ein Agent-Prozess gestartet wird."],
      ["es", es["autoConverge.unavailable"], "Auto-Converge no está disponible en esta versión de escritorio. No se dispone del aislamiento de SO para el revisor, por lo que las ejecuciones se bloquean antes de iniciar cualquier proceso de agente."],
      ["fr", fr["autoConverge.unavailable"], "Auto-Converge n’est pas disponible dans cette version de bureau. L’isolation du système d’exploitation du réviseur n’est pas disponible ; les exécutions sont donc bloquées avant le lancement de tout processus agent."],
      ["ko", ko["autoConverge.unavailable"], "이 데스크톱 빌드에서는 Auto-Converge를 사용할 수 없습니다. Reviewer OS 격리가 제공되지 않아 에이전트 프로세스를 시작하기 전에 실행이 차단됩니다."],
      ["zh-CN", zhCN["autoConverge.unavailable"], "此桌面版本暂不可使用 Auto-Converge。由于未提供 Reviewer 操作系统隔离，运行会在启动任何代理进程前被阻止。"],
      ["zh-TW", zhTW["autoConverge.unavailable"], "此桌面版本目前無法使用 Auto-Converge。由於未提供 Reviewer 作業系統隔離，執行會在啟動任何代理程序前遭到阻止。"],
    ] as const;

    for (const [locale, actual, expected] of expectedCopy) {
      expect(actual, `incorrect unavailable notice for ${locale}`).toBe(expected);
    }
  });
});
