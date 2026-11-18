import type { TranslationKey } from "../i18n";

export interface AgentOptErrorPresentation {
  message: string;
  guidance: string | null;
}

export function isUnregisteredWorkspaceError(message: string): boolean {
  return message.startsWith("Cannot switch to AgentOpt:")
    && message.includes(" workspace '")
    && message.includes(" is not registered. Register it explicitly first.");
}

function normalizeWindowsPath(path: string): string {
  const readable = path.startsWith("\\\\?\\UNC\\")
    ? `\\\\${path.slice(8)}`
    : path.startsWith("\\\\?\\")
      ? path.slice(4)
      : path;
  return readable.replace(/\//g, "\\").replace(/[\\]+$/, "").toLowerCase();
}

export function unregisteredWorkspaceErrorMatchesPath(message: string, workspacePath: string): boolean {
  if (!isUnregisteredWorkspaceError(message)) return false;
  const marker = " workspace '";
  const start = message.indexOf(marker) + marker.length;
  const end = message.indexOf("' is not registered.", start);
  if (end < start) return false;
  return normalizeWindowsPath(message.slice(start, end)) === normalizeWindowsPath(workspacePath);
}

export function getAgentOptErrorPresentation(
  message: string,
  t: (key: TranslationKey) => string,
): AgentOptErrorPresentation {
  if (message === "Assign or clear the legacy manual workspace on a project card before enabling AgentOpt."
    || message.startsWith("A legacy manual workspace is not assigned to either project.")) {
    return {
      message: t("agentOpt.legacyManualMigrationRequired"),
      guidance: t("agentOpt.legacyManualMigrationSteps"),
    };
  }
  if (!isUnregisteredWorkspaceError(message)) {
    return { message, guidance: null };
  }

  return {
    message: t("agentOpt.workspaceNotRegistered"),
    guidance: t("agentOpt.workspaceNotRegisteredSteps"),
  };
}
