import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "../i18n";
import type { AgentOptCliInfo, NerfEngineOverview } from "../types";

export default function AgentOptCliSettingsCard() {
  const { t } = useTranslation();
  const [cli, setCli] = useState<AgentOptCliInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const overview = await invoke<NerfEngineOverview>("get_antigravity_nerf_engine_overview");
      setCli(overview.cli);
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  const install = async () => {
    setBusy(true);
    setError(null);
    let installError: string | null = null;
    try {
      const result = await invoke<AgentOptCliInfo>("install_agent_opt_cli");
      setCli(result);
      setConfirm(false);
    } catch (e) {
      installError = String(e);
    } finally {
      setBusy(false);
      await refresh();
      if (installError) setError(installError);
    }
  };

  return (
    <section className="settings-tile" data-testid="agentopt-cli-settings-card">
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
        <div>
          <h3 style={{ margin: 0, fontSize: 14, fontWeight: 700, color: "var(--text-primary)" }}>{t("agentOpt.cliTitle", { version: cli?.pinned_version ?? "0.1.0" })}</h3>
          <p style={{ margin: "4px 0 0", fontSize: 12, color: "#656d76" }}>
            {loading ? t("agentOpt.checking") : cli?.installed
              ? t("agentOpt.cliInstalled", { version: cli.version ?? "unknown", node: cli.node_version ?? "unknown" })
              : t("agentOpt.cliNotInstalled", { node: cli?.node_version ?? "not detected" })}
          </p>
        </div>
        {cli && (!cli.installed || !cli.supported_version) && (
          <button type="button" className="btn btn-primary" data-testid="agentopt-cli-install"
            disabled={loading || busy || !cli.node_supported || !cli.npm_available}
            onClick={() => setConfirm((value) => !value)}>
            {cli.installed ? t("agentOpt.reinstallCli") : t("agentOpt.installCli")}
          </button>
        )}
      </div>
      {cli?.error && <div style={{ marginTop: 6, fontSize: 11, color: "#cf222e" }}>{cli.error}</div>}
      {error && <div role="alert" style={{ marginTop: 8, color: "#cf222e", fontSize: 12 }}>{error}</div>}
      {confirm && cli && (
        <div data-testid="agentopt-cli-install-confirm" style={{ marginTop: 10, padding: "10px 12px", backgroundColor: "#f6f8fa", border: "1px solid #d0d7de", borderRadius: 6, fontSize: 12 }}>
          <div style={{ fontWeight: 600, marginBottom: 4 }}>{t("agentOpt.installConfirmTitle")}</div>
          <ul style={{ margin: "0 0 8px 16px", padding: 0, color: "#57606a", fontSize: 11 }}>
            <li>{t("agentOpt.nodeRequirement", { node: cli.node_version ?? "unknown" })}</li>
            <li>{t("agentOpt.pinnedVersion", { version: cli.pinned_version })}</li>
            <li>{t("agentOpt.platformScope")}</li>
            <li>{t("agentOpt.installTarget")}</li>
          </ul>
          <div style={{ display: "flex", gap: 8 }}>
            <button type="button" className="btn btn-primary" disabled={busy} onClick={() => void install()}>{t("agentOpt.confirmInstall", { version: cli.pinned_version })}</button>
            <button type="button" className="btn" disabled={busy} onClick={() => setConfirm(false)}>{t("agentOpt.cancel")}</button>
          </div>
        </div>
      )}
    </section>
  );
}
