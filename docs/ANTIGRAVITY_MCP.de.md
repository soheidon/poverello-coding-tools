[English](ANTIGRAVITY_MCP.md) | [日本語](ANTIGRAVITY_MCP.ja.md) | [中文(简体)](ANTIGRAVITY_MCP.zh-CN.md) | [中文(繁體)](ANTIGRAVITY_MCP.zh-TW.md) | [한국어](ANTIGRAVITY_MCP.ko.md) | [Français](ANTIGRAVITY_MCP.fr.md) | [Deutsch](ANTIGRAVITY_MCP.de.md) | [Español](ANTIGRAVITY_MCP.es.md)

[← Zurück zur Poverello README](README.de.md)

# Poverello MCP mit Google Antigravity verwenden

Poverello erfordert keine separate ausführbare MCP-Serverdatei. Die installierte `anthro-bridge.exe` fungiert sowohl als Desktop-GUI-Anwendung als auch als MCP-Server. Antigravity startet den MCP-Modus durch Aufruf derselben Datei mit `--mcp-server`.

Der kanonische MCP-Servereintrag `poverello` stellt `poverello/plan`, `poverello/revise` und `poverello/review` bereit. Die ausführbare Datei `anthro-bridge.exe`, der alte Servereintrag `anthro-bridge` und die alten Tools `anthro-bridge/*` bleiben ausschließlich aus Kompatibilitätsgründen erhalten. Der Produktname lautet Poverello Coding Tools; in der App wird Poverello angezeigt.

```text
Normaler Start
anthro-bridge.exe
→ Poverello Desktop-App / 3P Gateway

MCP-Start
anthro-bridge.exe --mcp-server
→ Headless stdio MCP-Server für Antigravity
```

Dies ermöglicht es Agentenumgebungen wie Google Antigravity, Architektur- und Implementierungsplanung an externe LLMs (z. B. DeepSeek V4, MiMo, Kimi, MiniMax oder OpenRouter-Modelle) über `poverello/plan` zu delegieren, während die eigentlichen tokenintensiven Code-Änderungen, Terminalbefehle, Builds und Tests mit dem im Antigravity-Abonnement enthaltenen Modellkontingent durchgeführt werden.

---

## 1. Funktionsweise dieses Workflows

```text
Antigravity
    ↓ stdio
anthro-bridge.exe --mcp-server
    ↓
Konfiguriertes externes Planer-Modell
    ↓
Strukturierter Implementierungsplan wird zurückgegeben
    ↓
Antigravity führt Änderungen,
Builds und Tests über das Abonnement aus
```

- **Externe API**: Ausschließlich für die Generierung des Implementierungsplans basierend auf dem Repository-Kontext zuständig (wird vom jeweiligen Anbieter nutzungsbasiert abgerechnet).
- **Antigravity-Abonnement**: Übernimmt die aufwendigen Lese-/Schreibvorgänge, Code-Änderungen, Tool-Ausführungen und Testschleifen.
- **Aufgabenteilung**: Profitieren Sie von der überlegenen Planungsleistung externer Modelle, ohne API-Tokens für Routineaufgaben zu verschwenden.

---

## 2. Voraussetzungen

1. **Poverello** unter Windows installiert.
2. Provider-Authentifizierung in Poverello oder in den Systemumgebungsvariablen für den gewünschten Planer konfiguriert.
3. **Google Antigravity** installiert und aktiv.

---

## 3. MCP-Server in Antigravity konfigurieren

### Methode 1 — GUI-Konfiguration über Poverello (Empfohlen)

1. Öffnen Sie Poverello und wechseln Sie zu **Einstellungen** (Reiter `[Einstellungen]`) > linke Sub-Navigation **Antigravity**.
2. Überprüfen Sie die Karte **Google Antigravity Integration**:
   - **Ziel-Programmdatei**: Zeigt standardmäßig den Pfad der aktuell laufenden `anthro-bridge.exe` an. Um eine andere Binärdatei zu verwenden (z. B. portable oder eigene Builds), klicken Sie auf **Ändern** (`antigravity.btnChangeExe`) und wählen die Datei aus.
   - **Registrieren / Aktualisieren**: Klicken Sie auf **Antigravity-Konfiguration aktualisieren** (`antigravity.btnUpdate`), um den `anthro-bridge`-Eintrag in `%USERPROFILE%\.gemini\config\mcp_config.json` sicher einzutragen oder zu aktualisieren, während alle anderen MCP-Server erhalten bleiben.
   - **Entfernen**: Klicken Sie auf **Konfiguration entfernen** (`antigravity.btnRemove`), um den Server aus Antigravity zu deregistrieren.
   - **Ordner öffnen**: Klicken Sie auf **Einstellungsordner öffnen** (`antigravity.btnOpenFolder`), um das Verzeichnis im Windows Explorer zu öffnen.

---

### Methode 2 — Manuelle Konfiguration (Erweitert)

1. Klicken Sie in Poverello unter **Einstellungen > Antigravity** auf **Einstellungsordner öffnen**, um `%USERPROFILE%\.gemini\config\` im Windows Explorer zu öffnen.
2. Öffnen oder erstellen Sie `mcp_config.json` und fügen Sie `anthro-bridge` unter `mcpServers` hinzu:

```json
{
  "mcpServers": {
    "anthro-bridge": {
      "command": "C:\\Users\\<USER>\\AppData\\Local\\Poverello\\anthro-bridge.exe",
      "args": ["--mcp-server"]
    }
  }
}
```

Für Entwicklungs-Builds verweisen Sie direkt auf die Release-Ausführungsdatei:
```json
{
  "mcpServers": {
    "anthro-bridge": {
      "command": "C:\\Users\\<USER>\\path\\to\\poverello-coding-tools\\gui\\src-tauri\\target\\release\\anthro-bridge.exe",
      "args": ["--mcp-server"]
    }
  }
}
```

> [!TIP]
> Sie müssen **keine** API-Schlüssel in Antigravitys `mcp_config.json` eintragen. Der MCP-Server nutzt die bestehende Authentifizierungsauflösung von Poverello (automatisches Lesen aus Windows-Umgebungsvariablen wie `DEEPSEEK_API_KEY`, `OPENROUTER_API_KEY`, `MOONSHOT_API_KEY`, `MINIMAX_API_KEY`, `XIAOMI_API_KEY` oder den gespeicherten App-Einstellungen).

---

## 4. MCP-Verbindung überprüfen

Bestätigen Sie in der Ansicht **Installed MCP Servers** in Antigravity, dass `anthro-bridge` erkannt wurde:

```text
anthro-bridge
  1 tool enabled
  - plan
```

---

## 5. Planer-Modelle in Poverello konfigurieren

Poverello trennt die Planerauswahl klar von der detaillierten Parameterverwaltung:

1. **Oberster Reiter `MCP` (`MCP for Antigravity`)**:
   - Zeigt verfügbare Anbieter (DeepSeek, OpenRouter, MiniMax, MiMo (V2.6), Kimi und GLM) sowie Profile als Kacheln an.
   - Ein Klick auf eine Kachel wechselt das aktive Planer-Ziel sofort.
2. **`Einstellungen` > `Antigravity`**:
   - Karte **MCP Plan Detaileinstellungen**: Konfigurieren Sie Modell, Thinking-Modus und Reasoning Effort pro Anbieter/Profil.
   - Karte **Google Antigravity Integration**: Verwalten Sie die MCP-Serverregistrierung und die Antigravity Commands (Global Skills).

> [!NOTE]
> Der Poverello MCP-Server liest die aktuelle Konfiguration bei jedem `plan()`-Toolaufruf dynamisch ein. Sie müssen den MCP-Server oder Antigravity **nicht** neu starten, wenn Sie Einstellungen in der GUI ändern.

---

## 6. Antigravity Commands (`/pov-plan`, `/pov-revise` & `/pov-review`) verwenden (Empfohlen)

Über **Einstellungen > Antigravity > Google Antigravity Integration** können Sie Global Skills installieren, um Slash-Befehle in jedem Antigravity-Workspace zu nutzen:

- Klicken Sie auf **Alle installieren** (`antigravity.btnInstallAll`) oder auf **Installieren** (`antigravity.commandBtnInstall`) neben dem jeweiligen Befehl.

### Neuen Implementierungsplan erstellen:
```text
/pov-plan <Beschreibung der Aufgabe oder des Features>
```
*Sammelt den Repository-Kontext, ruft `poverello/plan` auf und stoppt sicher nach der Planpräsentation, ohne Dateien zu ändern oder Builds auszuführen.*

### Bestehenden Plan überarbeiten / Feedback einarbeiten:
```text
/pov-revise <Einzuarbeitendes Feedback oder geänderte Anforderungen>
```
*Identifiziert den aktuellen Plan (aktiver Kontext oder `implementation_plan.md`), übergibt Plan und Feedback an `poverello/plan` und aktualisiert den Plan unter Beibehaltung unveränderter Abschnitte.*

### Implementierung anhand des genehmigten Plans prüfen:
```text
/pov-review
```
*Prüft den genehmigten Plan, den Arbeitsbaum-Diff, relevante nicht verfolgte Dateien und Testergebnisse und berichtet die Implementierungsprüfung. Der Befehl ist schreibgeschützt und ändert keine Dateien.*

> [!IMPORTANT]
> Bei der Ausführung über `/pov-plan` oder `/pov-revise` verwaltet der Befehl den einzelnen Planeraufruf. Workspace Rules lösen keine zusätzlichen doppelten Aufrufe aus.

---

## 7. Planung mit Workspace Rules automatisieren

Sie können eine optionale projektspezifische Workspace-Regel nach dem folgenden Beispiel erstellen. Poverello liefert keine DeepSeek-spezifische Planerregel mit und installiert sie auch nicht; der konfigurierte MCP-Anbieter und das Modell werden in den Poverello-Einstellungen ausgewählt:

```markdown
---
trigger: model_decision
description: Use for implementation, debugging, architectural changes, or multi-file code changes where an external implementation plan would be useful. Do not use for trivial text-only edits.
---

# Poverello Planner Rule (Beispiel)

For non-trivial implementation tasks in this repository:

1. If the current task is being executed through the `/pov-plan` or `/pov-revise` command, do NOT invoke `poverello/plan` separately. The command workflow owns the planner call.
2. First inspect the repository yourself and identify the files and code relevant to the task.
3. Summarize only the context necessary for implementation planning.
4. Call the `poverello/plan` MCP tool exactly once with:
   - the user's task;
   - the relevant repository context;
   - important constraints.
   Note: "Exactly once" means duplicate planner calls are prohibited once a successful usable result is obtained. If the tool call itself fails or returns an unusable response (e.g. transport or decoding error), exactly 1 recovery retry is permitted.
5. Use the returned plan as the basis for implementation.
6. Perform file edits, builds, and tests yourself.
7. Do not call `poverello/plan` again unless the implementation encounters a major unresolved problem.
8. Do not ask the user to repeat this workflow.
9. Do not use the planner for trivial tasks such as a one-word text change unless planning would materially help.
```

### Auslöserichtlinie:
- **Triviale / lokale Aufgaben (Trivial / localized tasks)** (z. B. Tippfehler korrigieren, einzeilige Anpassungen, kleine Syntaxkorrekturen): Der Planer wird nicht ausgelöst.
- **Nicht-triviale Aufgaben (Non-trivial tasks)** (Architekturänderungen, dateiübergreifende Features, komplexes Debugging): Antigravity analysiert den Kontext, ruft 1x `poverello/plan` auf und führt die Implementierung auf Basis des Plans aus.

---

## 8. Typischer automatisierter Workflow

```text
Benutzer: "Refaktoriere Feature X für Multi-Profil-Unterstützung."
    ↓
Antigravity untersucht Code und fasst Kontext zusammen
    ↓
Antigravity löst automatisch den poverello/plan-Aufruf aus (genau 1x)
    ↓
Poverello sendet Prompt an das externe Modell
    ↓
Antigravity empfängt strukturierten Implementierungsplan
    ↓
Benutzer prüft und genehmigt den Plan
    ↓
Antigravity führt Codeänderungen durch und führt Tests aus
```

---

## 9. Wichtige Hinweise

- **Unabhängiger Betrieb**: Der MCP-Server arbeitet vollständig unabhängig vom Poverello 3P Gateway. Das 3P Gateway muss nicht aktiv (eingeschaltet) sein, um den MCP-Server zu nutzen.
- **Getrennte Abrechnung**: Aufrufe von `poverello/plan` verursachen API-Kosten beim jeweiligen Drittanbieter. Nachfolgende Codeänderungen und Tests nutzen Ihr Antigravity-Abonnement.
- **Sofortige Wirksamkeit**: Änderungen von Planer-Anbietern oder Modellparametern in der Poverello GUI gelten unmittelbar für den nächsten `plan()`-Aufruf.

---
> [!NOTE]
> **MCP-Planung und -Überprüfung nutzen Cloud-Provider**
>
> Antigravity MCP verwendet den in Poverello konfigurierten aktiven Cloud-Modellanbieter (`mcp.provider` / `mcp.targets`). Das **Ollama Local**-Backend ist ausschließlich **Claude Code CLI** vorbehalten und beeinflusst MCP-Operationen nicht. Für lokale Claude Code Inferenz siehe den [Ollama Local Leitfaden](OLLAMA_LOCAL.de.md).
