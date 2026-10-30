[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

> **Transición a v0.25.2:** Auto-Converge se conserva en el código base, pero se oculta de la navegación normal de la aplicación. El dispatch de producción sigue desactivado y el aislamiento de Stage 2 no está verificado. Esto no significa que esté listo para publicarse.

**Usa Claude Code / Claude Desktop como entorno de desarrollo, enruta la inferencia a APIs de LLM de terceros, orquesta flujos multiagente acotados con estimación de costes y gestiona de forma centralizada modelos externos y reglas para Google Antigravity.**

Poverello Coding Tools (que aparece en la aplicación como **Poverello**) es una aplicación complementaria para Windows orientada al desarrollo de software asistido por IA. Admite cinco flujos de trabajo complementarios:

1. **Gateway 3P para Claude Code / Claude Desktop** — Conserva la exploración de repositorios, el uso de herramientas, la edición de archivos y la ejecución de pruebas de Claude mientras enruta la inferencia a proveedores de terceros.
2. **LLM local dedicado para Claude Code (Ollama)** — Ejecuta Claude Code sin conexión mediante modelos locales y sin costes de API.
3. **Planificador y Revisor MCP para Google Antigravity** — Delega la planificación, revisión de planes y revisión de implementaciones a modelos externos mediante las herramientas MCP `poverello/plan`, `poverello/revise` y `poverello/review`.
4. **Orquestador de desarrollo multiagente** — Ejecuta los flujos acotados Planner–Reviewer actualmente compatibles y registra el uso normalizado de tokens y las estimaciones de coste verificadas.
5. **Gestión de Antigravity Skills & Rules** — Configura de forma centralizada la directiva Reviewer Policy para habilidades de flujo de trabajo (Codex CLI en solitario frente a revisión compuesta Codex CLI + DeepSeek Harness), gestiona plantillas estándar protegidas de reglas globales (`codex-cli.md`, `deepseek-harness.md`), ajusta el comportamiento de Direct Coding (`pov-direct.md`) y gestiona las habilidades Nerf.

> [!TIP]
> **Novedades:** Consulta [NEWS.md (Notas de la versión)](../NEWS.md) para ver las funciones, detalles de implementación y cambios de cada versión.

---

## Cinco Flujos de Trabajo Principales

### 1. Claude Code / Claude Desktop con Gateway 3P

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P Gateway
             ↓
Anthropic Direct / DeepSeek / Kimi Code / OpenRouter / MiniMax / MiMo / GLM
```

- **Separación de Entorno y Modelo**: Conserva las herramientas agénticas de Claude mientras enruta la inferencia a proveedores de terceros.
- **Enrutamiento Dinámico Multi-Perfil**: Cambia proveedores activos, perfiles de OpenRouter y rutas de modelos desde la interfaz gráfica.
- **Guía de Configuración**: [Configuración del Gateway 3P para Claude Desktop / Cowork](THIRD_PARTY_INFERENCE.es.md)

### 2. LLM locales dedicados para Claude Code (Ollama)

```text
Claude Code CLI
      ↓ (Loopback ANTHROPIC_BASE_URL)
Ollama Local (127.0.0.1:11434/v1)
      ↓
Gemma 4 / Qwen / Llama 3 / DeepSeek-R1 (Local en el dispositivo)
```

- **Gratuito, sin conexión y privado**: Ejecute Claude Code sin claves API, límites de uso ni transferencia de datos a la nube.
- **Enrutamiento dedicado y detección de modelos**: Ruta de Claude Code independiente (`active_route: "ollama"`) con detección automática de modelos mediante `/api/tags`.
- **Modo de pensamiento y ajuste de contexto**: Soporte completo para presupuestos de pensamiento (1024 tokens) y ventana de contexto (`r`num_ctx`) por alias.
- **Aislamiento total**: Claude Desktop, Cowork on 3P y Google Antigravity MCP continúan utilizando modelos en la nube de forma independiente.
- **Guía de configuración**: [Guía de Ollama Local](OLLAMA_LOCAL.es.md)

### 3. Antigravity con planificador y revisor MCP

```text
Antigravity
    ↓ stdio
anthro-bridge.exe --mcp-server
    ↓
Modelo externo configurado (planificador / revisor)
    ↓
Plan de implementación / Veredicto de revisión
    ↓
Antigravity implementa y prueba
usando capacidad respaldada por suscripción
```

- **División Planificación / Ejecución**: Los modelos externos generan el plan de alto nivel o el veredicto de revisión; la capacidad de suscripción de Antigravity ejecuta las ediciones de código intensivas en tokens.
- **Configuración en Vivo desde la GUI**: Cambiar el proveedor, el modelo o el nivel de razonamiento del planificador o revisor tiene efecto inmediato en la siguiente invocación.
- **Guía de Configuración**: [Configuración de Google Antigravity + Poverello MCP](ANTIGRAVITY_MCP.es.md)

El ejecutable y los identificadores de herramientas MCP conservan el nombre `anthro-bridge` por compatibilidad.

Los comandos canónicos de Antigravity son `/pov-mcp-plan`, `/pov-mcp-revise` y `/pov-mcp-review`; los comandos anteriores `/anthro-*` siguen disponibles por compatibilidad.

**Comandos globales de Antigravity:**

- **`/pov-mcp-plan`** — Delega la planificación de la implementación al modelo externo configurado.
- **`/pov-mcp-revise`** — Revisa un plan existente en función de nuevos comentarios o restricciones.
- **`/pov-mcp-review`** — Revisa una implementación completada contra el plan aprobado antes de hacer commit, con veredictos explícitos READY / NOT READY.

Flujos autónomos: `/pov-full-cycle`, `/pov-plan-only`, `/pov-build-review` (gestionados en **Configuración > Antigravity Skills & Rules** con sincronización de Reviewer Policy).

**Flujo de trabajo recomendado:**

```text
/pov-mcp-plan → Implementación y Pruebas → /pov-mcp-review → Commit
```

### 4. Orquestador de desarrollo multiagente (flujos acotados y costes)

```text
Tarea / Requisito
        ↓
+-------------------------------------------------------------+
| Poverello Orchestrator                                      |
|                                                             |
|  [Perfil Planner]  ⇄  [Perfil Reviewer]                     |
|  (Generar / Revisar)    (Verificar / Veredictos)            |
|                                                             |
|  • Bucle acotado plan/revisión (límite de turnos)           |
|  • Registros completos (%APPDATA%\Poverello\runs\*.json)    |
|  • Estimaciones enteras en nanodólares y verificación       |
|  • Insignias de fallo seguro (Complete / Partial / N/A)     |
+-------------------------------------------------------------+
        ↓
Plan Aprobado / Registro de Auditoría
        ↓
Implementación separada y controlada por el usuario en su entorno
```

- **Flujos acotados del Orchestrator**: Configura perfiles de agentes y límites de iteración para flujos compatibles.
- **Invariantes de seguridad y confirmación humana**: Las acciones que requieren aprobación siguen puertas de control explícitas; un nuevo plan principal siempre requiere confirmación explícita.
- **Registros auditables**: `%APPDATA%\Poverello\runs\<run_id>.json` guarda la configuración, tareas, metadatos de despacho, uso normalizado y estimaciones de costes.
- **Uso normalizado y estimaciones basadas en tarifas**: Cálculos en nanodólares enteros (`u64 amount_nanos`) para prevenir errores de redondeo, cerrando con fallo seguro ante tarifas desconocidas.

### 5. Gestión de Antigravity Skills & Rules

```text
Poverello GUI (Configuración > Antigravity Skills & Rules)
          ↓ (Reviewer Policy / Protección de Plantillas / Rollback)
%USERPROFILE%\.gemini\config\rules\
  • codex-cli.md / deepseek-harness.md (Plantillas Estándar de Reglas Globales)
  • pov-direct.md (Comportamiento de Direct Coding)
~/.gemini/antigravity/skills/
  • pov-full-cycle, pov-plan-only, pov-build-review (Habilidades Sincronizadas)
```

- **Reviewer Policy centralizada**: Cambia de inmediato las habilidades de flujo de trabajo (`/pov-full-cycle`, `/pov-plan-only`, `/pov-build-review`) entre `codex_only` (Codex CLI independiente en solitario) y `codex_plus_deepseek` (revisión compuesta Codex CLI + DeepSeek Harness) con validación automática de SHA-256 y sincronización.
- **Plantillas protegidas de reglas globales**: Instalación con un solo clic y eliminación segura de plantillas estándar (`codex-cli.md`, `deepseek-harness.md`) verificadas por SHA-256, con protección de plataforma (`can_remove`) en Windows.
- **Control de alcance y dirección (Poverello Direct Coding)**: Configura reglas ligeras para evitar ampliación indebida del alcance en tareas acotadas.
- **Integridad de un solo archivo y aplicación segura**: Protegido por verificación previa de SHA-256, detección de conflictos de eliminación externa y reversión verificada.

## Proveedores Compatibles

| Proveedor | Conexión | Familias Compatibles | Controles de Razonamiento |
|---|---|---|---|
| **Anthropic** | API Directa | Claude Opus 5.5, Sonnet 5.5, Haiku 4.5, Sonnet 3.7 | Thinking adaptativo / esfuerzo |
| **DeepSeek** | API directa | `deepseek-flash` (V4.1 Flash 1017), `deepseek-v4-pro` (V4 Pro 0813) | Normal / Thinking · Low / High / Max |
| **GLM (Z.ai)** | API Directa | GLM-5.3, GLM-5.3-Flash, GLM-5.3-FlashX, GLM-5.2 | GLM-5.3: Low / High / Max · GLM-5.2: Normal / Thinking |
| **Kimi Code** | API Directa | kimi-for-coding, kimi-for-coding-highspeed | Modo de pensamiento |
| **MiniMax** | API Directa | MiniMax M3, M2.7 | Específico del modelo |
| **Kimi / Moonshot** | API Directa | Kimi K2.x, Kimi K3 | Thinking / Reasoning effort |
| **MiMo / Xiaomi** | API Directa | MiMo V2.6 Flash, Pro, Pro-UltraSpeed (compatible con V2.5) | Normal / Thinking |
| **Ollama Local** | Loopback local | Gemma 4, Qwen 2.5, Llama 3.3, DeepSeek-R1 | Thinking (presupuesto 1024) / Disabled |
| **OpenRouter** | Gateway Multi-Perfil | Ver la sección de OpenRouter a continuación | Específico del modelo / del perfil |
| **Gateway Multi-Proveedor** | Rutas personalizadas | Cualquier combinación de proveedores compatibles | Controles por ruta |

### Anthropic (Directo)

Acceso oficial directo mediante `https://api.anthropic.com` con `ANTHROPIC_API_KEY`. Modelos compatibles: `claude-opus-5-5`, `claude-sonnet-5-5`, `claude-haiku-4-5`, `claude-3-7-sonnet`, `claude-3-5-sonnet` y `claude-3-5-haiku`. Admite controles nativos de adaptive thinking y esfuerzo de razonamiento.

### GLM / Z.ai (Directo)

Acceso directo a la API oficial de Z.ai compatible con Anthropic Messages (`https://api.z.ai/api/anthropic/v1/messages`) mediante `ZAI_API_KEY`. La familia GLM-5.3 permite seleccionar el esfuerzo (`low`, `high`, `max`); GLM-5.2 ofrece el selector Normal / Thinking.

### DeepSeek (Directo)

El preset integrado **Direct DeepSeek** enruta: Opus 5 → V4.1 Flash / Max · Sonnet 5 → V4.1 Flash / High · Haiku 4.5 → V4.1 Flash / Low.

- `deepseek-flash` — ID API canónico y estable de DeepSeek-V4.1-Flash-1017; contexto de 1M y hasta 384K tokens de salida.
- `deepseek-v4-pro` — ID API; la página oficial de precios actual lo identifica como DeepSeek-V4-Pro-0813. Contexto de 1M y hasta 384K tokens de salida.
- Flash admite visión; Pro no. DeepSeek aún acepta los antiguos ID `deepseek-v4-flash` y `deepseek-v4-flash-vision-exp` como alias de Flash y los factura con la tarifa Flash.

### Kimi Code (Directo)

API especializada en codificación (`KIMI_CODE_API_KEY`), independiente de Moonshot Kimi:

- `kimi-for-coding` — Modelo de codificación de calidad completa.
- `kimi-for-coding-highspeed` — Variante de baja latencia.

### MiMo / Xiaomi (Directo)

Presets integrados de **Direct MiMo**:
- Opus 5 → `mimo-v2.6-pro` / Thinking
- Sonnet 5 → `mimo-v2.6-pro` / Normal
- Haiku 4.5 → `mimo-v2.6-flash` / Thinking
- Modelo predeterminado: `mimo-v2.6-flash`

Modelos: `mimo-v2.6-flash`, `mimo-v2.6-pro`, `mimo-v2.6-pro-ultraspeed` (seleccionables). Los tres admiten una ventana de contexto de 1 millón de tokens y capacidades multimodales nativas (texto, imagen, vídeo).

**Normal / Thinking**: MiMo usa un simple interruptor Normal/Thinking — sin niveles de esfuerzo de razonamiento.

**Compatibilidad con V2.5**: Las rutas guardadas `mimo-v2.5`, `mimo-v2.5-pro` y `mimo-v2.5-pro-ultraspeed` se conservan y siguen funcionando. Los valores predeterminados heredados sin modificar migran automáticamente a V2.6 al inicio.

### LLM local dedicado (Ollama Local)

Backend de inferencia local **dedicado a Claude Code CLI**:
- Endpoint: `http://127.0.0.1:11434/v1` (bucle local del protocolo Anthropic Messages nativo)
- Detección dinámica de modelos locales instalados mediante `/api/tags`
- Presupuestos de pensamiento (1024 tokens) y ventanas de contexto (`r`num_ctx`) configurables por alias
- Consulta la [Guía de Ollama Local](OLLAMA_LOCAL.es.md) para obtener detalles de configuración

### OpenRouter

Admite múltiples perfiles con nombre. Catálogo actual de modelos OpenAI en un único desplegable (15 modelos):

| ID del Modelo | Nombre Mostrado |
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

**GPT-6 Astra**: 1.05M de contexto · nivel de razonamiento: `low / medium / high / xhigh / max`.
**GPT-6 Astra Pro**: 1.05M de contexto · razonamiento Pro siempre activo (`reasoning.mode = pro`), sin selección de nivel por parte del usuario.
**GPT Astra Latest**: alias que sigue el modelo más reciente de la familia Astra.

Preset integrado **OpenRouter: chatGPT**: Opus 5 → GPT-6 Astra / max · Sonnet 5 → GPT-6 Astra / high · Haiku 4.5 → GPT-6 Astra / medium.

También disponibles: **OpenRouter: Gemini** (Gemini 3.8 Flash · nivel de razonamiento `low / medium / high`), **OpenRouter: Poolside**, **OpenRouter: Tencent**, **OpenRouter: InclusionAI**, **OpenRouter: StepFun**.

---

## Precios de la API de DeepSeek (verificados el 19-10-2026)

| ID del modelo | Entrada pico (fallo / acierto de caché) | Entrada valle (fallo / acierto de caché) | Salida pico | Salida valle |
|---|---:|---:|---:|---:|
| `deepseek-flash` | \$0.30 (\$0.006) | \$0.15 (\$0.003) | \$1.20 | \$0.60 |
| `deepseek-v4-pro` | \$1.32 (\$0.044) | \$0.66 (\$0.022) | \$3.96 | \$1.98 |

Todos los precios de DeepSeek son por 1 millón de tokens. El horario pico es de lunes a viernes, de 01:00–04:00 y 06:00–10:00 UTC, excepto festivos oficiales chinos; los fines de semana y festivos son valle todo el día. Poverello incluye los festivos y días laborables compensatorios chinos verificados de 2026 e indica si falta el calendario del año siguiente. Los precios de otros proveedores son referencias del GUI y pueden cambiar. El estimador de costes del Orchestrator solo calcula actualmente llamadas elegibles a Anthropic Direct con uso de caché explícitamente igual a cero; no estima costes de DeepSeek. Para Pro, la página de precios actual publica tarifas separadas y confirma la continuidad del servicio API después del 14 de septiembre de 2026; sin embargo, un aviso del 10 de septiembre describió el enrutamiento temporal a V4.1 Flash con tarifas Flash hasta el lanzamiento de V4.1-Pro. Poverello no puede verificar el enrutamiento ni la facturación de cada solicitud; muestra tarifas publicadas, no una factura ni una garantía. Véanse los [precios actuales](https://api-docs.deepseek.com/quick_start/pricing/) y el [aviso del 10 de septiembre](https://api-docs.deepseek.com/news/news260910/).

---

## Instalación

Descarga el instalador de Windows actualmente publicado más reciente desde la página de [Releases](https://github.com/soheidon/poverello-coding-tools/releases) y ejecútalo.

El instalador de Windows admite 8 idiomas. Poverello conserva la identidad de aplicación existente y admite la actualización en el mismo directorio desde Anthro Bridge. En el primer inicio, está diseñado para copiar la configuración y los datos de ejecución de `%APPDATA%\Anthro Bridge` (o `%APPDATA%\Anthro Bridge Dev` en las compilaciones de desarrollo) a la carpeta de datos de Poverello correspondiente, conservando la carpeta original como copia de recuperación. No se han verificado en Windows real la instalación, migración, actualización, reinstalación, cancelación ni desinstalación; haga una copia de seguridad de los datos importantes antes de actualizar.

---

## Inicio Rápido

### Flujo de Trabajo 1: Gateway 3P para Claude Code / Claude Desktop

1. Abre Poverello en **Settings > API Key** y configura una clave de API para el proveedor deseado.
2. Selecciona tu proveedor o perfil de OpenRouter en el panel principal.
3. Haz clic en **Start Gateway** (se ejecuta en `http://127.0.0.1:4000`).
4. Conecta Claude Code o Claude Desktop:
   - **Claude Code**: Haz clic en **Copy Claude Code launch command** en Settings y pégalo en PowerShell.
   - **Claude Desktop / Cowork**: Sigue la [Guía de Configuración 3P para Claude Desktop](THIRD_PARTY_INFERENCE.es.md)

### Flujo de Trabajo 2: LLM locales dedicados para Claude Code (Ollama)

1. Asegúrate de que Ollama se esté ejecutando localmente (`http://127.0.0.1:11434`).
2. Selecciona **Ollama Local** en el panel de Poverello o configura los modelos en **Settings > Claude Code / Local LLMs**.
3. Haz clic en **Copy Claude Code launch command** y pégalo en PowerShell.
4. Claude Code se conecta directamente a tu instancia local de Ollama.

### Flujo de Trabajo 3: Planificador y Revisor MCP para Google Antigravity

1. Configura una clave de API para el modelo planificador/revisor elegido en Poverello.
2. Selecciona la pestaña **MCP** (o **Configuración > Antigravity MCP**) y configura tu modelo en **Configuración > Antigravity MCP > Configuración detallada de MCP**.
3. Registra `anthro-bridge.exe` con `["--mcp-server"]` bajo la clave de servidor `poverello` en la configuración MCP de Antigravity (o haz clic en **Configurar automáticamente** en Poverello).
4. Usa `/pov-mcp-plan` para diseñar planes, `/pov-mcp-revise` para actualizar planes y `/pov-mcp-review` para revisar implementaciones antes de hacer commit.
5. Sigue la [Guía Completa de Configuración de MCP para Antigravity](ANTIGRAVITY_MCP.es.md).

### Flujo de Trabajo 4: Orquestador de desarrollo multiagente

**Nota de acceso:** En v0.25.2, la opción Orchestrator está oculta en la subnavegación de Settings, pero se conservan la implementación y las rutas internas. Inicia flujos autónomos en Antigravity con `/pov-full-cycle`, `/pov-plan-only` o `/pov-build-review`.

### Flujo de Trabajo 5: Gestión de Antigravity Skills & Rules

1. Abre **Configuración > Antigravity Skills & Rules**.
2. Selecciona tu Reviewer Policy (`codex_only` o `codex_plus_deepseek`) para sincronizar las habilidades.
3. Instala o gestiona las plantillas estándar protegidas de reglas globales (`codex-cli.md`, `deepseek-harness.md`).
4. Configura los parámetros de Direct Coding y haz clic en **Aplicar** para sincronizar `pov-direct.md`.

## Claves de API

| Proveedor | Variable de Entorno |
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

## Documentación

- [Notas de la versión (NEWS.md)](../NEWS.md) — Funciones y detalles de implementación por versión
- [Configuración del Gateway 3P para Claude Desktop / Cowork](THIRD_PARTY_INFERENCE.es.md)
- [Ollama Local](OLLAMA_LOCAL.es.md)
- [Configuración de Google Antigravity + Poverello MCP](ANTIGRAVITY_MCP.es.md)
- [Referencia de Configuración (`config.json`)](CONFIGURATION.md)
- [Detalles de Proveedores y Controles de Razonamiento](PROVIDERS.md)
- [Guía de Desarrollo y Verificación](DEVELOPMENT.md)

---

## Solución de Problemas

### El Puerto 4000 Está Ocupado
```powershell
netstat -ano | findstr :4000
taskkill /PID <PID> /F
```

### La Configuración Se Revierte Después de una Actualización
Reinicia la aplicación para que las migraciones puedan ejecutarse. La configuración se almacena en `%APPDATA%\Poverello\config.json`.

### Las Llamadas al Planificador MCP Fallan
Asegúrate de que haya una clave de API configurada para el proveedor seleccionado en la pestaña **MCP**, o que esté exportada en las variables de entorno de usuario de Windows (por ejemplo, `DEEPSEEK_API_KEY`, `OPENROUTER_API_KEY`). El Gateway 3P no necesita estar en ejecución para usar MCP.

---

## Licencia

Licencia MIT. Ver [LICENSE](../LICENSE).
