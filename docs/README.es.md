[English](../README.md) | [日本語](README.ja.md) | [中文(简体)](README.zh-CN.md) | [中文(繁體)](README.zh-TW.md) | [한국어](README.ko.md) | [Français](README.fr.md) | [Deutsch](README.de.md) | [Español](README.es.md)

# Poverello Coding Tools

**Usa Claude Code / Claude Desktop como entorno de desarrollo, enruta la inferencia a modelos en la nube o locales y gestiona modelos externos y reglas para Google Antigravity.**

Poverello Coding Tools (que aparece en la aplicación como **Poverello**) es una aplicación complementaria para Windows orientada al desarrollo de software asistido por IA. Admite cuatro flujos de trabajo complementarios:

1. **Gateway 3P para Claude Code / Claude Desktop** — Conserva la exploración de repositorios, el uso de herramientas, la edición de archivos y la ejecución de pruebas de Claude mientras enruta la inferencia a proveedores de terceros.
2. **LLM local dedicado para Claude Code (Ollama)** — Ejecuta Claude Code sin conexión mediante modelos locales y sin costes de API.
3. **Planificador y Revisor MCP para Google Antigravity** — Delega la planificación, revisión de planes y revisión de implementaciones a modelos externos mediante las herramientas MCP `poverello/plan`, `poverello/revise` y `poverello/review`.
4. **Antigravity Autocode y Skills & Rules** — Configure la revisión externa y el comportamiento de codificación en la pestaña Antigravity Autocode; administre Workflow Commands, CLI Tools, Task Presets, Global Rules y Nerf Skills en Settings.

> [!TIP]
> **Novedades:** Consulta [NEWS.md (Notas de la versión)](../NEWS.md) para ver las funciones, detalles de implementación y cambios de cada versión.

---

## Cuatro Flujos de Trabajo Principales

### 1. Claude Code / Claude Desktop con Gateway 3P

```text
Claude Code / Claude Desktop
             ↓
  Poverello 3P Gateway
             ↓
Modelo en la nube o local configurado
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

Flujos autónomos: `/pov-full-cycle`, `/pov-plan-only`, `/pov-build-review` (instalación y reparación en **Configuración > Antigravity Skills & Rules**; Reviewer Policy en **Antigravity Autocode**).

**Flujo de trabajo recomendado:**

```text
/pov-mcp-plan → Implementación y Pruebas → /pov-mcp-review → Commit
```

### 4. Antigravity Autocode y gestión de Skills & Rules
```text
Antigravity Autocode tab
  • External Review policy / reviewer configuration
  • Customize Coding Settings / Direct Coding

Settings > Antigravity Skills & Rules
  • Workflow Commands / repair
  • CLI Tools / Task Presets
  • Global Rules / Nerf skills
```

- External Review y Customize Coding Settings se configuran en Antigravity Autocode. Settings administra los tres Workflow Skills, CLI Tools, Task Presets, Global Rules y Nerf Skills.
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

### Flujo de trabajo 4: Antigravity Autocode y Skills & Rules
1. Configure External Review y Customize Coding Settings en **Antigravity Autocode**. La política seleccionada controla los revisores que invocan los Workflow Skills.
2. Administre Workflow Commands, CLI Tools, Task Presets, Global Rules y Nerf Skills en **Settings > Antigravity Skills & Rules**. La reparación de un Skill requiere confirmación y conserva una copia de seguridad.
3. Auto-Converge sigue oculto; el dispatch de producción está desactivado y el aislamiento de Stage 2 no está verificado.
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
