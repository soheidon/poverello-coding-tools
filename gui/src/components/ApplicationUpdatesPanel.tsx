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
    setPortalTarget(visible ? document.getElementById("application-updates-slot") : null);
    setNoticeTarget(document.getElementById("application-update-notice-slot"));
  }, [visible]);

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

  const target = visible ? portalTarget : null;

  return <>
    {target && createPortal((
    <section className="settings-section" data-testid="application-updates-panel">
      <h2>{t("updater.title")}</h2>
      <p>{t("updater.description")}</p>
      <p>{t("updater.currentVersion", { version: runtime?.currentVersion ?? "…" })}</p>
      {runtimeError ? (
        <p role="alert">{t("updater.statusUnavailable", { error: runtimeError })}</p>
      ) : !runtime ? (
        <p role="status">{t("updater.loadingConfiguration")}</p>
      ) : !runtime.configured ? (
        <p role="status">{
          runtime.reason === "development_disabled" ? t("updater.developmentDisabled") :
          runtime.reason === "staging_misconfigured" ? t("updater.stagingMisconfigured") :
          runtime.reason === "stable_misconfigured" ? t("updater.stableMisconfigured") :
          t("updater.configurationUnavailable")
        }</p>
      ) : (
        <>
          {state === "available" && update && (
            <div>
              <p>{t("updater.available", { version: update.version })}</p>
              {update.body && <pre className="updater-release-notes">{update.body}</pre>}
              <button type="button" onClick={() => void install()} disabled={installing}>
                {t("updater.downloadInstall")}
              </button>
              <button type="button" onClick={() => { void update.close(); setUpdate(null); setState("idle"); }} disabled={installing}>
                {t("updater.later")}
              </button>
            </div>
          )}
          {state === "current" && <p role="status">{t("updater.current")}</p>}
          {state === "error" && <p role="alert">{t("updater.checkFailed", { error: error ?? "" })}</p>}
          {installing && <p role="status">{t("updater.installing", { progress: progress === null ? "" : `${progress}%` })}</p>}
          <button type="button" onClick={() => void checkForUpdates()} disabled={state === "checking" || installing}>
            {state === "checking" ? t("updater.checking") : t("updater.check")}
          </button>
          <p>{t("updater.lastChecked", { time: lastChecked ?? t("updater.never") })}</p>
        </>
      )}
    </section>
    ), target)}
    {!visible && noticeTarget && state === "available" && update && createPortal((
      <aside className="updater-notice" role="status" data-testid="application-update-notice">
        <span>{t("updater.available", { version: update.version })}</span>
        <button type="button" onClick={onOpenSettings}>{t("updater.viewDetails")}</button>
        <button type="button" onClick={() => { void update.close(); setUpdate(null); setState("idle"); }}>{t("updater.later")}</button>
      </aside>
    ), noticeTarget)}
  </>;
}
