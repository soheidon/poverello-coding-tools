import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { useTranslation } from "../i18n";

interface RuntimeStatus {
  configured: boolean;
  currentVersion: string;
  channel: string;
  buildMode: "dev" | "staging" | "stable" | string;
  reason: "ready" | "development_disabled" | "staging_misconfigured" | "stable_misconfigured" | string;
}

type CheckState = "idle" | "checking" | "current" | "available" | "error";

export function formatLastChecked(isoString: string | null | undefined, fallbackText: string): string {
  if (!isoString) return fallbackText;
  const d = new Date(isoString);
  if (isNaN(d.getTime())) return fallbackText;
  const pad = (n: number) => String(n).padStart(2, "0");
  const year = d.getFullYear();
  const month = pad(d.getMonth() + 1);
  const day = pad(d.getDate());
  const hours = pad(d.getHours());
  const minutes = pad(d.getMinutes());
  return `${year}-${month}-${day} ${hours}:${minutes}`;
}

export function ReleaseNotesViewer({ markdown }: { markdown: string }) {
  if (!markdown || !markdown.trim()) return null;
  const lines = markdown.split(/\r?\n/);
  const elements: React.ReactNode[] = [];
  let listBuffer: string[] = [];

  const flushList = (keyPrefix: string) => {
    if (listBuffer.length > 0) {
      elements.push(
        <ul key={`${keyPrefix}-list`} className="updater-notes-list">
          {listBuffer.map((item, idx) => (
            <li key={idx}>{item}</li>
          ))}
        </ul>
      );
      listBuffer = [];
    }
  };

  lines.forEach((rawLine, idx) => {
    const line = rawLine.trim();
    if (!line) {
      flushList(`line-${idx}`);
      return;
    }

    if (line.startsWith("### ")) {
      flushList(`h3-${idx}`);
      elements.push(<h4 key={`h3-${idx}`} className="updater-notes-h4">{line.slice(4)}</h4>);
    } else if (line.startsWith("## ")) {
      flushList(`h2-${idx}`);
      elements.push(<h3 key={`h2-${idx}`} className="updater-notes-h3">{line.slice(3)}</h3>);
    } else if (line.startsWith("# ")) {
      flushList(`h1-${idx}`);
      elements.push(<h3 key={`h1-${idx}`} className="updater-notes-h3">{line.slice(2)}</h3>);
    } else if (line.startsWith("- ") || line.startsWith("* ")) {
      listBuffer.push(line.slice(2));
    } else {
      flushList(`p-${idx}`);
      elements.push(<p key={`p-${idx}`} className="updater-notes-p">{line}</p>);
    }
  });

  flushList("final");

  if (elements.length === 0) return null;

  return (
    <div className="updater-release-notes" data-testid="updater-release-notes">
      <div className="updater-notes-body">{elements}</div>
    </div>
  );
}

export default function ApplicationUpdatesPanel({
  visible = true,
  onOpenSettings,
}: {
  visible?: boolean;
  onOpenSettings?: () => void;
}) {
  const { t } = useTranslation();
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null);
  const [runtimeError, setRuntimeError] = useState<string | null>(null);
  const [state, setState] = useState<CheckState>("idle");
  const [update, setUpdate] = useState<Update | null>(null);
  const [lastChecked, setLastChecked] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [progress, setProgress] = useState<number | null>(null);
  const [portalTarget, setPortalTarget] = useState<HTMLElement | null>(null);
  const [noticeTarget, setNoticeTarget] = useState<HTMLElement | null>(null);
  const checkedOnce = useRef(false);
  const checkRef = useRef<(automatic?: boolean) => Promise<void>>(async () => {});
  const updateRef = useRef<Update | null>(null);

  const checkForUpdates = useCallback(async (automatic = false) => {
    if (!runtime?.configured || state === "checking" || installing) return;
    setState("checking");
    setError(null);
    try {
      const result = await check();
      setLastChecked(new Date().toISOString());
      setUpdate((previous) => {
        if (previous && previous !== result) void previous.close();
        return result;
      });
      setState(result ? "available" : "current");
    } catch (cause) {
      setState("error");
      setError(cause instanceof Error ? cause.message : String(cause));
    }
    void automatic;
  }, [runtime?.configured, state, installing]);

  checkRef.current = checkForUpdates;
  updateRef.current = update;

  useEffect(() => {
    let mounted = true;
    void invoke<RuntimeStatus>("get_updater_runtime_status").then((status) => {
      if (!mounted) return;
      setRuntime(status);
      if (status.configured && !checkedOnce.current) {
        checkedOnce.current = true;
        window.setTimeout(() => void checkRef.current(true), 0);
      }
    }).catch((cause) => {
      if (mounted) {
        setRuntimeError(cause instanceof Error ? cause.message : String(cause));
      }
    });
    return () => { mounted = false; };
  }, []);

  useEffect(() => () => { if (updateRef.current) void updateRef.current.close(); }, []);

  useEffect(() => {
    if (!visible) {
      setPortalTarget(null);
      return;
    }

    const updateTarget = () => {
      const el = document.getElementById("application-updates-slot");
      setPortalTarget(el && el.isConnected ? el : null);
    };

    updateTarget();

    const observer = new MutationObserver(updateTarget);
    observer.observe(document.body, { childList: true, subtree: true });

    return () => {
      observer.disconnect();
    };
  }, [visible]);

  useEffect(() => {
    setNoticeTarget(document.getElementById("application-update-notice-slot"));
  }, []);

  const install = async () => {
    if (!update || installing) return;
    setInstalling(true);
    setProgress(0);
    try {
      let downloaded = 0;
      let total: number | undefined;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength;
        else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          setProgress(total && total > 0 ? Math.min(100, Math.round(downloaded * 100 / total)) : null);
        } else if (event.event === "Finished") setProgress(100);
      }, { restartAfterInstall: true });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      setInstalling(false);
    }
  };

  const handleLater = () => {
    if (update) {
      void update.close();
      setUpdate(null);
    }
    setState("idle");
  };

  const target = visible ? portalTarget : null;
  const formattedLastChecked = formatLastChecked(lastChecked, t("updater.never") || "未確認");

  return <>
    {target && createPortal((
      <section className="settings-tile updater-card" data-testid="application-updates-panel">
        <div style={{ display: "flex", alignItems: "baseline", gap: 10 }}>
          <h3 style={{ margin: 0 }}>{t("updater.title")}</h3>
          <p className="tile-desc" style={{ margin: 0 }}>{t("updater.description")}</p>
        </div>

        <div className="updater-info-row">
          <span className="updater-version">
            {t("updater.currentVersion", { version: runtime?.currentVersion ?? "…" })}
          </span>
        </div>

        {/* 9-Tier Precedence State Display */}
        {runtimeError ? (
          <p role="alert" className="updater-alert">
            {t("updater.statusUnavailable", { error: runtimeError })}
          </p>
        ) : !runtime ? (
          <p role="status" className="updater-status-msg">
            {t("updater.loadingConfiguration")}
          </p>
        ) : !runtime.configured ? (
          <p role="status" className="updater-status-msg">
            {runtime.reason === "development_disabled" ? t("updater.developmentDisabled") :
             runtime.reason === "staging_misconfigured" ? t("updater.stagingMisconfigured") :
             runtime.reason === "stable_misconfigured" ? t("updater.stableMisconfigured") :
             t("updater.configurationUnavailable")}
          </p>
        ) : installing ? (
          <div role="status">
            <p className="updater-status-msg">
              {t("updater.installing", { progress: progress === null ? "" : `${progress}%` })}
            </p>
            <div className="tile-actions" style={{ marginTop: 8 }}>
              <button type="button" className="btn btn-small" disabled>
                {t("updater.checking")}
              </button>
            </div>
          </div>
        ) : state === "checking" ? (
          <div>
            <p role="status" className="updater-status-msg">
              {t("updater.checking")}
            </p>
            <div className="tile-actions" style={{ marginTop: 8 }}>
              <button type="button" className="btn btn-small" disabled>
                {t("updater.checking")}
              </button>
            </div>
          </div>
        ) : state === "available" && update ? (
          <div className="updater-available-box">
            <p className="updater-available-badge" role="status">
              {t("updater.available", { version: update.version })}
            </p>
            {update.body && <ReleaseNotesViewer markdown={update.body} />}
            <div className="tile-actions" style={{ marginTop: 10 }}>
              <button
                type="button"
                className="btn btn-small btn-primary"
                onClick={() => void install()}
                disabled={installing}
              >
                {t("updater.downloadInstall")}
              </button>
              <button
                type="button"
                className="btn btn-small"
                onClick={handleLater}
                disabled={installing}
              >
                {t("updater.later")}
              </button>
              <button
                type="button"
                className="btn btn-small"
                onClick={() => void checkForUpdates()}
                disabled={installing}
              >
                {t("updater.check")}
              </button>
            </div>
          </div>
        ) : state === "error" ? (
          <div>
            <p role="alert" className="updater-alert">
              {t("updater.checkFailed", { error: error ?? "" })}
            </p>
            <div className="tile-actions" style={{ marginTop: 8 }}>
              <button
                type="button"
                className="btn btn-small"
                onClick={() => void checkForUpdates()}
              >
                {t("updater.check")}
              </button>
            </div>
          </div>
        ) : state === "current" ? (
          <div>
            <p role="status" className="updater-status-success">
              {t("updater.current")}
            </p>
            <div className="tile-actions" style={{ marginTop: 8 }}>
              <button
                type="button"
                className="btn btn-small"
                onClick={() => void checkForUpdates()}
              >
                {t("updater.check")}
              </button>
            </div>
          </div>
        ) : (
          <div>
            <p role="status" className="updater-status-msg">
              {t("updater.never")}
            </p>
            <div className="tile-actions" style={{ marginTop: 8 }}>
              <button
                type="button"
                className="btn btn-small"
                onClick={() => void checkForUpdates()}
              >
                {t("updater.check")}
              </button>
            </div>
          </div>
        )}

        <div className="updater-footer">
          <small className="tile-desc" style={{ margin: 0 }}>
            {t("updater.lastChecked", { time: formattedLastChecked })}
          </small>
        </div>
      </section>
    ), target)}

    {!visible && noticeTarget && state === "available" && update && createPortal((
      <aside className="updater-notice" role="status" data-testid="application-update-notice">
        <span>{t("updater.available", { version: update.version })}</span>
        <button type="button" onClick={onOpenSettings}>{t("updater.viewDetails")}</button>
        <button type="button" onClick={handleLater}>{t("updater.later")}</button>
      </aside>
    ), noticeTarget)}
  </>;
}
