import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import { MODEL_PRICING, PROVIDER_PRICE_ORDER } from "../config/modelPricing";
import { PROVIDER_MODELS } from "../modelCapabilities";
import { getModelDisplayName } from "../config/modelDisplayNames";
import { getChinaCalendarStatus, ChinaHolidayYearData } from "../config/chinaHolidays";
import {
  getLocalTimezone,
  formatDeepSeekPeakHoursLabel,
} from "../config/deepseekSchedule";

const TH_BASE: React.CSSProperties = {
  padding: "8px 10px",
  fontSize: 13,
  fontWeight: 700,
  color: "#374151",
  textAlign: "left",
  borderBottom: "1px solid #d1d5db",
  whiteSpace: "nowrap",
};

const TH_RIGHT: React.CSSProperties = {
  ...TH_BASE,
  textAlign: "right",
};

const TD_BASE: React.CSSProperties = {
  padding: "8px 10px",
  fontSize: 12,
  color: "#111827",
  borderBottom: "1px solid #e5e7eb",
};

const TD_RIGHT: React.CSSProperties = {
  ...TD_BASE,
  textAlign: "right",
  whiteSpace: "normal",
  fontVariantNumeric: "tabular-nums",
};

export function formatPrice(value: number | null | undefined, decimals: number): string {
  return value == null ? "—" : `$${value.toFixed(decimals)}`;
}

export function PriceCell({
  current,
  regular,
  decimals,
}: {
  current: number | null | undefined;
  regular?: number | null;
  decimals: number;
}) {
  const { t } = useTranslation();
  const currentText = formatPrice(current, decimals);

  if (regular == null) {
    return <span>{currentText}</span>;
  }

  const regularText = formatPrice(regular, decimals);
  const accessibleText = t("modelPricing.discountedPriceAria", {
    current: currentText,
    regular: regularText,
  });

  return (
    <span className="model-pricing-price-stack">
      <span className="sr-only">{accessibleText}</span>
      <span aria-hidden="true"><strong>{currentText}</strong></span>
      <s aria-hidden="true">{regularText}</s>
    </span>
  );
}

const TD_MONO: React.CSSProperties = {
  ...TD_BASE,
  fontFamily: "var(--font-mono)",
  fontSize: 12,
};

const TD_NOTES: React.CSSProperties = {
  ...TD_BASE,
  fontSize: 11,
  color: "#6b7280",
  lineHeight: 1.4,
  whiteSpace: "normal",
};

const EVEN_ROW_BG = "#fafafa";

const DEEPSEEK_PEAK_NOTE_KEY = "modelPricing.notes.deepseekPeakValley";

export interface ModelPricingAccordionProps {
  initialNow?: Date;
  calendars?: Record<number, ChinaHolidayYearData>;
}

export default function ModelPricingAccordion({
  initialNow,
  calendars,
}: ModelPricingAccordionProps = {}) {
  const { t, lang } = useTranslation();
  const [hoveredRow, setHoveredRow] = useState<number | null>(null);
  const [tzId, setTzId] = useState<string>(getLocalTimezone);
  const [now, setNow] = useState<Date>(() => initialNow ?? new Date());

  useEffect(() => {
    if (initialNow) {
      setNow(initialNow);
    }
  }, [initialNow]);

  useEffect(() => {
    const currentDate = new Date();
    const nextMidnight = new Date(
      currentDate.getFullYear(),
      currentDate.getMonth(),
      currentDate.getDate() + 1,
      0,
      0,
      0,
      0,
    );
    const msUntilMidnight = Math.max(1000, nextMidnight.getTime() - currentDate.getTime());

    const timer = setTimeout(() => {
      setNow(new Date());
    }, msUntilMidnight);

    const onForeground = () => {
      setNow(new Date());
    };
    const onVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        setNow(new Date());
      }
    };

    window.addEventListener("focus", onForeground);
    document.addEventListener("visibilitychange", onVisibilityChange);

    return () => {
      clearTimeout(timer);
      window.removeEventListener("focus", onForeground);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [now]);

  useEffect(() => {
    invoke<string | null>("get_pricing_display_timezone")
      .then((saved) => { if (saved) setTzId(saved); })
      .catch(() => {});
  }, []);

  // Build flat row list: provider × model
  const rows: Array<{
    provider: string;
    displayName: string;
    model: string;
    input: number;
    output: number;
    cached: number | null;
    regularInput?: number | null;
    regularOutput?: number | null;
    regularCached?: number | null;
    noteKey?: string;
    noteKeys?: string[];
  }> = [];

  for (const providerId of PROVIDER_PRICE_ORDER) {
    const models =
      providerId === "zai"
        ? ["zai:glm-5.3", "zai:glm-5.3-flash", "zai:glm-5.3-flashx", "zai:glm-5.2"]
        : PROVIDER_MODELS[providerId] ?? [];
    const displayName =
      providerId === "deepseek"
        ? "DeepSeek"
        : providerId === "openrouter"
          ? "OpenRouter"
          : providerId === "minimax"
            ? "MiniMax"
            : providerId === "kimi"
              ? "Kimi"
              : providerId === "mimo"
                ? "MiMo"
                : providerId === "zai"
                  ? "GLM"
                  : providerId;

    for (const model of models) {
      const p = MODEL_PRICING[model];
      if (!p) continue;
      rows.push({
        provider: providerId,
        displayName,
        model,
        input: p.inputPerMillionUsd,
        output: p.outputPerMillionUsd,
        cached: p.cachedInputPerMillionUsd ?? null,
        regularInput: p.regularInputPerMillionUsd,
        regularOutput: p.regularOutputPerMillionUsd,
        regularCached: p.regularCachedInputPerMillionUsd,
        noteKey: p.pricingNoteKey,
        noteKeys: p.pricingNoteKeys,
      });
    }
  }

  const calendarStatus = getChinaCalendarStatus(now, calendars);
  const calendarWarning =
    calendarStatus.status === "update_due" && calendarStatus.targetYear != null
      ? t("modelPricing.chinaHolidays.updateDue", { year: calendarStatus.targetYear })
      : calendarStatus.status === "expired" && calendarStatus.targetYear != null
        ? t("modelPricing.chinaHolidays.expired", { year: calendarStatus.targetYear })
        : null;

  return (
    <div className="settings-tile">
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 8,
          marginBottom: 10,
          padding: "2px 0",
        }}
      >
        <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>{t("modelPricing.header")}</h3>
        <span style={{ fontSize: 12, color: "#6b7280" }}>{t("modelPricing.usdLabel")}</span>
        <span style={{ fontSize: 11, color: "#9ca3af" }}>{t("modelPricing.pricingDate")}</span>
        <span style={{ flex: 1 }} />
      </div>

      {calendarWarning && rows.some((r) => r.provider === "deepseek") && (
        <div
          role="status"
          data-testid="deepseek-calendar-warning"
          style={{
            margin: "6px 0 10px 0",
            padding: "8px 12px",
            fontSize: 12,
            color: "#92400e",
            backgroundColor: "#fef3c7",
            borderRadius: 4,
            border: "1px solid #fde68a",
            lineHeight: 1.4,
          }}
        >
          {calendarWarning}
        </div>
      )}

      <div style={{ marginTop: 8 }}>
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr>
              <th style={{ ...TH_BASE, width: 90 }}>{t("modelPricing.colProvider")}</th>
              <th style={{ ...TH_BASE, width: 220 }}>{t("modelPricing.colModel")}</th>
              <th style={{ ...TH_RIGHT, width: 90 }}>{t("modelPricing.colInput")}</th>
              <th style={{ ...TH_RIGHT, width: 90 }}>{t("modelPricing.colOutput")}</th>
              <th style={{ ...TH_RIGHT, width: 100 }}>{t("modelPricing.colCachedInput")}</th>
              <th style={TH_BASE}>{t("modelPricing.colNotes")}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r, i) => {
              const isEven = i % 2 === 1;
              const isHovered = hoveredRow === i;
              const rowBg = isHovered ? "#f9fafb" : isEven ? EVEN_ROW_BG : "#fff";
              return (
                <tr
                  key={`${r.provider}-${r.model}`}
                  style={{ background: rowBg }}
                  onMouseEnter={() => setHoveredRow(i)}
                  onMouseLeave={() => setHoveredRow(null)}
                >
                  <td style={TD_BASE}>{r.displayName}</td>
                  <td style={TD_MONO}>{getModelDisplayName(r.model, r.provider)}</td>
                  <td style={TD_RIGHT}>
                    <PriceCell current={r.input} regular={r.regularInput} decimals={3} />
                  </td>
                  <td style={TD_RIGHT}>
                    <PriceCell current={r.output} regular={r.regularOutput} decimals={3} />
                  </td>
                  <td style={TD_RIGHT}>
                    <PriceCell current={r.cached} regular={r.regularCached} decimals={4} />
                  </td>
                  <td style={TD_NOTES}>
                    {(() => {
                      const noteKeys = r.noteKeys && r.noteKeys.length > 0
                        ? r.noteKeys
                        : r.noteKey
                          ? [r.noteKey]
                          : [];
                      return noteKeys.map((key) => {
                        if (key === DEEPSEEK_PEAK_NOTE_KEY) {
                          const peakHours = formatDeepSeekPeakHoursLabel(now, tzId, lang);
                          const prefix = t("modelPricing.notes.deepseekPeakValleyPrefix");
                          return <div key={key}>{prefix} {peakHours}。</div>;
                        }
                        return <div key={key}>{t(key as any)}</div>;
                      });
                    })()}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <div style={{ fontSize: 11, color: "#9ca3af", marginTop: 6, lineHeight: 1.4 }}>
        {t("modelPricing.disclaimer")}
      </div>
    </div>
  );
}
