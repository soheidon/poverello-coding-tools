[English](../SPEC.md) | [日本語](SPEC.ja.md) | [中文(简体)](SPEC.zh-CN.md) | [中文(繁體)](SPEC.zh-TW.md) | [한국어](SPEC.ko.md) | [Français](SPEC.fr.md) | [Deutsch](SPEC.de.md) | [Español](SPEC.es.md)

# SPEC: Poverello Coding Tools

## Resumen

Una herramienta ligera de proxy + gestión con GUI que enruta solicitudes API de Claude Desktop / Claude Code a través de múltiples proveedores con endpoints compatibles con Anthropic.

### Arquitectura

```
[Claude Desktop / Cowork 3P]      [Google Antigravity]
             |                              | stdio (--mcp-server)
             v                              v
+-------------------------------------------------------------+
| Poverello Gateway (127.0.0.1:4000)                      |
| (axum 0.7 + reqwest proxy, API key check, route resolution) |
+-------------------------------------------------------------+
             |
             +---> Cloud Providers (DeepSeek / MiMo / Kimi / MiniMax / OpenRouter)

[Claude Code CLI] (active_route: "gateway")
             |
             v
+-------------------------------------------------------------+
| Poverello Gateway (127.0.0.1:4000)                      |
+-------------------------------------------------------------+
             |
             +---> Cloud Providers

[Claude Code CLI] (active_route: "ollama")
             |
             v (Loopback ANTHROPIC_BASE_URL)
+-------------------------------------------------------------+
| Ollama Local Backend (127.0.0.1:11434/v1)                   |
| (/v1/messages, thinking budget 1024, num_ctx override)      |
+-------------------------------------------------------------+
             |
             +---> Local Models (Gemma 4 / Qwen / Llama 3 / DeepSeek-R1)
```

#### Principios de diseño

- **LLM local dedicado para Claude Code**: Claude Code CLI puede usar de forma independiente una instancia local de Ollama en loopback (`http://127.0.0.1:11434/v1`) sin claves API. Se controla mediante `claude_code.active_route` (`gateway` u `ollama`) y está completamente aislado de Claude Desktop y MCP.
- **Modelo shell + selección de proveedor**: Claude Desktop siempre ve `claude-opus-5` / `claude-sonnet-5` / `claude-haiku-4-5`. El LLM real se selecciona en la GUI (DeepSeek / MiniMax / Kimi / MiMo / OpenRouter). El mapeo de modelos del proveedor activo se usa para el enrutamiento.
- **Soporte de OpenRouter**: Enruta hacia el endpoint compatible con Anthropic de OpenRouter con valores predeterminados de Poolside Laguna S/XS. Los controles dedicados de modo thinking (Max/On/Off) se traducen al formato `reasoning` de OpenRouter en el momento de la solicitud.
- **Solo el proveedor activo necesita API key**: Desde v0.5.0, solo se verifican los proveedores referenciados por la tabla de enrutamiento al iniciar. Las claves de proveedores inactivos no son requeridas.
- **Proxy delgado**: Nada se modifica excepto el campo `model`. SSE se reenvía byte por byte.
- **Reenvío sin pérdidas**: Cuerpos de mensajes, tool calls, bloques thinking pasan sin modificaciones.
- **GUI nativa de Windows**: Tauri v2 + React 19 + TypeScript. Backend en Rust, frontend Vite + React 19.
- **Cero dependencias externas**: Proxy incrustado en el binario de Tauri desde v0.3.0. Python no es necesario.
- **Multilingüe**: 8 idiomas (en, ja, zh-CN, zh-TW, ko, fr, de, es). Agregue nuevos idiomas colocando archivos en `lang/`. Selector de idioma en el primer inicio.
- **Modelos DeepSeek directos**: Los ID de API canónicos son `deepseek-flash` y `deepseek-v4-pro`; la página oficial de precios actual los identifica como DeepSeek-V4.1-Flash-1017 y DeepSeek-V4-Pro-0813, respectivamente. Ambos admiten el endpoint compatible con Anthropic Messages, llamadas a herramientas, contexto de 1M y hasta 384K tokens de salida. Flash admite visión; Pro no. DeepSeek aún acepta los ID retirados `deepseek-v4-flash` y `deepseek-v4-flash-vision-exp` como alias de Flash; los valores predeterminados de Poverello usan los ID canónicos. Las fuentes oficiales difieren sobre el enrutamiento y la facturación de Pro; Poverello muestra la tarifa publicada, pero no puede verificar la ejecución ni el cargo de cada solicitud. Véanse la [página de precios](https://api-docs.deepseek.com/quick_start/pricing/) y el [aviso del 10 de septiembre](https://api-docs.deepseek.com/news/news260910/).
- **Detección de capacidades**: Banderas de capacidad en vivo (supports_image_url, supports_image_base64, supports_video_url, supports_video_base64) obtenidas de la API de OpenRouter y persistidas en config.json.
- **Conocimiento de precios peak/valley**: Los rangos de horarios punta de DeepSeek y OpenRouter se muestran en la zona horaria local.
- **Razonamiento y calendario de precios de DeepSeek**: Low / High / Max se envía mediante `output_config.effort` en modo Thinking; Normal desactiva el razonamiento. Las horas pico son de lunes a viernes, 01:00–04:00 y 06:00–10:00 UTC, excepto festivos chinos; fines de semana y festivos son valle todo el día. Poverello incluye festivos y días laborables compensatorios chinos verificados de 2026 y muestra el estado `current`, `update_due` o `expired`. Los años no incluidos usan el fallback normal de día de semana/fin de semana/horario. DeepSeek no está incluido en las estimaciones de costes del Orchestrator.
- **Conmutador de thinking MiniMax-M3**: MiniMax-M3 admite Thinking ON/OFF mediante la API compatible con Anthropic (`thinking: {"type":"adaptive"}` / `{"type":"disabled"}`). Los modelos M2.x siguen siendo solo-thinking. La migración al iniciar convierte el `thinking_only` heredado → `thinking` para usuarios existentes.
- **Soporte de Xiaomi MiMo-V2.6**: MiMo-V2.6-Flash, MiMo-V2.6-Pro y MiMo-V2.6-Pro-UltraSpeed añadidos como modelos del proveedor Direct MiMo. Los tres tienen una ventana de contexto de 1.000.000 tokens y soporte multimodal nativo (texto, imagen, vídeo). Rutas predeterminadas integradas: Opus 5 → `mimo-v2.6-pro` (Thinking), Sonnet 5 → `mimo-v2.6-pro` (Normal), Haiku 4.5 → `mimo-v2.6-flash` (Thinking). MiMo usa únicamente el conmutador Normal/Thinking — sin niveles de esfuerzo de razonamiento (`supportsReasoningEffort: false`).
- **Compatibilidad con versiones anteriores de MiMo V2.5**: Los modelos heredados `mimo-v2.5`, `mimo-v2.5-pro` y `mimo-v2.5-pro-ultraspeed` permanecen reconocidos estáticamente con sus capacidades originales. Las rutas predeterminadas históricas no modificadas migran automáticamente a V2.6 al iniciar; los destinos de ruta, modos thinking y configuraciones default_model personalizados por el usuario se conservan. Las configuraciones heredadas que usan `model_map` sin objeto `models` resuelven las capacidades por ruta desde el resolvedor de capacidades estático (`try_resolve_static_model_capabilities`) en lugar de recurrir a los indicadores globales del proveedor, garantizando que las rutas V2.6 reciban la capacidad de vídeo correcta incluso en configuraciones heredadas.
- **Normalización de la identidad del modelo de respuesta**: Reescribe los nombres de los modelos upstream en las respuestas de la API (tanto streaming SSE como no streaming) de vuelta a los nombres oficiales de modelos de Anthropic. Se controla mediante `normalize_response_model_identity` en config.json y un `AtomicBool` en tiempo de ejecución. Comando de guardado independiente (`update_normalize_model_identity`) para evitar contaminación cruzada con los guardados de la configuración del servidor.
- **Registro de comunicación estructurado**: `tracing` + `tracing-appender` escriben registros estructurados en `%APPDATA%\Poverello\Communication-Logs\proxy-*.log`. Cada solicitud recibe un ID de correlación de un contador `AtomicU64`. Las entradas de registro incluyen modelo solicitado, modelo pasarela, modelo upstream, resultado de la normalización y motivos de omisión. No se registran datos sensibles (prompts, cuerpos, API keys).
- **Insignia PEAK**: Insignia rosa con código de color en el panel principal para modelos con precio punta.
- **Visualización de offset UTC**: El selector de zona horaria muestra offsets UTC dinámicos (p. ej. UTC+09:00) junto a cada opción.
- **Detección de fallo por límite de tokens de Laguna S/XS 2.1**: Detecta respuestas de solo razonamiento con `stop_reason: "max_tokens"` tanto en flujos SSE como en respuestas no streaming. Registra una advertencia cuando se alcanza el límite de tokens por turno sin producir texto utilizable ni tool calls. Disponible para todos los modelos Poolside Laguna mediante OpenRouter.
- **Paso a través de thinking:disabled de Poolside**: Traduce el `thinking: { type: "disabled" }` enviado por el cliente al formato `reasoning: { enabled: false }` de OpenRouter para los modelos Poolside, asegurando que el thinking desactivado se reenvíe correctamente incluso sin una configuración guardada.
- **Migración del valor predeterminado de Laguna Opus**: Una migración idempotente de una sola vez cambia el valor predeterminado de `claude-opus-5` de thinking-activado a modo normal para los usuarios de OpenRouter con `poolside/laguna-s-2.1`. La plantilla de instalación nueva refleja el valor predeterminado actualizado.
- **Multiperfil de OpenRouter**: Múltiples perfiles de OpenRouter por usuario, cada uno con su propia API key y configuración de modelos. CRUD de perfiles mediante comandos Tauri. Cambio de perfil activo desde el panel principal o la configuración. Los perfiles se pueden reordenar mediante arrastrar y soltar, ocultar y persistir en el orden configurado.
- **Tarjetas de OpenRouter en el panel principal**: El panel principal crea una tarjeta por perfil de OpenRouter visible, con una tarjeta de respaldo cuando no hay perfiles. Los resúmenes de modelos ocultan el espacio de nombres del proveedor antes de la primera `/` solo para la visualización en OpenRouter; los ID upstream completos permanecen sin cambios para el enrutamiento.
- **Registro de modelos de OpenRouter**: Registro integrado local de modelos conocidos de OpenRouter (`model_capabilities.rs`, `builtinOpenRouter.ts`) con capacidades preconfiguradas (visión, video, política de thinking, nivel de razonamiento), agrupación por proveedor y datos de precios. Se usa para la clasificación de modelos sin llamadas en vivo a la API.
- **Detalles de precios de OpenRouter**: Los precios integrados admiten valores actuales y estándar revisados para tarifas de prompt, salida y entrada en caché, incluidas las variantes GPT-5.6 Sol, Terra, Luna y Pro. La GUI muestra las tarifas promocionales y estándar juntas cuando ambas están disponibles.
- **Soporte de modelos GPT-5.6**: Los perfiles de OpenRouter pueden usar las variantes de modelo Sol, Terra y Luna, con controles de thinking conscientes de las capacidades y notas de precios para tarifas de contexto largo cuando corresponda. El perfil integrado OpenAI GPT-5.6 Balanced enruta Opus 5 → GPT-5.6 Sol, Sonnet 5 → GPT-5.6 Terra y Haiku 4.5 → GPT-5.6 Luna con nivel de razonamiento Thinking High en las tres rutas para instalaciones nuevas; el enrutamiento guardado existente no se modifica automáticamente.
- **Tamaño de ventana basado en el panel principal**: El cambio inicial y de recuento de filas calcula la altura de la ventana a partir de las tarjetas visibles del panel principal en una cuadrícula de tres columnas. El cálculo tiene en cuenta la altura de las tarjetas, los espacios de la cuadrícula, el tamaño mínimo nativo, el área de trabajo del monitor, la escala DPI y los adornos de la ventana, preservando el redimensionado manual cuando el recuento de filas no cambia.
- **Instalador NSIS localizado**: El instalador de Windows expone opciones de idioma en inglés, japonés, chino simplificado, chino tradicional, coreano, francés, alemán y español, e incluye el icono de la aplicación Poverello.
- **Cobertura de regresión**: La cobertura de Vitest incluye el ordenamiento de perfiles de OpenRouter y las condiciones de carrera de guardado, los datos de precios de producción, la semántica del recuento de tarjetas del panel principal y el tamaño de ventana consciente del monitor.
- **Nuevos proveedores mediante OpenRouter**: InclusionAI y StepFun añadidos como proveedores de modelos de OpenRouter con banderas de capacidad dedicadas, controles de modo thinking y agrupación por proveedor.
- **Modos de thinking de Tencent Hy3**: Soporte de nivel de razonamiento Low/High para el modelo Hunyuan de Tencent. La traducción del modo thinking en proxy.rs asigna `thinking_mode` al formato `reasoning` de OpenRouter. La UI muestra Low/High como opciones de desplegable.
- **Correcciones de Kimi K3**: Se eliminó el `forced_reasoning_effort` codificado de las definiciones de capacidades. Se reemplazó la visualización fija "Max" por un selector desplegable configurable. Los valores predeterminados provienen de la configuración guardada, con respaldo a "max".
- **Serialización de escritura de configuración**: Todos los comandos Tauri que escriben configuración se serializan a través de `execute_serialized_config_mutation` con un guard `Mutex`. La estructura `ConfigState` proporciona seguimiento de `applied_config`, `in_flight_config` y `pending_ops` con validación. Evita condiciones de carrera cuando se guardan varios cambios de configuración de forma concurrente.
- **Correcciones de condiciones de carrera en la UI de OpenRouter**: (1) El ref de último callback `syncUiFromSavedRouteRef` evita que un closure obsoleto sobrescriba la UI de la nueva ruta. (2) El guard `rollbackRouteId` evita la reversión de la Fase 2 entre rutas. (3) El hook `useRouteSaveGeneration` proporciona guards de generación `begin()`/`isCurrent()` para todos los handlers. (4) Hook de cola de guardado (`useOpenRouterSaveQueue`) con bucle de drenaje, detección de superposición y reinicio de la agregación de OR.
- **Aislamiento de identidad de app dev/estable**: El enum `AppChannel` (`Stable`/`Dev`) en `paths.rs` selecciona identificadores separados (`com.soheidon.anthro-bridge` vs `.dev`), directorio de configuración (`Poverello` vs `Poverello Dev`) y rutas de caché. El canal Dev usa `tauri.dev.conf.json`. Scripts NPM: `npm run dev` (dev), `npm run dev:stable` (estable).
- **Incrustación de plantilla de configuración**: `include_str!()` incrusta `config_template.rs` en tiempo de compilación, eliminando la dependencia en tiempo de ejecución del `config.json` incluido. `merge_bundled_providers` devuelve un `Result` con manejo de errores tipado.
- **Pruebas de regresión del frontend**: 7 pruebas de regresión de vitest para condiciones de carrera de guardado de OpenRouter usando `QueueHarness` y `GenerationHandlerHarness`. Las pruebas cubren: ref de último callback, guard de reversión entre rutas, captura de identidad, reintento de actualización (rutas de fallo + éxito), superposición en vuelo y guard de generación.
- **Gestión del contexto de Claude Code**: Auto-compactación consciente del modelo para Claude Code. `resolve_effective_auto_compact` resuelve cada ruta estándar (claude-opus-5, claude-sonnet-5, claude-haiku-4-5) a su modelo upstream, busca la capacidad de contexto de cada modelo en el registro estático `model_catalog.json` y, en modo Auto, usa la capacidad conocida más pequeña como ventana de contexto segura. El control de contexto se aplica solo cuando las tres capacidades son conocidas (de lo contrario, el estado es Incomplete). Un conmutador del encabezado activa/desactiva la gestión del contexto; los modos avanzados y los umbrales se establecen en `config.json` bajo `claude_code.auto_compact`. Modos: `auto`, `manual` (`window_tokens`), `claude_default`.
- **Generación del comando de lanzamiento de Claude Code**: `build_claude_code_launch_command` genera un comando PowerShell completo que combina las variables de conexión de la pasarela (`ANTHROPIC_BASE_URL` apuntando a la pasarela local, `ANTHROPIC_AUTH_TOKEN` = `sk-local-gateway`) con las variables de control de contexto de Claude Code (`CLAUDE_CODE_AUTO_COMPACT_WINDOW`, `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE`). Cuando la gestión del contexto está desactivada, incompleta o configurada como valor predeterminado de Claude, el comando elimina las variables de contexto obsoletas con `Remove-Item Env:... -ErrorAction SilentlyContinue` para que los valores de sesión configurados previamente no se filtren a un nuevo lanzamiento. El botón "Copiar comando de lanzamiento de Claude Code" del panel de configuración de Claude copia el comando al portapapeles. Poverello solo genera y copia el comando — nunca lo ejecuta.
- **Módulo de enrutamiento de modelos compartido**: `model_routing.rs` extrae la resolución de ruta-a-upstream en funciones puras compartidas por `proxy.rs` y el resolvedor de contexto, garantizando que las ventanas de contexto resuelvan los mismos modelos upstream a los que el proxy realmente reenvía.
- **Registro de capacidad de contexto**: `model_catalog.json` es un registro estático de capacidades de contexto conocidas que cubre los modelos integrados de proveedores directos (DeepSeek, MiniMax, Kimi, MiMo V2.6 y V2.5) y los modelos integrados de OpenRouter (Poolside, Tencent, InclusionAI, StepFun, OpenAI GPT-6.1, GPT-6, GPT Astra Latest, GPT-5.6 y Google Gemini). Los modelos personalizados desconocidos de OpenRouter siguen siendo objetivos de ruta válidos, pero informan la gestión del contexto como Incomplete hasta que se agregue metadatos o se configure el modo manual.
- **Modelos Google Gemini**: Los perfiles de OpenRouter incluyen Gemini 3.1 Pro Preview y Gemini 3.7 Flash con datos integrados de precios y capacidades. Gemini 3.7 Flash muestra el precio promocional junto al precio habitual tachado.
- **Auto-Converge en v0.24.6**: El flujo completo de Auto-Converge y su pestaña propia están ocultos y no disponibles. Se conserva el código de implementación, pero no es una función operativa compatible en esta versión. Los flujos acotados de planificación y revisión del Orchestrator siguen disponibles y no implementan código automáticamente.
- **Proveedor directo GLM (Z.ai)**: Envía solicitudes al endpoint compatible con Anthropic de Z.ai usando `ZAI_API_KEY` configurada (o la clave guardada en Poverello). Poverello no determina el tipo de clave, compras de Coding Plan, autorización de cuenta ni elegibilidad de facturación; consulte la [guía de configuración actual de Z.ai](https://zcode.z.ai/en/docs/configuration). Catálogo: `glm-5.3`, `glm-5.3-flash`, `glm-5.3-flashx` y `glm-5.2`. GLM-5.3 siempre usa thinking con esfuerzo seleccionable (`low`, `high`, `max`); GLM-5.2 ofrece el conmutador Normal/Thinking. Las respuestas vacías o solo de thinking fallan de forma segura con un error explícito en Orchestrator y MCP.
- **Gestión de reglas de Antigravity**: La pestaña “Antigravity Rules” administra `pov-direct.md` mediante una máquina de estados que solo modifica el escalar YAML `trigger` y el bloque administrado delimitado, preservando el frontmatter, comentarios e inclusiones de Skills del usuario. Incluye copias de seguridad persistentes y resistentes a colisiones, comprobación SHA-256 y de conflictos por eliminación externa, rollback verificado, protección ante carpetas sidecar heredadas y límite de 24.000 bytes UTF-8. Localizada en ocho idiomas.
- **Proveedor directo oficial de Anthropic**: Acceso directo a `https://api.anthropic.com` mediante `ANTHROPIC_API_KEY`, reenvío del protocolo Messages nativo sin traducción, catálogo validado, diagnóstico de límite de tokens en streaming y no streaming, y validación nativa de parámetros de muestreo.
- **Perfiles de gateway multiproveedor (`gateway_profile`)**: Asigna proveedor y modelo independientes a cada ruta Claude canónica (`claude-opus-5`, `claude-sonnet-5`, `claude-haiku-4-5`). Se almacena mediante `active_provider` y `active_gateway_profile_id`, con perfiles en `gateway_profiles`; se integra con la comprobación de capacidades y el cálculo de contexto para auto-compact de Claude Code.
- **Namespace canónico de Skills y MCP de Poverello**: Herramientas `poverello/plan`, `poverello/revise`, `poverello/review` y comandos `/pov-plan`, `/pov-revise`, `/pov-review`. Poverello registra aditivamente el servidor `poverello` en Antigravity y mantiene `anthro-bridge` y los Skills `/anthro-*` heredados por compatibilidad. Gestión segura de archivos individuales.
- **Uso normalizado y motor de costes financieros del Orchestrator**: Registra los campos de uso admitidos (`input_tokens`, `output_tokens`, `raw_total_tokens`, `cache_read_tokens`, `cache_write_tokens`) en RunJournal y auditorías; los valores ausentes o no válidos siguen siendo desconocidos, y la creación de caché se serializa como `cacheWriteTokens`. Actualmente solo estima dispatches Planner–Reviewer elegibles con Anthropic Direct Opus 5.5/Sonnet 5.5 y buckets de caché explícitamente en cero; los demás casos no tienen precio y no equivalen a coste cero. Usa nano-dólares enteros (`amount_nanos: u64`) serializados como cadenas decimales, sin aritmética de coma flotante. Verifica exactamente ID de tarifa, proveedor, modelo y moneda; las tarifas desconocidas o discordantes fallan de forma segura como “unpriced”. `RunCostSummary` puede ser `Complete`, `Partial` o `Unavailable`; `list_journals()` recalcula las sumas en cada lectura a partir de registros de dispatch inmutables.

### Herramienta de gestión con GUI

Tauri v2 + React 19 + TypeScript. Diseño de dos paneles: Panel principal + Configuración.

```
+------------------------------------------+
|  Poverello                   |
|  [Iniciar/Detener pasarela] [Estado] [=] |
+------------------------------------------+
|  Panel principal                         |
|  +- Seleccionar proveedor LLM ----------+|
|  | [DeepSeek] [MiMo] [MiniMax] [Kimi]   ||
|  +- Estado ------------------------------+
|  | Puerto 4000 | API Key | URL pasarela  ||
|  | Tabla de enrutamiento de modelos      ||
|  +- Último registro ---------------------+
|  | Visor de registros con contadores     ||
|  +---------------------------------------+
+------------------------------------------+

Configuración (=):
  +- Idioma -------------------------------+
  | Desplegable para cambio instantáneo    |
  +- API Key ------------------------------+
  | Gestión de API key por proveedor       |
  +- Configuración Claude Desktop ---------+
  | Generación de JSON de configuración,   |
  | copia, detección de archivo de config  |
  +- Configuración de la pasarela ---------+
  | Editor de config.json (avanzado)       |
  +---------------------------------------+
```

### Comandos Tauri

| # | Comando | Tipo | Descripción |
|---|---------|------|-------------|
| 1 | `check_health` | async | Verificación de salud del proxy |
| 2 | `check_gateway_status` | sync | Puerto 4000 + vivacidad de tarea tokio |
| 3 | `check_api_key` | sync | Estado de la API key del proveedor activo |
| 4 | `set_env_api_key` | sync | Persistir API key mediante setx |
| 5 | `get_port_4000_process` | sync | Obtener PID del puerto 4000 vía netstat |
| 6 | `read_config` | sync | Leer config.json |
| 7 | `read_config_raw` | sync | Texto raw de config.json + detección de codificación |
| 8 | `write_config` | sync | Guardar config.json (UTF-8 / Shift-JIS) |
| 9 | `read_latest_log` | sync | Leer último registro |
| 10 | `read_log` | sync | Leer archivo de registro especificado |
| 11 | `list_logs` | sync | Listar archivos de registro |
| 12 | `create_new_log` | sync | Crear nuevo archivo de registro |
| 13 | `open_logs_folder` | sync | Abrir carpeta de registros |
| 14 | `open_path` | sync | Abrir ruta arbitraria |
| 15 | `find_claude_configs` | sync | Detectar automáticamente archivos de configuración de Claude Desktop |
| 16 | `start_proxy` | sync | Iniciar proxy (resolver config -> iniciar -> verificar puerto) |
| 17 | `stop_proxy` | sync | Detener proxy (apagado graceful) |
| 18 | `proxy_status` | sync | Verificar vivacidad de tarea |
| 19 | `check_all_api_keys` | sync | Estado de API keys de todos los proveedores |
| 20 | `update_active_provider` | sync | Guardar active_provider |
| 21 | `update_provider_api_key_env` | sync | Guardar provider api_key_env |
| 22 | `get_user_language` | sync | Obtener preferencia de idioma guardada |
| 23 | `set_user_language` | sync | Guardar preferencia de idioma |
| 24 | `is_first_run` | sync | Determinar primer inicio (existencia de user_prefs.json) |
| 25 | `openrouter_get_models` | async | Obtener/cachear catálogo de modelos de OpenRouter |
| 26 | `set_model_upstream` | sync | Guardar modelo upstream + configuración de thinking + banderas de capacidad para un modelo de pasarela |
| 27 | `update_server_config` | sync | Guardar configuración de host/puerto/CORS del servidor |
| 28 | `update_normalize_model_identity` | sync | Guardar el conmutador de normalización de la identidad del modelo de respuesta (actualiza config + AtomicBool en tiempo de ejecución) |
| 29 | `update_claude_code_auto_compact_global` | sync | Conmutar la gestión del contexto global de Claude Code (activada + porcentaje de activación) |
| 30 | `update_claude_code_auto_compact_target` | sync | Establecer el modo de contexto por proveedor/perfil (auto / manual / claude_default) + tokens de ventana manuales |
| 31 | `update_claude_code_context_settings` | sync | Actualización atómica combinada de la configuración de contexto global + objetivo |
| 32 | `resolve_claude_code_auto_compact` | sync | Resolver la configuración de contexto efectiva (modo, tokens de ventana, porcentaje de activación, estado) |
| 33 | `build_claude_code_launch_command` | sync | Generar el comando completo de lanzamiento de Claude Code en PowerShell (variables de entorno de pasarela + contexto) |
| 34 | `fetch_ollama_models` | async | Discover local models from loopback `GET http://127.0.0.1:11434/api/tags` (2s bounded timeout) |
| 35 | `get_claude_code_config` | sync | Read Claude Code routing, auto_compact, and third_party_provider settings |
| 36 | `update_claude_code_third_party` | sync | Save Ollama Local configuration (models, thinking_modes, context_windows, show_on_dashboard) |
| 37 | `set_claude_code_active_route` | sync | Set Claude Code active route (`gateway` or `ollama`) |

### Servidor Proxy (proxy.rs)

Portado de Python a Rust (axum 0.7/reqwest) en v0.3.0.

#### Endpoints

| Método | Ruta | Comportamiento |
|--------|------|----------------|
| GET | `/health` | Verificación de salud |
| GET | `/v1/models` | Lista pública de modelos (solo `visible: true`) |
| POST | `/v1/messages` | Resolución de modelo -> inyección thinking -> verificación multimedia -> reenvío (stream/non-stream) |
| POST | `/v1/messages/count_tokens` | Reenviar a upstream si es compatible |

#### Enrutamiento de modelos

Construye una tabla de búsqueda inversa de modelo de pasarela -> (proveedor, modelo upstream) usando la sección `models` de cada proveedor. Como todos los proveedores usan los mismos nombres de modelo de pasarela, `active_provider` gana en caso de colisión. Efectivamente, solo los modelos del proveedor activo terminan en la tabla de enrutamiento.

#### Validación de API key (desde v0.5.0)

Paso 1: Construir tabla de enrutamiento de modelos (no se necesitan API keys)
Paso 2: Solo verificar API keys de proveedores referenciados por la tabla de enrutamiento

#### Inyección de thinking

Para modelos con `thinking: "disabled"` en su entrada de configuración, inyecta `{"type": "disabled"}` solo cuando el usuario no ha configurado thinking explícitamente.

#### Normalización del modelo de respuesta

Cuando `normalize_response_model_identity` está habilitado, el proxy reescribe el campo `model` en las respuestas upstream:

- **No streaming**: Analiza la respuesta JSON, reescribe `model` al nombre canónico de Anthropic, re-serializa
- **Streaming (SSE)**: Intercepta los marcos de eventos `message_start`, reescribe `model` in-place mediante reemplazo por rango de bytes para preservar el formato SSE y los espacios en blanco
- **Motivos de omisión**: `disabled` (conmutador desactivado), `non_success_status` (respuesta distinta de 200), `content_encoding_not_transformable` (gzip/brotli), `stream_error`, `stream_cancelled`
- **Lógica de decisión**: Funciones puras (`should_normalize_nonstream`, `nonstream_skip_reason`) usadas tanto por el código de producción como por las pruebas

#### Verificación multimedia / Sanitización de imágenes

Las banderas `supports_vision` / `supports_video` por modelo determinan el comportamiento. Para modelos sin soporte de visión que reciben imágenes, se aplica `non_vision_image_policy`:
- `replace` (predeterminado): Reemplazar bloques de imagen con texto de marcador
- `drop`: Eliminar bloques de imagen (insertar marcador si el contenido queda vacío)
- `reject`: Retornar error 400

Los bloques de video siempre retornan 400. `non_vision_image_policy` es visible a través de `/health`.

#### Gestión del contexto de Claude Code

El control de contexto de Claude Code usa dos variables de entorno oficiales:

```
CLAUDE_CODE_AUTO_COMPACT_WINDOW
CLAUDE_AUTOCOMPACT_PCT_OVERRIDE
```

Proceso de resolución:

1. Resolver cada ruta estándar (claude-opus-5, claude-sonnet-5, claude-haiku-4-5) a su modelo upstream
2. Buscar la capacidad de contexto de cada modelo upstream en `model_catalog.json`
3. Exigir que se conozcan las tres capacidades
4. Usar la capacidad conocida más pequeña como ventana de contexto segura
5. Aplicar el porcentaje de activación configurado

Modos: `auto` (capacidad conocida más pequeña), `manual` (`window_tokens`), `claude_default` (valor predeterminado propio de Claude Code; no se establece ninguna variable). El estado efectivo es `applied`, `disabled` o `incomplete`.

El comando de lanzamiento combina las variables de conexión de la pasarela con las variables de contexto:

```powershell
$env:ANTHROPIC_BASE_URL='http://127.0.0.1:4000'; $env:ANTHROPIC_AUTH_TOKEN='sk-local-gateway'; $env:CLAUDE_CODE_AUTO_COMPACT_WINDOW='262144'; $env:CLAUDE_AUTOCOMPACT_PCT_OVERRIDE='90'; claude
```

Cuando no se aplica el control de contexto, el comando primero elimina las variables obsoletas:

```powershell
Remove-Item Env:CLAUDE_CODE_AUTO_COMPACT_WINDOW -ErrorAction SilentlyContinue;
Remove-Item Env:CLAUDE_AUTOCOMPACT_PCT_OVERRIDE -ErrorAction SilentlyContinue;
```

El porcentaje de anulación solo adelanta la compactación; los valores que retrasarían la compactación más allá del valor predeterminado de Claude Code pueden ignorarse. Poverello solo genera y copia el comando — nunca lo ejecuta, y esto no prueba que una versión específica de Claude Code respete las variables (la confirmación final requiere diagnósticos de Claude Code o el comportamiento de compactación observado).

### Multilingüe

Arquitectura de archivo-por-idioma con auto-descubrimiento de `import.meta.glob`:

```
gui/src/i18n/lang/
  en.ts      Inglés (canónico — define el tipo TranslationKey)
  ja.ts      Japonés
  zh-CN.ts   Chino (Simplificado)
  zh-TW.ts   Chino (Tradicional)
  ko.ts      Coreano
  fr.ts      Francés
  de.ts      Alemán
  es.ts      Español
```

Para agregar un idioma: copiar `en.ts`, traducir, reconstruir. No se necesitan cambios de código.

### Referencia de config.json

```json
{
  "active_provider": "deepseek",
  "providers": {
    "<provider_id>": {
      "display_name": "Display name",
      "upstream_url": "Anthropic-compatible API base URL",
      "api_key_env": "API key env var name",
      "default_model": "Fallback model name",
      "force_anthropic_version": null,
      "supports_count_tokens": false,
      "supports_vision": false,
      "supports_video": false,
      "model_map": { "claude-sonnet-4-5": "real-model-name" },
      "visible_models": ["claude-public-model-name"],
      "models": {
        "claude-sonnet-4-6": {
          "upstream_model": "real-model-name",
          "thinking_mode": "normal",
          "reasoning_effort": "high",
          "supports_vision": true,
          "supports_video": true,
          "visible": true
        }
      }
    },
    "openrouter": {
      "display_name": "OpenRouter",
      "upstream_url": "https://openrouter.ai/api/v1",
      "api_key_env": "OPENROUTER_API_KEY",
      "default_model": "openrouter/auto",
      "models": {
        "claude-opus-5": {
          "upstream_model": "poolside/laguna-s-2.1",
          "thinking_mode": "thinking",
          "reasoning_effort": "max",
          "supports_image_url": false,
          "supports_image_base64": false,
          "supports_video_url": false,
          "supports_video_base64": false
        },
        "claude-sonnet-5": {
          "upstream_model": "poolside/laguna-s-2.1",
          "thinking_mode": "normal",
          "supports_image_url": false,
          "supports_image_base64": false,
          "supports_video_url": false,
          "supports_video_base64": false
        },
        "claude-haiku-4-5": {
          "upstream_model": "poolside/laguna-xs-2.1",
          "thinking_mode": "thinking",
          "supports_image_url": false,
          "supports_image_base64": false,
          "supports_video_url": false,
          "supports_video_base64": false
        }
      }
    }
  },
  "non_vision_image_policy": "replace",
  "normalize_response_model_identity": true,
  "server": { "host": "127.0.0.1", "port": 4000, "enable_cors": false },
  "claude_code": {
    "auto_compact": {
      "enabled": false,
      "trigger_percent": 90
    }
  }
}
```

Cada proveedor o perfil de OpenRouter también puede establecer un modo de contexto predeterminado mediante `claude_code: { "auto_compact": { "mode": "auto" } }`. El modo efectivo de una ruta es el valor del proveedor/perfil, con respaldo al bloque global; `resolve_claude_code_auto_compact` devuelve el resultado resuelto.
