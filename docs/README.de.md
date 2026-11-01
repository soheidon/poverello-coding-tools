[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

**Verwende Claude Code / Claude Desktop als Entwicklungsumgebung, route Inferenz an konfigurierte Cloud- oder lokale Modelle und verwalte externe Modelle und Regeln für Google Antigravity.**

Poverello Coding Tools (in der App als **Poverello** angezeigt) ist eine Windows-Begleitanwendung für KI-gestützte Softwareentwicklung. Es unterstützt vier sich ergänzende Arbeitsabläufe:

1. **3P-Gateway für Claude Code / Claude Desktop** — Behalte Claudes Repository-Erkundung, Tool-Nutzung, Dateibearbeitung und Testausführung bei, während die Inferenz an Drittanbieter weitergeleitet wird.
2. **Dediziertes lokales LLM für Claude Code (Ollama)** — Führe Claude Code mit lokalen Modellen offline und ohne API-Kosten aus.
3. **MCP-Planer & Reviewer für Google Antigravity** — Delegiere Implementierungsplanung, Planrevision und Nachimplementierungsüberprüfung über die MCP-Tools `poverello/plan`, `poverello/revise` und `poverello/review` an externe Modelle.
4. **Antigravity Autocode und Skills & Regeln** — Konfigurieren Sie externe Reviews und das Codierverhalten im Tab Antigravity Autocode; verwalten Sie Workflow Commands, CLI Tools, Task Presets, Global Rules und Nerf-Skills unter Settings.

> [!TIP]
> **Neuigkeiten:** Versionsbezogene Funktionen, Implementierungsdetails und Änderungsprotokolle findest du in den [Release Notes (NEWS.md)](../NEWS.md).

---

## Vier Hauptarbeitsabläufe

### 1. Claude Code / Claude Desktop mit 3P-Gateway

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P-Gateway
             ↓
Konfigurierter Cloud- oder lokaler Anbieter
```

- **Trennung von Umgebung und Modell**: Behalte Claudes agentische Tooling-Infrastruktur, während die Inferenz an Drittanbieter weitergeleitet wird.
- **Dynamisches Multi-Profil-Routing**: Wechsle aktive Anbieter, OpenRouter-Profile und Modellrouten über die grafische Oberfläche.
- **Einrichtungsanleitung**: [Claude Desktop / Cowork 3P-Gateway-Einrichtung](THIRD_PARTY_INFERENCE.de.md)

### 2. Dedizierte lokale LLMs für Claude Code (Ollama)

```text
Claude Code CLI
      ↓ (Loopback ANTHROPIC_BASE_URL)
Ollama Local (127.0.0.1:11434/v1)
      ↓
Gemma 4 / Qwen / Llama 3 / DeepSeek-R1 (Lokal auf dem Gerät)
```

- **Kostenlos, offline und privat**: Führen Sie Claude Code ohne API-Schlüssel, Nutzungslimits oder Cloud-Datenübertragung aus.
- **Dediziertes Routing und Modell-Erkennung**: Separate Claude Code Route (`active_route: "ollama"`) mit automatischer Modellsuche über `/api/tags`.
- **Denkmodus und Kontext-Anpassung**: Volle Unterstützung für Denk-Token-Budgets (1024 Tokens) und Kontextfenster-Überschreibungen (`r`num_ctx`) pro Alias.
- **Vollständige Isolation**: Claude Desktop, Cowork on 3P und Google Antigravity MCP nutzen weiterhin unabhängig Cloud-Modelle.
- **Einrichtungsleitfaden**: [Ollama Local Leitfaden](OLLAMA_LOCAL.de.md)

### 3. Antigravity mit MCP-Planer und -Reviewer

```text
Antigravity
    ↓ stdio
anthro-bridge.exe --mcp-server
    ↓
Konfiguriertes externes Modell (Planer / Reviewer)
    ↓
Implementierungsplan / Review-Urteil
    ↓
Antigravity implementiert und testet
unter Nutzung abonnementbasierter Kapazität
```

- **Trennung von Planung und Ausführung**: Externe Modelle erstellen den übergeordneten Plan oder das Review-Urteil; die Antigravity-Abonnementkapazität führt tokenintensive Code-Änderungen durch.
- **Live-GUI-Konfiguration**: Das Wechseln des Planer- oder Reviewer-Anbieters, Modells oder Reasoning-Aufwands wird beim nächsten Aufruf sofort wirksam.
- **Einrichtungsanleitung**: [Google Antigravity + Poverello MCP-Einrichtung](ANTIGRAVITY_MCP.de.md)

Die ausführbare Datei und die MCP-Toolnamen behalten aus Kompatibilitätsgründen den Bezeichner `anthro-bridge`.

Die kanonischen Antigravity-Befehle sind `/pov-mcp-plan`, `/pov-mcp-revise` und `/pov-mcp-review`; die älteren `/anthro-*`-Befehle bleiben aus Kompatibilitätsgründen verfügbar.

**Globale Antigravity-Befehle:**

- **`/pov-mcp-plan`** — Delegiere die Implementierungsplanung an das konfigurierte externe Modell.
- **`/pov-mcp-revise`** — Überarbeite einen bestehenden Plan auf der Grundlage neuer Anforderungen oder Einschränkungen.
- **`/pov-mcp-review`** — Überprüfe eine abgeschlossene Implementierung anhand des genehmigten Plans vor dem Commit, mit expliziten READY / NOT READY-Urteilen.

Autonome Workflows: `/pov-full-cycle`, `/pov-plan-only`, `/pov-build-review` (Installation und Reparatur unter **Einstellungen > Antigravity Skills & Rules**; Reviewer Policy unter **Antigravity Autocode**).

**Empfohlener Arbeitsablauf:**

```text
/pov-mcp-plan → Implementierung & Tests → /pov-mcp-review → Commit
```

### 4. Antigravity Autocode und Skills-&-Rules-Verwaltung
```text
Antigravity Autocode tab
  • External Review policy / reviewer configuration
  • Customize Coding Settings / Direct Coding

Settings > Antigravity Skills & Rules
  • Workflow Commands / repair
  • CLI Tools / Task Presets
  • Global Rules / Nerf skills
```

- External Review und Customize Coding Settings werden in Antigravity Autocode konfiguriert. Settings verwaltet die drei Workflow-Skills, CLI Tools, Task Presets, Global Rules und Nerf-Skills.
## Installation

Lade das neueste derzeit veröffentlichte Windows-Installationsprogramm von der [Releases](https://github.com/soheidon/poverello-coding-tools/releases)-Seite herunter und führe es aus.

Das Windows-Installationsprogramm unterstützt 8 Sprachen. Poverello behält die bestehende Anwendungsidentität bei und unterstützt ein In-Place-Upgrade von Anthro Bridge. Beim ersten Start soll es vorhandene Einstellungen und Laufdaten aus `%APPDATA%\Anthro Bridge` (bei Entwicklungs-Builds `%APPDATA%\Anthro Bridge Dev`) in den entsprechenden Poverello-Datenordner kopieren und den ursprünglichen Ordner als Wiederherstellungskopie behalten. Die tatsächliche Windows-Installation, Migration, Aktualisierung, Neuinstallation, Abbruch- und Deinstallationsabläufe wurden nicht überprüft; sichern Sie wichtige Daten vor einem Upgrade.

---

## Schnellstart

### Arbeitsablauf 1: 3P-Gateway für Claude Code / Claude Desktop

1. Öffne Poverello unter **Einstellungen > API-Schlüssel** und konfiguriere einen API-Schlüssel für deinen gewünschten Anbieter.
2. Wähle deinen Anbieter oder dein OpenRouter-Profil im Dashboard aus.
3. Klicke auf **Gateway starten** (läuft auf `http://127.0.0.1:4000`).
4. Verbinde Claude Code oder Claude Desktop:
   - **Claude Code**: Klicke in den Einstellungen auf **Claude Code-Startbefehl kopieren** und füge ihn in PowerShell ein.
   - **Claude Desktop / Cowork**: Folge der [Claude Desktop 3P-Einrichtungsanleitung](THIRD_PARTY_INFERENCE.de.md)
- [Ollama Local](OLLAMA_LOCAL.de.md).

### Arbeitsablauf 2: Dedizierte lokale LLMs für Claude Code (Ollama)

1. Stelle sicher, dass Ollama lokal läuft (`http://127.0.0.1:11434`).
2. Wähle **Ollama Local** im Poverello-Dashboard oder konfiguriere Modelle unter **Einstellungen > Claude Code / Local LLMs**.
3. Klicke auf **Claude Code-Startbefehl kopieren** und füge ihn in PowerShell ein.
4. Claude Code verbindet sich direkt mit deiner lokalen Ollama-Instanz.

### Arbeitsablauf 3: MCP-Planer & Reviewer für Google Antigravity

1. Konfiguriere in Poverello einen API-Schlüssel für dein gewähltes Planer-/Reviewer-Modell.
2. Wähle den **MCP**-Tab (oder **Einstellungen > Antigravity MCP**) und konfiguriere dein Modell unter **Einstellungen > Antigravity MCP > MCP-Planeinstellungen**.
3. Registriere `anthro-bridge.exe` mit `["--mcp-server"]` unter dem Server-Schlüssel `poverello` in Antigravitys MCP-Konfiguration (oder klicke in Poverello auf **Automatisch konfigurieren**).
4. Verwende `/pov-mcp-plan` zum Erstellen von Plänen, `/pov-mcp-revise` zum Aktualisieren von Plänen und `/pov-mcp-review` zum Überprüfen von Implementierungen vor dem Commit.
4. Folge der vollständigen [Antigravity MCP-Einrichtungsanleitung](ANTIGRAVITY_MCP.de.md).

### Workflow 4: Antigravity Autocode und Skills & Regeln
1. Konfigurieren Sie External Review und Customize Coding Settings in **Antigravity Autocode**. Die Richtlinie steuert Reviewer-Aufrufe der Workflow-Skills.
2. Verwalten Sie Workflow Commands, CLI Tools, Task Presets, Global Rules und Nerf-Skills unter **Settings > Antigravity Skills & Rules**. Eine Skill-Reparatur erfordert Bestätigung und bewahrt eine Sicherung auf.
3. Auto-Converge bleibt ausgeblendet; der Produktions-Dispatch ist deaktiviert und die Stage-2-Isolierung nicht verifiziert.
## Dokumentation

- [Release Notes (NEWS.md)](../NEWS.md) — Versionsbezogene Funktionen und Implementierungsdetails
- [Claude Desktop / Cowork 3P-Gateway-Einrichtung](THIRD_PARTY_INFERENCE.de.md)
- [Ollama Local](OLLAMA_LOCAL.de.md)
- [Google Antigravity + Poverello MCP-Einrichtung](ANTIGRAVITY_MCP.de.md)
- [Konfigurationsreferenz (`config.json`)](CONFIGURATION.md)
- [Entwicklungs- & Verifikationsanleitung](DEVELOPMENT.md)

---

## Fehlerbehebung

### Port 4000 ist bereits belegt
```powershell
netstat -ano | findstr :4000
taskkill /PID <PID> /F
```

### Einstellungen werden nach einem Upgrade zurückgesetzt
Starte die Anwendung neu, damit Migrationen ausgeführt werden können. Die Konfiguration wird unter `%APPDATA%\Poverello\config.json` gespeichert.

### MCP-Planer-Aufrufe schlagen fehl
Stelle sicher, dass ein API-Schlüssel für den unter dem **MCP**-Tab ausgewählten Anbieter gesetzt ist oder in deinen Windows-Benutzerumgebungsvariablen exportiert wurde (z. B. `DEEPSEEK_API_KEY`, `OPENROUTER_API_KEY`). Das 3P-Gateway muss für MCP nicht laufen.

---

## Lizenz

MIT-Lizenz. Siehe [LICENSE](../LICENSE).
