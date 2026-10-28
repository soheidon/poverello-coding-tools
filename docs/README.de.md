[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

> **Übergang zu v0.25.1:** Auto-Converge bleibt im Code erhalten, ist aber in der normalen App-Navigation ausgeblendet. Der Produktions-Dispatch bleibt deaktiviert und die Stage-2-Isolierung ist nicht verifiziert. Dies ist keine Freigabe zur Veröffentlichung.

**Verwende Claude Code / Claude Desktop als Entwicklungsumgebung, leite Inferenz an Drittanbieter-LLM-APIs weiter und nutze externe Modelle als Planer und Reviewer für Google Antigravity.**

Poverello Coding Tools (in der App als **Poverello** angezeigt) ist eine Windows-Begleitanwendung für KI-gestützte Softwareentwicklung. Es unterstützt fünf sich ergänzende Arbeitsabläufe:

1. **3P-Gateway für Claude Code / Claude Desktop** — Behalte Claudes Repository-Erkundung, Tool-Nutzung, Dateibearbeitung und Testausführung bei, während die Inferenz an Drittanbieter weitergeleitet wird.
2. **Dediziertes lokales LLM für Claude Code (Ollama)** — Führe Claude Code mit lokalen Modellen offline aus.
3. **MCP-Planer & Reviewer für Google Antigravity** — Delegiere Implementierungsplanung, Planrevision und Nachimplementierungsüberprüfung über die MCP-Tools `poverello/plan`, `poverello/revise` und `poverello/review` an externe Modelle.
4. **Multi-Agent Development Orchestrator** — Führe die unterstützten begrenzten Planner–Reviewer-Abläufe für Planung und Review aus und protokolliere normalisierte Token-Nutzung sowie verifizierte Kostenschätzungen.
5. **Verwaltung von Antigravity-Regeln** — Konfiguriere, prüfe und synchronisiere sicher die Poverello-Direct-Coding-Regeln (`pov-direct.md`) für Antigravity.

> **Neuigkeiten:** Versionsbezogene Funktionen, Implementierungsdetails und Änderungsprotokolle findest du in den [Release Notes (NEWS.md)](../NEWS.md).

---

## Fünf Hauptarbeitsabläufe

### 1. Claude Code / Claude Desktop mit 3P-Gateway

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P-Gateway
             ↓
Anthropic Direct / DeepSeek / Kimi Code / OpenRouter / MiniMax / MiMo / GLM
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
- **Denkmodus und Kontext-Anpassung**: Volle Unterstützung für Denk-Token-Budgets (1024 Tokens) und Kontextfenster-Überschreibungen (`num_ctx`) pro Alias.
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

Die kanonischen Antigravity-Befehle sind `/pov-plan`, `/pov-revise` und `/pov-review`; die älteren `/anthro-*`-Befehle bleiben aus Kompatibilitätsgründen verfügbar.

**Globale Antigravity-Befehle:**

- **`/pov-plan`** — Delegiere die Implementierungsplanung an das konfigurierte externe Modell.
- **`/pov-revise`** — Überarbeite einen bestehenden Plan auf der Grundlage neuer Anforderungen oder Einschränkungen.
- **`/pov-review`** — Überprüfe eine abgeschlossene Implementierung anhand des genehmigten Plans vor dem Commit, mit expliziten READY / NOT READY-Urteilen.

**Empfohlener Arbeitsablauf:**

```text
/pov-plan → Implementierung & Tests → /pov-review → Commit
```

---

### 4. Multi-Agent Development Orchestrator (Begrenzte Workflows & Kostenschätzungen)

Der Orchestrator unterstützt begrenzte Planner–Reviewer-Schleifen für Planung und Review mit konfigurierbaren Profilen und Iterationsgrenzen. Ein neuer Hauptplan erfordert immer eine ausdrückliche Bestätigung durch den Menschen. Journale unter `%APPDATA%\Poverello\runs\` enthalten verfügbare normalisierte Nutzungsdaten, Audit-Metadaten und verifizierte Kostenschätzungen. Schätzungen sind keine Rechnungen; nicht unterstützte Routen bleiben nicht berechnet.

### 5. Verwaltung von Antigravity-Regeln (Poverello Direct Coding)

Konfiguriere, prüfe und synchronisiere `pov-direct.md` über die GUI. Die Anwendung bewahrt benutzerdefiniertes Frontmatter und Kommentare und verwendet verifizierte Anwendung, Backups und Rollback-Schutz.

## Unterstützte Anbieter

| Anbieter | Verbindung | Unterstützte Familien | Reasoning-Steuerung |
|---|---|---|---|
| **Anthropic** | Direkte API | Claude Opus 5.5, Sonnet 5.5, Haiku 4.5, 3.7 Sonnet | Adaptives Denken / Aufwand |
| **DeepSeek** | Direkte API | `deepseek-flash` (V4.1 Flash 1017), `deepseek-v4-pro` (V4 Pro 0813) | Normal / Thinking · Low / High / Max |
| **GLM (Z.ai)** | Direkte API | GLM-5.3, GLM-5.3-Flash, GLM-5.3-FlashX, GLM-5.2 | GLM-5.3: Low / High / Max · GLM-5.2: Normal / Thinking |
| **Kimi Code** | Direkte API | kimi-for-coding, kimi-for-coding-highspeed | Thinking-Modus |
| **MiniMax** | Direkte API | MiniMax M3, M2.7 | Modellspezifisch |
| **Kimi / Moonshot** | Direkte API | Kimi K2.x, Kimi K3 | Thinking / Reasoning-Aufwand |
| **MiMo / Xiaomi** | Direkte API | MiMo V2.6 Flash, Pro, Pro-UltraSpeed (V2.5 abwärtskompatibel) | Normal / Thinking |
| **Ollama Local** | Lokaler Loopback | Gemma 4, Qwen 2.5, Llama 3.3, DeepSeek-R1 | Thinking (1024 Budget) / Disabled |
| **OpenRouter** | Multi-Profil-Gateway | Siehe OpenRouter-Abschnitt unten | Modell- / Profilspezifisch |
| **Multi-Provider-Gateway** | Benutzerdefinierte Routen | Beliebige Kombination geeigneter Anbieter | Steuerung je Route |

### Anthropic (Direkt)

Offizieller Direktzugriff über `https://api.anthropic.com` mit `ANTHROPIC_API_KEY`. Unterstützte Modelle sind `claude-opus-5-5`, `claude-sonnet-5-5`, `claude-haiku-4-5`, `claude-3-7-sonnet`, `claude-3-5-sonnet` und `claude-3-5-haiku`. Native Adaptive-Thinking- und Reasoning-Aufwandssteuerung wird unterstützt.

### GLM / Z.ai (Direkt)

Direktzugriff über Z.ais offizielle Anthropic-Messages-kompatible API (`https://api.z.ai/api/anthropic/v1/messages`) mit `ZAI_API_KEY`. GLM-5.3-Modelle unterstützen auswählbaren Reasoning-Aufwand (`low`, `high`, `max`); GLM-5.2 bietet einen Normal-/Thinking-Schalter.

### DeepSeek (Direkt)

Das integrierte Preset **Direct DeepSeek** leitet Opus 5 → V4.1 Flash / Max · Sonnet 5 → V4.1 Flash / High · Haiku 4.5 → V4.1 Flash / Low weiter.

- `deepseek-flash` — Kanonische stabile API-ID für DeepSeek-V4.1-Flash-1017; 1M Kontext und bis zu 384K Ausgabetoken.
- `deepseek-v4-pro` — API-ID; die aktuelle offizielle Preisseite führt DeepSeek-V4-Pro-0813 auf. Kontext 1M und bis zu 384K Ausgabetoken.
- Flash unterstützt Vision, Pro nicht. Die früheren IDs `deepseek-v4-flash` und `deepseek-v4-flash-vision-exp` werden von DeepSeek noch als Flash-Aliase angenommen und zum Flash-Tarif abgerechnet.

### Kimi Code (Direkt)

Dedizierte Coding-Spezialist-API (`KIMI_CODE_API_KEY`), getrennt von Moonshot Kimi:

- `kimi-for-coding` — Vollqualitäts-Coding-Modell.
- `kimi-for-coding-highspeed` — Variante mit niedrigerer Latenz.

### MiMo / Xiaomi (Direkt)

Integrierte **Direct MiMo** Preset-Routen:
- Opus 5 → `mimo-v2.6-pro` / Thinking
- Sonnet 5 → `mimo-v2.6-pro` / Normal
- Haiku 4.5 → `mimo-v2.6-flash` / Thinking
- Standardmodell: `mimo-v2.6-flash`

Modelle: `mimo-v2.6-flash`, `mimo-v2.6-pro`, `mimo-v2.6-pro-ultraspeed` (wählbar). Alle drei unterstützen ein Kontextfenster von 1 Million Tokens und native multimodale Fähigkeiten (Text, Bild, Video).

**Normal / Thinking**: MiMo verwendet einen einfachen Normal/Thinking-Umschalter — keine Reasoning-Aufwandsstufen.

**V2.5-Abwärtskompatibilität**: Gespeicherte Routen `mimo-v2.5`, `mimo-v2.5-pro` und `mimo-v2.5-pro-ultraspeed` bleiben erhalten und funktionieren weiterhin. Unveränderte Legacy-Standardwerte werden beim Start automatisch auf V2.6 migriert.

### OpenRouter

Unterstützt mehrere benannte Profile. Aktueller OpenAI-Modellkatalog im einzelnen Dropdown (15 Modelle):

| Modell-ID | Anzeigename |
|---|---|
| `openai/gpt-6.1-sol` | GPT-6.1 Sol |
| `openai/gpt-6.1-sol-pro` | GPT-6.1 Sol Pro |
| `openai/gpt-6-sol` | GPT-6 Sol |
| `openai/gpt-6-sol-pro` | GPT-6 Sol Pro |
| `openai/gpt-6-luna` | GPT-6 Luna |
| `openai/gpt-6-luna-pro` | GPT-6 Luna Pro |
| `openai/gpt-6-astra` | GPT-6 Astra |
| `openai/gpt-6-astra-pro` | GPT-6 Astra Pro |
| `openai/gpt-astra-latest` | GPT Astra Latest |
| `openai/gpt-5.6-sol` | GPT-5.6 Sol |
| `openai/gpt-5.6-sol-pro` | GPT-5.6 Sol Pro |
| `openai/gpt-5.6-terra` | GPT-5.6 Terra |
| `openai/gpt-5.6-terra-pro` | GPT-5.6 Terra Pro |
| `openai/gpt-5.6-luna` | GPT-5.6 Luna |
| `openai/gpt-5.6-luna-pro` | GPT-5.6 Luna Pro |

**GPT-6 Astra**: 1,05M Kontext · Reasoning-Aufwand: `low / medium / high / xhigh / max`.
**GPT-6 Astra Pro**: 1,05M Kontext · dauerhaftes Pro-Reasoning (`reasoning.mode = pro`), kein benutzerwählbarer Aufwand.
**GPT Astra Latest**: Alias, der das neueste Modell der Astra-Familie verfolgt.

Integriertes Preset **OpenRouter: chatGPT**: Opus 5 → GPT-6 Astra / max · Sonnet 5 → GPT-6 Astra / high · Haiku 4.5 → GPT-6 Astra / medium.

Ebenfalls verfügbar: **OpenRouter: Gemini** (Gemini 3.8 Flash · Reasoning-Aufwand `low / medium / high`), **OpenRouter: Poolside**, **OpenRouter: Tencent**, **OpenRouter: InclusionAI**, **OpenRouter: StepFun**.

---

## DeepSeek-API-Preise (Stand: 19.10.2026)

| Modell-ID | Peak-Eingabe (Cache-Miss / -Hit) | Off-Peak-Eingabe (Cache-Miss / -Hit) | Peak-Ausgabe | Off-Peak-Ausgabe |
|---|---:|---:|---:|---:|
| `deepseek-flash` | \$0,30 (\$0,006) | \$0,15 (\$0,003) | \$1,20 | \$0,60 |
| `deepseek-v4-pro` | \$1,32 (\$0,044) | \$0,66 (\$0,022) | \$3,96 | \$1,98 |

Alle DeepSeek-Preise gelten pro 1 Mio. Token. Peak-Zeiten: werktags 01:00–04:00 und 06:00–10:00 UTC, ausgenommen chinesische Feiertage; Wochenenden und Feiertage sind ganztägig Off-Peak. Poverello enthält geprüfte chinesische Feiertags- und Ausgleichsarbeitstage für 2026 und zeigt an, wenn der Kalender für das Folgejahr fehlt. Preise anderer Anbieter sind veränderliche Referenzwerte im GUI. Der Orchestrator-Kostenschätzer berechnet derzeit nur berechtigte Anthropic-Direct-Aufrufe mit ausdrücklich null Cache-Nutzung, nicht DeepSeek-Kosten. Für Pro führt die aktuelle Preisseite separate Tarife auf und bestätigt den fortgesetzten API-Dienst nach dem 14. September 2026; eine Mitteilung vom 10. September beschrieb jedoch eine vorübergehende Weiterleitung zu V4.1 Flash zum Flash-Tarif bis zum Start von V4.1-Pro. Poverello kann Routing und Abrechnung einzelner Anfragen nicht prüfen; angezeigt werden veröffentlichte Tarife, keine Rechnung oder Garantie. Siehe [aktuelle Preise](https://api-docs.deepseek.com/quick_start/pricing/) und die [Mitteilung vom 10. September](https://api-docs.deepseek.com/news/news260910/).

---

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
2. Wähle den **MCP**-Tab und konfiguriere dein Modell unter **Einstellungen > Antigravity > MCP-Planeinstellungen**.
3. Registriere `anthro-bridge.exe` mit `["--mcp-server"]` in Antigravitys MCP-Konfiguration (oder klicke in Poverello auf **Automatisch konfigurieren**).
4. Verwende `/pov-plan` zum Erstellen von Plänen, `/pov-revise` zum Aktualisieren von Plänen und `/pov-review` zum Überprüfen von Implementierungen vor dem Commit.
5. Folge der vollständigen [Antigravity MCP-Einrichtungsanleitung](ANTIGRAVITY_MCP.de.md).

### Arbeitsablauf 4: Multi-Agent Development Orchestrator

1. Öffne die Registerkarte **Orchestrator**.
2. Wähle Profile für **Planner** und **Reviewer** aus oder passe sie an.
3. Lege das Aufgabenziel und die Iterationsgrenzen des ausgewählten unterstützten Orchestrator-Workflows fest.
4. Prüfe Fortschritt, Audit-Urteile, Kostenschätzungsstatus sowie Journale unter `%APPDATA%\Poverello\runs\`.

### Arbeitsablauf 5: Verwaltung von Antigravity-Regeln

1. Öffne **Einstellungen** und wähle in der Einstellungsnavigation **Antigravity Rules** aus.
2. Wähle den gewünschten Regelmodus und prüfe die Vorschau.
3. Klicke auf **Apply Settings**, um `pov-direct.md` sicher anzuwenden.
4. Benutzerdefinierte Kommentare, Frontmatter-Felder und Backups bleiben erhalten.

---

## API-Schlüssel

| Anbieter | Umgebungsvariable |
|---|---|
| Anthropic | `ANTHROPIC_API_KEY` |
| DeepSeek | `DEEPSEEK_API_KEY` |
| GLM / Z.ai | `ZAI_API_KEY` |
| Kimi Code | `KIMI_CODE_API_KEY` |
| Kimi / Moonshot | `MOONSHOT_API_KEY` |
| MiniMax | `MINIMAX_API_KEY` |
| MiMo / Xiaomi | `XIAOMI_API_KEY` |
| OpenRouter | `OPENROUTER_API_KEY` |

---

## Dokumentation

- [Release Notes (NEWS.md)](../NEWS.md) — Versionsbezogene Funktionen und Implementierungsdetails
- [Claude Desktop / Cowork 3P-Gateway-Einrichtung](THIRD_PARTY_INFERENCE.de.md)
- [Ollama Local](OLLAMA_LOCAL.de.md)
- [Google Antigravity + Poverello MCP-Einrichtung](ANTIGRAVITY_MCP.de.md)
- [Konfigurationsreferenz (`config.json`)](CONFIGURATION.md)
- [Anbieterdetails & Reasoning-Steuerung](PROVIDERS.md)
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
