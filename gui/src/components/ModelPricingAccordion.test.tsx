import { fireEvent, render, screen, within, act } from "@testing-library/react";
import { describe, expect, it, vi, afterEach, beforeEach } from "vitest";
import ModelPricingAccordion, { PriceCell, formatPrice } from "./ModelPricingAccordion";
import { MODEL_PRICING } from "../config/modelPricing";
import { PROVIDER_MODELS } from "../modelCapabilities";
import { CHINA_HOLIDAYS_2026 } from "../config/chinaHolidays";
import * as i18n from "../i18n";
import { translations as en } from "../i18n/lang/en";
import { translations as ja } from "../i18n/lang/ja";
import { translations as zhCN } from "../i18n/lang/zh-CN";
import { translations as zhTW } from "../i18n/lang/zh-TW";
import { translations as ko } from "../i18n/lang/ko";
import { translations as de } from "../i18n/lang/de";
import { translations as fr } from "../i18n/lang/fr";
import { translations as es } from "../i18n/lang/es";

describe("ModelPricingAccordion", () => {
  function openPricing() {
    render(<ModelPricingAccordion />);
  }

  it("renders pricing table unconditionally without accordion toggle", () => {
    render(<ModelPricingAccordion />);
    expect(screen.getByRole("columnheader", { name: "Input/1M" })).toBeInTheDocument();
  });

  it("keeps OpenRouter IDs unique and orders all GPT-5.6 variants", () => {
    const openRouterIds = PROVIDER_MODELS.openrouter;
    expect(new Set(openRouterIds).size).toBe(openRouterIds.length);
    expect(openRouterIds.filter((id) => id.startsWith("openai/gpt-5.6-"))).toEqual([
      "openai/gpt-5.6-sol",
      "openai/gpt-5.6-sol-pro",
      "openai/gpt-5.6-terra",
      "openai/gpt-5.6-terra-pro",
      "openai/gpt-5.6-luna",
      "openai/gpt-5.6-luna-pro",
    ]);

    openPricing();
    const modelCells = screen.getAllByRole("cell").map((cell) => cell.textContent ?? "");
    const gpt56Ids = modelCells.filter((text) => text.startsWith("openai/gpt-5.6-"));
    expect(gpt56Ids).toHaveLength(6);
  });

  it("renders current and revised standard prices for Sol, Laguna, Step, and Gemini Flash", () => {
    openPricing();

    // Sol / Sol Pro ($2.000 current, $4.000 regular) + GPT-6 Sol/Pro, GPT-6.1 Sol/Pro ($2.000 current)
    expect(screen.getAllByText("$2.000")).toHaveLength(9);
    // Sol / Sol Pro regular input ($4.000) + Kimi K2.6 & Kimi K2.7 Code output ($4.000)
    expect(screen.getAllByText("$4.000")).toHaveLength(4);

    // Gemini 3.7 / 3.8 Flash ($0.750 current, $1.500 regular)
    expect(screen.getAllByText("$0.750")).toHaveLength(2);
    expect(screen.getAllByText("$1.500")).toHaveLength(2);
    expect(screen.getAllByText("$3.750")).toHaveLength(2);
    expect(screen.getAllByText("$7.500")).toHaveLength(2);

    // Sol (3x2) + Laguna S 2.1 (3x1) + Gemini 3.7 (3x1) + Gemini 3.8 (3x1) + Step 3.7 (2x1) = 17 strikethrough elements
    expect(document.querySelectorAll("s")).toHaveLength(17);
  });

  it("renders all three regular prices in the Sol Pro row", () => {
    openPricing();

    const solProRow = screen
      .getByText("openai/gpt-5.6-sol-pro")
      .closest("tr");

    expect(solProRow).not.toBeNull();
    expect(within(solProRow!).getByText("$2.000")).toBeInTheDocument();
    expect(within(solProRow!).getByText("$4.000")).toBeInTheDocument();
    expect(within(solProRow!).getByText("$10.000")).toBeInTheDocument();
    expect(within(solProRow!).getByText("$20.000")).toBeInTheDocument();
    expect(within(solProRow!).getByText("$0.2000")).toBeInTheDocument();
    expect(within(solProRow!).getByText("$0.4000")).toBeInTheDocument();
    expect(solProRow!.querySelectorAll("s")).toHaveLength(3);
  });

  it("uses the production price catalog for the complete data test", () => {
    const promoIds = [
      "openai/gpt-5.6-sol",
      "openai/gpt-5.6-sol-pro",
      "google/gemini-3.7-flash",
      "google/gemini-3.8-flash",
      "poolside/laguna-s-2.1",
    ] as const;

    for (const id of promoIds) {
      const pricing = MODEL_PRICING[id];
      const regular = [
        pricing.regularInputPerMillionUsd,
        pricing.regularOutputPerMillionUsd,
        pricing.regularCachedInputPerMillionUsd,
      ];

      expect(regular.some((value) => value !== undefined)).toBe(true);
    }
  });

  it("renders Sol with promotional discount and regular prices", () => {
    openPricing();

    const solRows = screen.getAllByRole("row").filter((row) =>
      row.textContent?.includes("openai/gpt-5.6-sol"),
    );
    expect(solRows).toHaveLength(2);
    for (const row of solRows) {
      expect(row.querySelectorAll("s")).toHaveLength(3);
      expect(row.textContent).toContain("$2.000");
      expect(row.textContent).toContain("$4.000");
      expect(row.textContent).toContain("$10.000");
      expect(row.textContent).toContain("$20.000");
    }
  });

  it("provides localized screen-reader text for discounted prices", () => {
    openPricing();

    const texts = screen.getAllByText(
      "Current price $2.000, revised standard price $4.000",
    );
    expect(texts).toHaveLength(2);
    for (const text of texts) {
      expect(text).toHaveClass("sr-only");
    }
    expect(document.querySelectorAll('[aria-hidden="true"]')).not.toHaveLength(0);
  });

  it("renders the promotion and long-context notes", () => {
    openPricing();

    // Sol (1), Sol Pro (1), Laguna S (1), Gemini 3.7 (1), Gemini 3.8 (1), Step 3.7 (1) = 6 models with promotion
    expect(
      screen.getAllByText(/Limited-time promotional pricing is currently active on OpenRouter/),
    ).toHaveLength(6);
    expect(
      screen.queryByText(/50% provider discount/),
    ).not.toBeInTheDocument();
    expect(screen.getAllByText(/272K tokens or more/)).toHaveLength(6);
  });

  it("keeps existing models as single-price rows", () => {
    openPricing();

    const existingRow = Array.from(document.querySelectorAll("tbody tr")).find((row) =>
      row.textContent?.includes("deepseek-v4-pro"),
    ) ?? null;
    expect(existingRow).not.toBeNull();
    expect(existingRow?.querySelectorAll("s")).toHaveLength(0);
  });

  it("preserves zero as a valid price", () => {
    expect(formatPrice(0, 3)).toBe("$0.000");
    render(<PriceCell current={0} decimals={3} />);
    expect(screen.getByText("$0.000")).toBeInTheDocument();
  });

  it("keeps standard and pro data aligned and complete", () => {
    const families = ["sol", "terra", "luna"] as const;
    for (const family of families) {
      const standard = MODEL_PRICING[`openai/gpt-5.6-${family}`];
      const pro = MODEL_PRICING[`openai/gpt-5.6-${family}-pro`];
      expect(pro).toMatchObject(standard);
    }

    expect(MODEL_PRICING["openai/gpt-5.6-sol"].regularInputPerMillionUsd).toBe(4);
    expect(MODEL_PRICING["openai/gpt-5.6-terra"].regularInputPerMillionUsd).toBeUndefined();
    expect(MODEL_PRICING["openai/gpt-5.6-luna"].regularInputPerMillionUsd).toBeUndefined();
  });

  it("does not render kimi-code row in pricing accordion and does not display $0.000 for Kimi Code", () => {
    openPricing();

    const allCellTexts = screen.getAllByRole("cell").map((cell) => cell.textContent ?? "");
    expect(allCellTexts.some((text) => text.includes("kimi-for-coding"))).toBe(false);
    expect(allCellTexts.some((text) => text.includes("kimi-for-coding-highspeed"))).toBe(false);

    const rows = screen.getAllByRole("row");
    const kimiCodeRows = rows.filter(
      (row) => row.textContent?.includes("Kimi Code") || row.textContent?.includes("kimi-for-coding"),
    );
    expect(kimiCodeRows).toHaveLength(0);

    expect(MODEL_PRICING["kimi-for-coding"]).toBeUndefined();
    expect(MODEL_PRICING["kimi-for-coding-highspeed"]).toBeUndefined();
  });

  it("renders GPT-6 Astra, GPT-6 Astra Pro, and GPT Astra Latest with $10/$50 pricing", () => {
    openPricing();

    const astraModels = ["openai/gpt-6-astra", "openai/gpt-6-astra-pro", "openai/gpt-astra-latest"];
    for (const id of astraModels) {
      const pricing = MODEL_PRICING[id];
      expect(pricing).toBeDefined();
      expect(pricing.inputPerMillionUsd).toBe(10.0);
      expect(pricing.outputPerMillionUsd).toBe(50.0);
      expect(pricing.cachedInputPerMillionUsd).toBeUndefined();

      const row = screen.getByText(id).closest("tr");
      expect(row).not.toBeNull();
      expect(within(row!).getByText("$10.000")).toBeInTheDocument();
      expect(within(row!).getByText("$50.000")).toBeInTheDocument();
    }
  });

  it("renders GLM (direct API) with 4 models and separates it from OpenRouter", () => {
    openPricing();

    const rows = screen.getAllByRole("row");
    const glmRows = rows.filter((row) => row.textContent?.includes("GLM"));
    expect(glmRows).toHaveLength(4);

    expect(screen.getByText("GLM-5.2")).toBeInTheDocument();
    expect(screen.getByText("GLM-5.3")).toBeInTheDocument();
    expect(screen.getByText("GLM-5.3 Flash")).toBeInTheDocument();
    expect(screen.getByText("GLM-5.3 FlashX")).toBeInTheDocument();
  });

  it("shows GLM pricing without a Coding Plan eligibility or PAYG warning", () => {
    openPricing();

    for (const modelName of [
      "GLM-5.2",
      "GLM-5.3",
      "GLM-5.3 Flash",
      "GLM-5.3 FlashX",
    ]) {
      const row = screen.getByText(modelName).closest("tr");
      expect(row).not.toBeNull();
      expect(row).not.toHaveTextContent(/Coding Plan|PAYG/i);
    }
  });

  describe("Chapter 5: Chinese holiday calendar maintenance status UI (§18)", () => {
    beforeEach(() => {
      vi.spyOn(i18n, "useTranslation").mockImplementation(() => ({
        t: (key: string, params?: Record<string, string | number>) => {
          if (key === "modelPricing.chinaHolidays.updateDue") {
            return `${params?.year}年の中国休日データが未収録です`;
          }
          if (key === "modelPricing.chinaHolidays.expired") {
            return `${params?.year}年の中国休日データが未収録です。DeepSeekの休日料金判定は不完全です`;
          }
          if (key === "modelPricing.header") return "モデル料金";
          if (key === "modelPricing.usdLabel") return "（USD / 1M tokens）";
          if (key === "modelPricing.pricingDate") return "2026年10月13日時点の価格";
          if (key === "modelPricing.colProvider") return "プロバイダ";
          if (key === "modelPricing.colModel") return "モデル";
          if (key === "modelPricing.colInput") return "入力/1M";
          if (key === "modelPricing.colOutput") return "出力/1M";
          if (key === "modelPricing.colCachedInput") return "キャッシュ/1M";
          if (key === "modelPricing.colNotes") return "備考";
          if (key === "modelPricing.disclaimer") return "料金は参考です";
          return key;
        },
        lang: "ja",
      }));
    });

    afterEach(() => {
      vi.restoreAllMocks();
      vi.useRealTimers();
    });

    it("provides translations for both updateDue and expired keys in all eight supported locales", () => {
      const locales = [
        { code: "en", map: en },
        { code: "ja", map: ja },
        { code: "zh-CN", map: zhCN },
        { code: "zh-TW", map: zhTW },
        { code: "ko", map: ko },
        { code: "de", map: de },
        { code: "fr", map: fr },
        { code: "es", map: es },
      ];

      for (const { code, map } of locales) {
        const updateDue = (map as Record<string, string>)["modelPricing.chinaHolidays.updateDue"];
        const expired = (map as Record<string, string>)["modelPricing.chinaHolidays.expired"];
        expect(updateDue, `Missing updateDue in ${code}`).toBeDefined();
        expect(updateDue).toContain("{year}");
        expect(expired, `Missing expired in ${code}`).toBeDefined();
        expect(expired).toContain("{year}");
      }
    });

    it("does not display calendar maintenance warning when status is current (2026-11-30)", () => {
      render(
        <ModelPricingAccordion
          initialNow={new Date(2026, 10, 30, 12, 0, 0)}
          calendars={{ 2026: CHINA_HOLIDAYS_2026 }}
        />,
      );
      expect(screen.queryByTestId("deepseek-calendar-warning")).not.toBeInTheDocument();
    });

    it("displays update_due warning only in the pricing section when December 1 arrives", () => {
      render(
        <ModelPricingAccordion
          initialNow={new Date(2026, 11, 1, 10, 0, 0)}
          calendars={{ 2026: CHINA_HOLIDAYS_2026 }}
        />,
      );
      const warning = screen.getByTestId("deepseek-calendar-warning");
      expect(warning).toBeInTheDocument();
      expect(warning.textContent).toContain("2027");
    });

    it("displays expired warning when current calendar year data is missing", () => {
      render(
        <ModelPricingAccordion
          initialNow={new Date(2027, 0, 1, 10, 0, 0)}
          calendars={{ 2026: CHINA_HOLIDAYS_2026 }}
        />,
      );
      const warning = screen.getByTestId("deepseek-calendar-warning");
      expect(warning).toBeInTheDocument();
      expect(warning.textContent).toContain("2027");
    });

    it("refreshes status at next local midnight when view is left open", () => {
      vi.useFakeTimers();
      // Set system time to 10 seconds before midnight on 2026-11-30
      const beforeMidnight = new Date(2026, 10, 30, 23, 59, 50);
      vi.setSystemTime(beforeMidnight);

      render(
        <ModelPricingAccordion
          calendars={{ 2026: CHINA_HOLIDAYS_2026 }}
        />,
      );
      // On 2026-11-30, status is current -> no warning
      expect(screen.queryByTestId("deepseek-calendar-warning")).not.toBeInTheDocument();

      // Advance system time past midnight into Dec 1
      act(() => {
        vi.setSystemTime(new Date(2026, 11, 1, 0, 0, 5));
        vi.advanceTimersByTime(15000); // 15 seconds passes midnight
      });

      // Now Dec 1 -> update_due warning is displayed
      const warning = screen.getByTestId("deepseek-calendar-warning");
      expect(warning).toBeInTheDocument();
      expect(warning.textContent).toContain("2027");
    });

    it("refreshes status immediately on returning to foreground (focus / visibilitychange) after date change", () => {
      vi.useFakeTimers();
      const daytimeNov30 = new Date(2026, 10, 30, 15, 0, 0);
      vi.setSystemTime(daytimeNov30);

      render(
        <ModelPricingAccordion
          calendars={{ 2026: CHINA_HOLIDAYS_2026 }}
        />,
      );
      expect(screen.queryByTestId("deepseek-calendar-warning")).not.toBeInTheDocument();

      // Advance system time to Dec 1 without advancing timer
      vi.setSystemTime(new Date(2026, 11, 1, 9, 0, 0));

      // Return to foreground via window focus
      act(() => {
        window.dispatchEvent(new Event("focus"));
      });

      // Should immediately display the warning
      const warning = screen.getByTestId("deepseek-calendar-warning");
      expect(warning).toBeInTheDocument();
      expect(warning.textContent).toContain("2027");
    });
  });
});
