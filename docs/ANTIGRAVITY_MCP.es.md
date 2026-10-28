[English](ANTIGRAVITY_MCP.md) | [日本語](ANTIGRAVITY_MCP.ja.md) | [中文(简体)](ANTIGRAVITY_MCP.zh-CN.md) | [中文(繁體)](ANTIGRAVITY_MCP.zh-TW.md) | [한국어](ANTIGRAVITY_MCP.ko.md) | [Français](ANTIGRAVITY_MCP.fr.md) | [Deutsch](ANTIGRAVITY_MCP.de.md) | [Español](ANTIGRAVITY_MCP.es.md)

[← Volver al README de Poverello](README.es.md)

# Uso de Poverello MCP con Google Antigravity

Poverello no requiere un ejecutable de servidor MCP independiente. El único archivo `anthro-bridge.exe` instalado proporciona tanto la aplicación GUI de escritorio como las funciones de servidor MCP. Antigravity inicia el modo MCP ejecutando ese mismo archivo con el argumento `--mcp-server`.

La entrada canónica del servidor MCP es `poverello` y expone `poverello/plan`, `poverello/revise` y `poverello/review`. El ejecutable `anthro-bridge.exe`, la entrada antigua `anthro-bridge` y las herramientas antiguas `anthro-bridge/*` se mantienen únicamente por compatibilidad. El producto se llama Poverello Coding Tools y la aplicación aparece como Poverello.

```text
Inicio normal
anthro-bridge.exe
→ Aplicación de escritorio Poverello / 3P Gateway

Inicio MCP
anthro-bridge.exe --mcp-server
→ Servidor MCP stdio headless para Antigravity
```

Esto permite a entornos de agentes como Google Antigravity delegar el diseño arquitectónico y la planificación de implementación en modelos LLM externos (p. ej., DeepSeek V4, MiMo, Kimi, MiniMax o modelos de OpenRouter) a través de `poverello/plan`, mientras realiza las modificaciones de código, comandos de terminal, compilación y pruebas de alto consumo de tokens con la cuota incluida en la suscripción de Antigravity.

---

## 1. Funcionamiento de este flujo de trabajo

```text
Antigravity
    ↓ stdio
anthro-bridge.exe --mcp-server
    ↓
Modelo planificador externo configurado
    ↓
Plan de implementación estructurado devuelto
    ↓
Antigravity ejecuta ediciones,
compilación y pruebas mediante su suscripción
```

---

## 2. Requisitos previos

1. **Poverello** instalado en Windows.
2. Autenticación del proveedor configurada en Poverello o en las variables de entorno del sistema para el planificador que desea utilizar.
3. **Google Antigravity** instalado y en ejecución.

---

## 3. Configurar el servidor MCP en Antigravity

### Método 1 — Configuración mediante la GUI de Poverello (Recomendada)

1. Abra Poverello y vaya a **Configuración** (pestaña `[Configuración]`) > subnavegación izquierda **Antigravity**.
2. Revise la tarjeta **Integración con Google Antigravity**:
   - **Ejecutable de destino**: Muestra por defecto la ruta de `anthro-bridge.exe` en ejecución. Para usar otro binario (portable o desarrollo propio), haga clic en **Cambiar** (`antigravity.btnChangeExe`) y seleccione el ejecutable.
   - **Registrar / Actualizar**: Haga clic en **Actualizar configuración de Antigravity** (`antigravity.btnUpdate`) para registrar o actualizar de forma segura `anthro-bridge` en `%USERPROFILE%\.gemini\config\mcp_config.json`, preservando intactos los demás servidores MCP.
   - **Eliminar**: Haga clic en **Eliminar configuración** (`antigravity.btnRemove`) si desea anular el registro del servidor en Antigravity.
   - **Abrir carpeta**: Haga clic en **Abrir carpeta de configuración** (`antigravity.btnOpenFolder`) para inspeccionar el directorio en el Explorador de Windows.

---

### Método 2 — Configuración manual (Avanzada)

1. En Poverello **Configuración > Antigravity**, haga clic en **Abrir carpeta de configuración** para abrir `%USERPROFILE%\.gemini\config\` en el Explorador de Windows.
2. Abra o cree `mcp_config.json` y agregue `anthro-bridge` dentro de `mcpServers`:

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

Para compilaciones de desarrollo, apunte directamente al ejecutable de Release:
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
> **No** necesita escribir claves API en el archivo `mcp_config.json` de Antigravity. El servidor MCP utiliza el mecanismo de resolución de credenciales de Poverello (lectura automática de variables de entorno de Windows como `DEEPSEEK_API_KEY`, `OPENROUTER_API_KEY`, `MOONSHOT_API_KEY`, `MINIMAX_API_KEY`, `XIAOMI_API_KEY` o configuración guardada).

---

## 4. Verificar la conexión MCP

En la vista **Installed MCP Servers** de Antigravity, confirme que `anthro-bridge` esté reconocido:

```text
anthro-bridge
  1 tool enabled
  - plan
```

---

## 5. Configurar los modelos planificadores en Poverello

Poverello separa claramente la selección del planificador de la gestión detallada de parámetros:

1. **Pestaña superior `MCP` (`MCP for Antigravity`)**:
   - Muestra tarjetas de los proveedores disponibles (DeepSeek, OpenRouter, MiniMax, MiMo (V2.6), Kimi y GLM), además de los perfiles.
   - Haga clic en una tarjeta para cambiar de inmediato el destino del planificador activo.
2. **`Configuración` > `Antigravity`**:
   - Tarjeta **Configuración detallada de MCP**: Configure el modelo, el modo Thinking y el nivel Reasoning Effort por proveedor/perfil.
   - Tarjeta **Integración con Google Antigravity**: Gestione el registro del servidor MCP, los comandos de Antigravity (Global Skills) y las reglas globales (`deepseek-harness.md`, `codex-cli.md`).

> [!NOTE]
> El servidor MCP de Poverello lee dinámicamente la configuración actual en cada invocación de la herramienta `plan()`. **No** necesita reiniciar el servidor MCP ni Antigravity al cambiar de proveedor o parámetros en la GUI.

---

## 6. Uso de los comandos de Antigravity (`/pov-mcp-plan`, `/pov-mcp-revise` y `/pov-mcp-review`) (Recomendado)

Desde **Configuración > Antigravity > Integración con Google Antigravity**, instale habilidades globales para utilizar comandos de barra en todos sus espacios de trabajo de Antigravity:

- Haga clic en **Instalar todos** (`antigravity.btnInstallAll`) o en **Instalar** (`antigravity.commandBtnInstall`) junto a cada comando.

### Crear un nuevo plan de implementación:
```text
/pov-mcp-plan <descripción de la tarea o función a implementar>
```
*Recopila el contexto del repositorio, invoca `poverello/plan` y se detiene de forma limpia tras presentar el plan, sin modificar archivos ni compilar.*

### Revisar un plan de implementación existente:
```text
/pov-mcp-revise <comentarios o nuevos requisitos a incorporar>
```
*Identifica el plan actual (contexto activo o `implementation_plan.md`), pasa el plan y los comentarios a `poverello/plan`, y actualiza el plan preservando las secciones no afectadas.*

### Revisar la implementación según el plan aprobado:
```text
/pov-mcp-review
```
*Revisa el plan aprobado, las diferencias del árbol de trabajo, los archivos relevantes sin seguimiento y los resultados de las pruebas, y presenta la revisión de la implementación. Es de solo lectura y no modifica archivos.*

> [!IMPORTANT]
> Al ejecutar mediante `/pov-mcp-plan`, `/pov-mcp-revise` o `/pov-mcp-review` (o los antiguos `/anthro-*`), el comando gestiona la llamada a la herramienta. Las reglas de Workspace no generarán llamadas duplicadas.

---

---

## 7. Comandos de Antigravity de nivel superior (`/pov-full-cycle`, `/pov-plan-only` y `/pov-build-review`)

Además de las habilidades MCP directas, Poverello proporciona tres habilidades de flujo de trabajo autónomas de nivel superior instaladas en `~/.gemini/antigravity/skills/`:

### Ciclo completo de desarrollo:
```text
/pov-full-cycle <descripción de la tarea>
```
*Gestiona el ciclo de principio a fin: redacción del plan, revisión externa independiente del plan, revisión de requisitos, implementación autorizada, verificación de compilación y pruebas, revisión externa independiente de la implementación y pases correctivos delimitados.*

### Ciclo de vida exclusivo de planificación:
```text
/pov-plan-only <descripción de la tarea>
```
*Dedicado a la redacción y revisión externa independiente del plan. Se detiene de forma limpia tras la aprobación del plan (`APPROVED` / `APPROVED_WITH_NON_BLOCKING_FINDINGS`) sin modificar archivos ni continuar hacia la implementación.*

### Ciclo de vida de compilación y revisión:
```text
/pov-build-review
```
*Implementa un plan ya aprobado, ejecuta conjuntos de pruebas, realiza la revisión externa independiente de la implementación y gestiona los límites de evidencia y las correcciones autorizadas.*
## 8. Automatización de la planificación con Workspace Rules

Puede crear una regla de espacio de trabajo opcional y específica del proyecto basándose en el ejemplo siguiente. Poverello no incluye ni instala una regla de planificación específica de DeepSeek; el proveedor y el modelo MCP configurados se seleccionan en los ajustes de Poverello:

```markdown
---
trigger: model_decision
description: Use for implementation, debugging, architectural changes, or multi-file code changes where an external implementation plan would be useful. Do not use for trivial text-only edits.
---

# Poverello Planner Rule (ejemplo)

For non-trivial implementation tasks in this repository:

1. If the current task is being executed through the `/pov-mcp-plan` or `/pov-mcp-revise` command (or legacy `/anthro-plan` / `/anthro-revise`), do NOT invoke `poverello/plan` (or `anthro-bridge/plan`) separately. The command workflow owns the planner call.
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

### Política de activación:
- **Tareas triviales / localizadas (Trivial / localized tasks)** (p. ej., corregir erratas, cambios de una línea, ajustes de sintaxis simples): No activan el planificador.
- **Tareas complejas (Non-trivial tasks)** (cambios arquitectónicos, funciones multi-archivo, depuración compleja): Antigravity inspecciona el repositorio, invoca `poverello/plan` 1 sola vez y realiza la implementación según el plan obtenido.

---

## 9. Flujo de trabajo automatizado típico

```text
Usuario: "Refactoriza la función X para admitir múltiples perfiles."
    ↓
Antigravity inspecciona el código y resume el contexto
    ↓
Antigravity activa automáticamente la llamada a poverello/plan (1 sola vez)
    ↓
Poverello envía la solicitud al modelo externo seleccionado
    ↓
Antigravity recibe el plan de implementación estructurado
    ↓
El usuario revisa y aprueba el plan
    ↓
Antigravity realiza los cambios en los archivos y ejecuta las pruebas
```

---

## 10. Notas importantes

- **Operación independiente**: El servidor MCP opera de forma completamente independiente de la pasarela 3P Gateway. La pasarela 3P no necesita estar encendida (ON) para que funcionen las llamadas MCP.
- **Facturación separada**: Las llamadas a `poverello/plan` generan costos de API facturados por el proveedor externo seleccionado. Las ediciones posteriores y las pruebas utilizan la cuota de suscripción de Antigravity.
- **Efecto inmediato**: Cambiar proveedores o parámetros de modelo en la GUI de Poverello surte efecto de inmediato en la siguiente invocación de `plan()`.

---
> [!NOTE]
> **La planificación y revisión de MCP utilizan proveedores en la nube**
>
> Antigravity MCP utiliza el proveedor de modelos en la nube activo configurado en Poverello (`mcp.provider` / `mcp.targets`). El backend **Ollama Local** está dedicado exclusivamente a **Claude Code CLI** y no afecta las operaciones de MCP. Para inferencia local con Claude Code, consulte la [Guía de Ollama Local](OLLAMA_LOCAL.es.md).
