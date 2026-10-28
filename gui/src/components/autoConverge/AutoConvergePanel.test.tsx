import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import AutoConvergePanel from "./AutoConvergePanel";
import { invoke } from "@tauri-apps/api/core";
import * as i18n from "../../i18n";
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
    vi.spyOn(i18n, "useTranslation").mockImplementation(() => ({
      t: (key: string) => (en as Record<string, string>)[key] ?? key,
      lang: "en",
    }) as ReturnType<typeof i18n.useTranslation>);
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

  it("renders start run button when no run is active and keeps it strictly disabled in Phase A", () => {
    const mockedInvoke = vi.mocked(invoke);
    mockedInvoke.mockResolvedValueOnce([]);
    render(<AutoConvergePanel />);
    const startBtn = screen.getByTestId("start-run-btn");
    expect(startBtn).toBeDisabled();
    expect(startBtn).toHaveAttribute("title", en["autoConverge.startDisabledReason"]);
    expect(screen.getByRole("alert")).toBeInTheDocument();
  });

  it("renders R0-D notice banner and allows toggling opt-in checkbox", () => {
    render(<AutoConvergePanel />);
    const banner = screen.getByTestId("auto-converge-r0d-banner");
    expect(banner).toBeInTheDocument();
    expect(banner).toHaveTextContent(en["autoConverge.r0dNotice"]);

    const checkbox = screen.getByTestId("r0d-acknowledge-checkbox") as HTMLInputElement;
    expect(checkbox).toBeInTheDocument();
    expect(checkbox.checked).toBe(false);

    fireEvent.click(checkbox);
    expect(checkbox.checked).toBe(true);

    // Start button must still remain disabled even when acknowledged
    const startBtn = screen.getByTestId("start-run-btn");
    expect(startBtn).toBeDisabled();
    expect(startBtn).toHaveAttribute("title", en["autoConverge.startDisabledReason"]);
  });

  it("provides the R0-D notice, acknowledge label, and disabled reason in every supported locale", () => {
    const expectedCopy = [
      [
        "en",
        en["autoConverge.r0dNotice"],
        "Reviewer Level R0-D: detection-only; unexpected changes are detected and recoverable within the snapshot scope, not prevented.",
        en["autoConverge.r0dAcknowledge"],
        "I understand R0-D is detection-only",
        en["autoConverge.startDisabledReason"],
        "Verification pending (Phases B–G)",
      ],
      [
        "ja",
        ja["autoConverge.r0dNotice"],
        "Reviewer Level R0-D: 検知専用モードです。予期しない変更はスナップショット範囲内で検知・復旧されますが、事前の書き込み防止は行われません。",
        ja["autoConverge.r0dAcknowledge"],
        "R0-Dが検知専用であることを理解しました",
        ja["autoConverge.startDisabledReason"],
        "検証完了待ち（Phase B–G）",
      ],
      [
        "de",
        de["autoConverge.r0dNotice"],
        "Reviewer-Stufe R0-D: Nur-Erkennung; unerwartete Änderungen werden innerhalb des Snapshot-Bereichs erkannt und wiederhergestellt, aber nicht verhindert.",
        de["autoConverge.r0dAcknowledge"],
        "Ich verstehe, dass R0-D nur der Erkennung dient",
        de["autoConverge.startDisabledReason"],
        "Ausstehende Verifizierung (Phasen B–G)",
      ],
      [
        "es",
        es["autoConverge.r0dNotice"],
        "Nivel de revisor R0-D: solo detección; los cambios inesperados se detectan y recuperan dentro del alcance de la instantánea, no se previenen.",
        es["autoConverge.r0dAcknowledge"],
        "Entiendo que R0-D es solo de detección",
        es["autoConverge.startDisabledReason"],
        "Verificación pendiente (Fases B–G)",
      ],
      [
        "fr",
        fr["autoConverge.r0dNotice"],
        "Niveau de réviseur R0-D : détection uniquement ; les modifications inattendues sont détectées et récupérées dans le périmètre de l'instantané, mais ne sont pas empêchées.",
        fr["autoConverge.r0dAcknowledge"],
        "Je comprends que le mode R0-D fonctionne en détection uniquement",
        fr["autoConverge.startDisabledReason"],
        "Vérification en attente (Phases B–G)",
      ],
      [
        "ko",
        ko["autoConverge.r0dNotice"],
        "리뷰어 레벨 R0-D: 감지 전용 모드입니다. 예기치 않은 변경은 스냅샷 범위 내에서 감지 및 복구되지만, 사전 쓰기 방지는 수행되지 않습니다.",
        ko["autoConverge.r0dAcknowledge"],
        "R0-D가 감지 전용임을 이해했습니다",
        ko["autoConverge.startDisabledReason"],
        "검증 대기 중 (Phase B–G)",
      ],
      [
        "zh-CN",
        zhCN["autoConverge.r0dNotice"],
        "审阅者级别 R0-D：仅检测模式；非预期更改将在快照范围内被检测并可恢复，但不会被提前阻止写入。",
        zhCN["autoConverge.r0dAcknowledge"],
        "我了解 R0-D 为仅检测模式",
        zhCN["autoConverge.startDisabledReason"],
        "等待验证完成（阶段 B–G）",
      ],
      [
        "zh-TW",
        zhTW["autoConverge.r0dNotice"],
        "審查者層級 R0-D：僅偵測模式；非預期變更將在快照範圍內被偵測並可復原，但不會被提前阻止寫入。",
        zhTW["autoConverge.r0dAcknowledge"],
        "我了解 R0-D 為僅偵測模式",
        zhTW["autoConverge.startDisabledReason"],
        "等待驗證完成（階段 B–G）",
      ],
    ] as const;

    for (const [locale, notice, expNotice, ack, expAck, reason, expReason] of expectedCopy) {
      expect(notice, `incorrect r0dNotice for ${locale}`).toBe(expNotice);
      expect(ack, `incorrect r0dAcknowledge for ${locale}`).toBe(expAck);
      expect(reason, `incorrect startDisabledReason for ${locale}`).toBe(expReason);
    }
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
