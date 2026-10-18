[English](THIRD_PARTY_INFERENCE.md) | [日本語](THIRD_PARTY_INFERENCE.ja.md) | [中文(简体)](THIRD_PARTY_INFERENCE.zh-CN.md) | [中文(繁體)](THIRD_PARTY_INFERENCE.zh-TW.md) | [한국어](THIRD_PARTY_INFERENCE.ko.md) | [Français](THIRD_PARTY_INFERENCE.fr.md) | [Deutsch](THIRD_PARTY_INFERENCE.de.md) | [Español](THIRD_PARTY_INFERENCE.es.md)

# Poverello mit Claude Desktop / Cowork on 3P verwenden

Poverello kann als lokales Anthropic-kompatibles Gateway für
Claude Desktop / Cowork on 3P verwendet werden.

Claude Desktop / Cowork on 3P unterstützt Drittanbieter-Inferenz über
das integrierte Konfigurationsfenster.

Offizielle Dokumentation:

- [https://claude.com/docs/cowork/3p/installation](https://claude.com/docs/cowork/3p/installation)
- [https://claude.com/docs/cowork/3p/configuration](https://claude.com/docs/cowork/3p/configuration)

## 1. Poverello starten

Starten Sie Poverello und stellen Sie sicher, dass das Gateway läuft.

Standardmäßig lauscht Poverello auf:

```text
http://127.0.0.1:4000
```

Lassen Sie Poverello während der Nutzung von Claude Desktop / Cowork on 3P geöffnet.

## 2. Entwicklermodus in Claude Desktop aktivieren

Öffnen Sie Claude Desktop.

Öffnen Sie auf Windows das Anwendungsmenü oben links.

Wählen Sie dann:

```text
Help → Troubleshooting → Enable Developer Mode
```

Nach der Aktivierung des Entwicklermodus erscheint ein neues `Developer`-Menü.

## 3. Drittanbieter-Inferenz-Einstellungen öffnen

Öffnen Sie:

```text
Developer → Configure third-party inference
```

Dies öffnet das Konfigurationsfenster für Drittanbieter-Inferenz.

## 4. Verbindung konfigurieren

Wählen Sie im Abschnitt `Connection`:

```text
Gateway
```

Geben Sie dann die folgenden Werte ein.

| Feld                  | Wert                                       |
| --------------------- | ------------------------------------------ |
| Gateway base URL      | `http://127.0.0.1:4000`                    |
| Gateway API key       | `sk-local-gateway`                         |
| Gateway auth scheme   | `bearer`                                   |
| Gateway extra headers | Leer lassen, sofern keine benutzerdefinierten Header benötigt werden |

Der `Gateway API key` muss mit dem in Poverello konfigurierten lokalen API-Schlüssel übereinstimmen.

## 5. Identität und Modelle konfigurieren

Fügen Sie im Abschnitt `Identity & Models` die Modell-IDs hinzu, die Claude Desktop in der Modellauswahl anzeigen soll.

Beispiel:

```text
claude-sonnet-4-6
claude-haiku-4-5
```

Sie können jedem Modell auch ein Anzeigekennzeichen geben.

Beispiel:

| Modell-ID            | Anzeigekennzeichen |
| -------------------- | ------------------ |
| `claude-sonnet-4-6`  | `Gateway Pro`      |
| `claude-haiku-4-5`   | `Gateway Flash`    |

Das erste Modell in der Liste wird als Standard-Eintrag verwendet.

Für jedes Modell: Klappen Sie die Zeile auf und bestätigen Sie, dass die `Model ID` genau der Modellname ist, den Claude Desktop an Poverello senden soll.

Aktivieren Sie `Offer 1M-context variant` nur, wenn Ihr Upstream-Anbieter und das ausgewählte Modell das erweiterte Kontextfenster tatsächlich unterstützen.

## 6. Beispielkonfiguration

Die obigen Einstellungen entsprechen der folgenden Drittanbieter-Inferenz-Konfiguration:

```json
{
  "inferenceProvider": "gateway",
  "inferenceGatewayBaseUrl": "http://127.0.0.1:4000",
  "inferenceGatewayApiKey": "sk-local-gateway",
  "inferenceGatewayAuthScheme": "bearer",
  "inferenceModels": [
    {
      "name": "claude-sonnet-4-6",
      "labelOverride": "Gateway Pro"
    },
    {
      "name": "claude-haiku-4-5",
      "labelOverride": "Gateway Flash"
    }
  ]
}
```

## 7. Anwenden und Claude Desktop neu starten

Wenden Sie die Gateway- und Modelllisteneinstellungen lokal an.

Starten Sie Claude Desktop bei Aufforderung neu.

Nach dem Neustart von Claude Desktop werden Anfragen von Cowork on 3P an Poverello gesendet. Poverello leitet die Anfragen dann an den in Poverello konfigurierten Upstream-Anbieter weiter.

## Direct DeepSeek API

Wenn Poverello für den Direct-DeepSeek-Anbieter konfiguriert ist, werden Anfragen von Claude Desktop / Cowork on 3P an den Anthropic-Messages-kompatiblen DeepSeek-Endpunkt weitergeleitet.

Die kanonischen DeepSeek-API-Modell-IDs sind `deepseek-flash` und `deepseek-v4-pro`; die aktuelle offizielle Preisseite führt sie als DeepSeek-V4.1-Flash-1017 bzw. DeepSeek-V4-Pro-0813 auf. Dies sind Upstream-API-IDs. Die Claude-seitigen Namen in `inferenceModels` sind lokale Einträge für die Modellauswahl, die Poverello dem ausgewählten Anbieterprofil zuordnet.

Beide Modelle unterstützen Tool-Aufrufe, ein Kontextfenster mit 1M Token und bis zu 384K Ausgabetoken. Nur Flash unterstützt Bildeingaben. Die ausgemusterten DeepSeek-IDs `deepseek-v4-flash` und `deepseek-v4-flash-vision-exp` werden upstream weiterhin als Aliase für Flash akzeptiert. Poverellos integrierte Direct-DeepSeek-Standards verwenden die kanonischen IDs. Offizielle Quellen widersprechen sich beim Pro-Routing und der Abrechnung: Die aktuelle Preisseite führt separate Pro-Tarife auf und bestätigt den fortgesetzten API-Dienst nach dem 14. September 2026, während eine Mitteilung vom 10. September eine vorübergehende Weiterleitung zu V4.1 Flash zum Flash-Tarif bis zum Start von V4.1-Pro beschrieb. Poverello kann Routing und Abrechnung einzelner Anfragen nicht prüfen; angezeigt werden veröffentlichte Tarife, keine Rechnung oder Garantie. Siehe [aktuelle Preise](https://api-docs.deepseek.com/quick_start/pricing/) und [Mitteilung vom 10. September](https://api-docs.deepseek.com/news/news260910/).

Aktuelle Tokenpreise und Peak-/Off-Peak-Regeln finden Sie in DeepSeeks [offizieller Preisliste](https://api-docs.deepseek.com/quick_start/pricing/) und im [Provider-Details-Leitfaden](PROVIDERS.md#deepseek). Der mitgelieferte Feiertagskalender von Poverello beeinflusst die lokale Peak-/Off-Peak-Schätzung; Abdeckung und Fallback-Verhalten sind im Provider-Leitfaden beschrieben.

## Xiaomi MiMo-Anbieter

Wenn Poverello für die Verwendung des Direct MiMo-Anbieters konfiguriert ist, werden Inferenzanfragen von Claude Desktop an Xiaomis MiMo-API weitergeleitet.

### MiMo-V2.6-Modelle

| Model ID | Anzeigename | Multimodal |
| --- | --- | --- |
| `mimo-v2.6-flash` | MiMo-V2.6-Flash | Image + Video |
| `mimo-v2.6-pro` | MiMo-V2.6-Pro | Image + Video |
| `mimo-v2.6-pro-ultraspeed` | MiMo-V2.6-Pro-UltraSpeed | Image + Video |

Alle V2.6-Modelle unterstützen ein Kontextfenster von 1.000.000 Token.

### Normal / Thinking

MiMo verwendet einen einfachen Normal/Thinking-Schalter. Es gibt keine Reasoning-Aufwandsstufen (Low/Medium/High).

| Einstellung | Upstream-Parameter | Verhalten |
| --- | --- | --- |
| Normal | `thinking: {"type": "disabled"}` | Standardinferenz |
| Thinking | `thinking: {"type": "enabled"}` | Erweitertes Denken |

### Standardrouten

Die eingebauten Routen des Direct MiMo-Presets:

| Gateway-Modell | Upstream-Modell | Thinking |
| --- | --- | --- |
| `claude-opus-5` | `mimo-v2.6-pro` | Thinking |
| `claude-sonnet-5` | `mimo-v2.6-pro` | Normal |
| `claude-haiku-4-5` | `mimo-v2.6-flash` | Thinking |

### V2.5-Abwärtskompatibilität

Gespeicherte `mimo-v2.5`-, `mimo-v2.5-pro`- und `mimo-v2.5-pro-ultraspeed`-Konfigurationen bleiben erhalten und funktionieren weiterhin. Beim Start werden nur unveränderte Werksrouten automatisch auf V2.6 migriert. Angepasste Routen, Thinking-Modus-Einstellungen und benutzerdefinierte `default_model`-Werte werden niemals überschrieben.

> [!NOTE]
> **Claude Desktop / Cowork on 3P nutzt Cloud-Provider über das Gateway**
>
> Claude Desktop und Cowork on 3P senden Inferenzanfragen an das Poverello Gateway (`http://127.0.0.1:4000`), das sie an Ihren konfigurierten Cloud-Upstream-Provider weiterleitet (DeepSeek, MiMo, Kimi, MiniMax, OpenRouter).
>
> Das **Ollama Local**-Backend ist strikt **Claude Code CLI** vorbehalten und vollständig von Claude Desktop und MCP isoliert. Für die lokale LLM-Einrichtung mit Claude Code lesen Sie bitte den [Ollama Local Leitfaden](OLLAMA_LOCAL.de.md).

---

## MiniMax-M3 Thinking-Modus

MiniMax-M3 unterstützt das Ein- und Ausschalten von erweitertem Thinking über die Anthropic-kompatible API.

| Einstellung | Upstream-API-Parameter | Verhalten |
| --- | --- | --- |
| Thinking EIN | `thinking: {"type": "adaptive"}` | Aktiviert erweitertes Thinking |
| Thinking AUS | `thinking: {"type": "disabled"}` | Deaktiviert erweitertes Thinking |
| Standard (nicht gesetzt) | *weggelassen* | API-Standard: Thinking deaktiviert |

In den Poverello-Einstellungen wird für MiniMax-M3 ein Umschalter zwischen Thinking und Normal angezeigt. MiniMax-M2.x-Modelle (z. B. `MiniMax-M2.7-highspeed`) unterstützen das Deaktivieren von Thinking nicht und bleiben auf den Thinking-only-Modus festgelegt.

## Laguna S/XS 2.1 (OpenRouter) konfigurieren

Standardmäßig leitet Poverello OpenRouter `claude-opus-5` und `claude-sonnet-5` an `poolside/laguna-s-2.1` sowie `claude-haiku-4-5` an `poolside/laguna-xs-2.1` weiter.

### Thinking-Verhalten

Drittanbieter-Tests beobachteten, dass die Struktur des System-Prompts das Thinking-Verhalten von Laguna S 2.1 beeinflussen kann. Berichten zufolge verringerte eine klare berufliche Persona mit ausdrücklichen Abnahmekriterien unnötiges Reasoning deutlich. Aufgrund dieser Ergebnisse wurde Laguna Opus (`claude-opus-5`) standardmäßig auf Normal (Thinking aus) gesetzt; in den Einstellungen kann Thinking wieder aktiviert werden.

### Bekannte Einschränkung: keine nutzbare Ausgabe am Tokenlimit

Unter bestimmten Bedingungen – insbesondere wenn das Modell während des Reasoning das Tokenlimit pro Runde erreicht – kann Laguna S 2.1 ausschließlich Reasoning-Inhalt ohne Textausgabe oder Tool-Aufruf und mit `stop_reason: "max_tokens"` zurückgeben. Poverello erkennt dies und protokolliert eine Warnung. Bei wiederholter Meldung „Reasoning-only response reached the per-turn token limit“ können Sie das maximale Ausgabe-Tokenlimit erhöhen oder Thinking für dieses Modell deaktivieren.

Diese Beobachtungen stammen aus Tests der Community von Drittanbietern; die Ergebnisse können je nach Anbieterkonfiguration und Modellrevision variieren.

## Hinweise

Poverello ist ein inoffizielles Anthropic-kompatibles lokales Gateway.

Es ist nicht mit Anthropic, Moon Bridge oder einem beliebigen Upstream-Modellanbieter verbunden.

Claude Desktop / Cowork on 3P wird über die Drittanbieter-Inferenz-Einstellungen von Claude konfiguriert. Menübeschriftungen und Konfigurationsfelder können sich ändern, wenn Anthropic Claude Desktop aktualisiert.
