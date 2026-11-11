import React, { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";

export interface PlanFolderLocationCardProps {
  surface: "antigravity" | "codex";
}

export default function PlanFolderLocationCard({ surface }: PlanFolderLocationCardProps) {
  const { t } = useTranslation();

  const [location, setLocation] = useState<string | null>(null);
  const [inputValue, setInputValue] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedMessage, setSavedMessage] = useState(false);

  const fetchLocation = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await invoke<string | null>("get_plan_manager_location", { surface });
      setLocation(res);
      setInputValue(res ?? "");
    } catch (e: unknown) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [surface]);

  useEffect(() => {
    void fetchLocation();
  }, [fetchLocation]);

  const saveLocation = async (rawPath: string | null) => {
    const trimmed = rawPath && rawPath.trim() !== "" ? rawPath.trim() : null;
    if (trimmed === location) {
      setInputValue(location ?? "");
      return;
    }

    try {
      setSaving(true);
      setError(null);
      setSavedMessage(false);
      const canonical = await invoke<string | null>("set_plan_manager_location", {
        surface,
        path: trimmed,
      });
      setLocation(canonical);
      setInputValue(canonical ?? "");
      setSavedMessage(true);
      setTimeout(() => setSavedMessage(false), 3000);
    } catch (e: unknown) {
      setError(String(e));
      setSavedMessage(false);
      setInputValue(location ?? "");
    } finally {
      setSaving(false);
    }
  };

  const handleSelectFolder = async () => {
    try {
      setSaving(true);
      setError(null);
      setSavedMessage(false);
      const picked = await invoke<string | null>("select_plan_root_dialog", { surface });
      if (picked) {
        // Route picked path through the canonical save and backend validation flow
        await saveLocation(picked);
      }
    } catch (e: unknown) {
      setError(String(e));
      setSavedMessage(false);
    } finally {
      setSaving(false);
    }
  };

  const handleResetLocation = async () => {
    await saveLocation(null);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      void saveLocation(inputValue);
    }
  };

  const handleBlur = () => {
    void saveLocation(inputValue);
  };

  const title =
    surface === "antigravity"
      ? t("planFolder.antigravityTitle") || "プランフォルダの場所 (Antigravity Autocode)"
      : t("planFolder.codexTitle") || "プランフォルダの場所 (Codex Autocode)";

  const placeholder =
    t("planFolder.inputPlaceholder") ||
    t("planFolder.notConfigured") ||
    "未設定（プロジェクトのパスを入力またはフォルダを選択）";

  return (
    <div className="settings-tile" data-testid={`plan-folder-location-card-${surface}`}>
      <div
        style={{
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
          marginBottom: 6,
          flexWrap: "wrap",
          gap: 8,
        }}
      >
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>
            {title}
          </h3>
          {(loading || saving) && (
            <span
              style={{ fontSize: 11, color: "#656d76" }}
              role="status"
              aria-live="polite"
              data-testid="plan-folder-loading"
            >
              ...
            </span>
          )}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {savedMessage && (
            <span
              style={{ fontSize: 12, color: "#1a7f37", fontWeight: 500 }}
              role="status"
              aria-live="polite"
              data-testid="saved-message"
            >
              ✓ {t("antigravity.savedMessage") || "設定を保存しました"}
            </span>
          )}
        </div>
      </div>

      <p style={{ margin: "0 0 10px 0", fontSize: 12, color: "#656d76" }}>
        {t("planFolder.desc") ||
          "Autocode 画面の「プランフォルダ」ボタンを押したときに開く基準プロジェクトディレクトリを設定します。"}
      </p>

      {error && (
        <div
          role="alert"
          aria-live="assertive"
          style={{
            margin: "0 0 10px 0",
            padding: "8px 12px",
            background: "#ffebe9",
            border: "1px solid #ff8182",
            borderRadius: 6,
            fontSize: 12,
            color: "#cf222e",
          }}
          data-testid="plan-folder-error"
        >
          {error}
        </div>
      )}

      <div
        style={{
          display: "flex",
          alignItems: "center",
          flexWrap: "wrap",
          gap: 8,
          width: "100%",
        }}
        data-testid="plan-folder-controls-row"
      >
        <input
          type="text"
          value={inputValue}
          onChange={(e) => setInputValue(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={handleBlur}
          placeholder={placeholder}
          disabled={loading || saving}
          aria-label={title}
          style={{
            flex: "1 1 200px",
            minWidth: 0,
            padding: "6px 10px",
            fontSize: 12,
            fontFamily: inputValue ? "monospace" : "inherit",
            border: "1px solid #d0d7de",
            borderRadius: 6,
            background: "var(--bg-input, #ffffff)",
            color: "var(--text-primary, #24292f)",
            outline: "none",
            boxSizing: "border-box",
          }}
          data-testid="input-plan-folder-location"
        />
        <div style={{ display: "flex", alignItems: "center", gap: 8, flexShrink: 0 }}>
          <button
            type="button"
            className="btn btn-primary"
            style={{ fontSize: 11, padding: "6px 12px", whiteSpace: "nowrap", flexShrink: 0 }}
            onClick={handleSelectFolder}
            disabled={loading || saving}
            data-testid="btn-select-plan-root"
          >
            📁 {t("planFolder.btnSelectFolder") || "フォルダを選択"}
          </button>
          {(location || inputValue) && (
            <button
              type="button"
              className="btn"
              style={{ fontSize: 11, padding: "6px 10px", whiteSpace: "nowrap", flexShrink: 0 }}
              onClick={handleResetLocation}
              disabled={loading || saving}
              data-testid="btn-reset-plan-root"
            >
              {t("planFolder.btnReset") || "リセット"}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
