[English](THIRD_PARTY_INFERENCE.md) | [日本語](THIRD_PARTY_INFERENCE.ja.md) | [中文(简体)](THIRD_PARTY_INFERENCE.zh-CN.md) | [中文(繁體)](THIRD_PARTY_INFERENCE.zh-TW.md) | [한국어](THIRD_PARTY_INFERENCE.ko.md) | [Français](THIRD_PARTY_INFERENCE.fr.md) | [Deutsch](THIRD_PARTY_INFERENCE.de.md) | [Español](THIRD_PARTY_INFERENCE.es.md)

# Usar Poverello con Claude Desktop / Cowork on 3P

Poverello se puede usar como una puerta de enlace local compatible con Anthropic para
Claude Desktop / Cowork on 3P.

Claude Desktop / Cowork on 3P admite inferencia de terceros a través de
la ventana de configuración integrada.

Documentación oficial:

- [https://claude.com/docs/cowork/3p/installation](https://claude.com/docs/cowork/3p/installation)
- [https://claude.com/docs/cowork/3p/configuration](https://claude.com/docs/cowork/3p/configuration)

## 1. Iniciar Poverello

Inicie Poverello primero y asegúrese de que la puerta de enlace esté en ejecución.

Por defecto, Poverello escucha en:

```text
http://127.0.0.1:4000
```

Mantenga Poverello abierto mientras usa Claude Desktop / Cowork on 3P.

## 2. Activar el modo de desarrollador en Claude Desktop

Abra Claude Desktop.

En Windows, abra el menú de la aplicación en la esquina superior izquierda.

Luego seleccione:

```text
Help → Troubleshooting → Enable Developer Mode
```

Después de activar el modo de desarrollador, aparecerá un nuevo menú `Developer`.

## 3. Abrir la configuración de inferencia de terceros

Abra:

```text
Developer → Configure third-party inference
```

Esto abre la ventana de configuración de inferencia de terceros.

## 4. Configurar la conexión

En la sección `Connection`, seleccione:

```text
Gateway
```

Luego ingrese los siguientes valores.

| Campo                 | Valor                                      |
| --------------------- | ------------------------------------------ |
| Gateway base URL      | `http://127.0.0.1:4000`                    |
| Gateway API key       | `sk-local-gateway`                         |
| Gateway auth scheme   | `bearer`                                   |
| Gateway extra headers | Déjelo en blanco a menos que necesite encabezados personalizados |

La `Gateway API key` debe coincidir con la API key local configurada en Poverello.

## 5. Configurar identidad y modelos

En la sección `Identity & Models`, agregue los IDs de modelo que Claude Desktop debe mostrar en el selector de modelos.

Ejemplo:

```text
claude-sonnet-4-6
claude-haiku-4-5
```

También puede asignar una etiqueta de visualización a cada modelo.

Ejemplo:

| ID del modelo        | Etiqueta          |
| -------------------- | ----------------- |
| `claude-sonnet-4-6`  | `Gateway Pro`     |
| `claude-haiku-4-5`   | `Gateway Flash`   |

El primer modelo de la lista se usa como entrada predeterminada del selector.

Para cada modelo, expanda la fila y confirme que el `Model ID` sea exactamente el nombre del modelo que desea que Claude Desktop envíe a Poverello.

Solo active `Offer 1M-context variant` si su proveedor upstream y el modelo seleccionado realmente admiten la ventana de contexto extendida.

## 6. Ejemplo de configuración

La configuración anterior corresponde a la siguiente configuración de inferencia de terceros:

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

## 7. Aplicar y reiniciar Claude Desktop

Después de configurar la puerta de enlace y la lista de modelos, aplique la configuración localmente.

Reinicie Claude Desktop si se le solicita.

Una vez que Claude Desktop se reinicie, las solicitudes de Cowork on 3P se enviarán a Poverello. Poverello luego enruta las solicitudes al proveedor upstream configurado en Poverello.

## API directa de DeepSeek

Cuando Poverello está configurado con el proveedor Direct DeepSeek, las solicitudes de Claude Desktop / Cowork on 3P se enrutan al endpoint de DeepSeek compatible con Anthropic Messages.

Los ID de API canónicos de DeepSeek son `deepseek-flash` y `deepseek-v4-pro`; la página oficial de precios actual los identifica como DeepSeek-V4.1-Flash-1017 y DeepSeek-V4-Pro-0813, respectivamente. Son ID de la API upstream. Los nombres que ve Claude en `inferenceModels` son entradas locales del selector que Poverello asigna al perfil del proveedor seleccionado.

Ambos modelos admiten llamadas a herramientas, un contexto de 1M tokens y hasta 384K tokens de salida. Solo Flash admite entrada de imágenes. DeepSeek sigue aceptando los ID retirados `deepseek-v4-flash` y `deepseek-v4-flash-vision-exp` como alias upstream de Flash. Los valores predeterminados de Direct DeepSeek de Poverello usan los ID canónicos. Las fuentes oficiales difieren sobre el enrutamiento y la facturación de Pro: la página actual publica tarifas Pro separadas y confirma la continuidad del servicio API después del 14 de septiembre de 2026, mientras que un aviso del 10 de septiembre describió el enrutamiento temporal a V4.1 Flash con tarifas Flash hasta el lanzamiento de V4.1-Pro. Poverello no puede verificar el enrutamiento ni la facturación de cada solicitud; muestra tarifas publicadas, no una factura ni una garantía. Véanse los [precios actuales](https://api-docs.deepseek.com/quick_start/pricing/) y el [aviso del 10 de septiembre](https://api-docs.deepseek.com/news/news260910/).

Consulte los [precios oficiales de DeepSeek](https://api-docs.deepseek.com/quick_start/pricing/) y la [guía de detalles de proveedores](PROVIDERS.md#deepseek) para conocer los precios actuales por token y las reglas de horas punta y valle. El calendario de festivos incluido en Poverello afecta a la estimación local; la cobertura y el comportamiento alternativo se explican en la guía de proveedores.

## Proveedor Xiaomi MiMo

Cuando Poverello está configurado para usar el proveedor Direct MiMo, las solicitudes de inferencia de Claude Desktop se enrutan a la API MiMo de Xiaomi.

### Modelos MiMo-V2.6

| Model ID | Nombre de visualización | Multimodal |
| --- | --- | --- |
| `mimo-v2.6-flash` | MiMo-V2.6-Flash | Image + Video |
| `mimo-v2.6-pro` | MiMo-V2.6-Pro | Image + Video |
| `mimo-v2.6-pro-ultraspeed` | MiMo-V2.6-Pro-UltraSpeed | Image + Video |

Todos los modelos V2.6 admiten una ventana de contexto de 1 000 000 tokens.

### Normal / Thinking

MiMo usa un simple interruptor Normal/Thinking. No hay niveles de esfuerzo de razonamiento (Low/Medium/High).

| Configuración | Parámetro upstream | Comportamiento |
| --- | --- | --- |
| Normal | `thinking: {"type": "disabled"}` | Inferencia estándar |
| Thinking | `thinking: {"type": "enabled"}` | Pensamiento extendido |

### Rutas predeterminadas

Las rutas integradas del preset Direct MiMo:

| Modelo de pasarela | Modelo upstream | Thinking |
| --- | --- | --- |
| `claude-opus-5` | `mimo-v2.6-pro` | Thinking |
| `claude-sonnet-5` | `mimo-v2.6-pro` | Normal |
| `claude-haiku-4-5` | `mimo-v2.6-flash` | Thinking |

### Compatibilidad con V2.5

Las configuraciones guardadas de `mimo-v2.5`, `mimo-v2.5-pro` y `mimo-v2.5-pro-ultraspeed` se conservan y siguen funcionando. Al iniciar, solo las rutas predeterminadas de fábrica sin modificar se migran automáticamente a V2.6. Las rutas personalizadas, la configuración del modo Thinking y los valores personalizados de `default_model` nunca se sobrescriben.

> [!NOTE]
> **Claude Desktop / Cowork on 3P utiliza proveedores en la nube mediante la pasarela**
>
> Claude Desktop y Cowork on 3P envían solicitudes de inferencia a la pasarela Poverello (`http://127.0.0.1:4000`), que las enruta hacia el proveedor en la nube configurado (DeepSeek, MiMo, Kimi, MiniMax, OpenRouter).
>
> El backend **Ollama Local** está estrictamente reservado para **Claude Code CLI** y está completamente aislado de Claude Desktop y MCP. Para configurar LLM locales con Claude Code, consulte la [Guía de Ollama Local](OLLAMA_LOCAL.es.md).

---

## Modo Thinking de MiniMax-M3

MiniMax-M3 permite activar o desactivar el thinking ampliado mediante la API compatible con Anthropic.

| Ajuste | Parámetro de la API upstream | Comportamiento |
| --- | --- | --- |
| Thinking activado | `thinking: {"type": "adaptive"}` | Activa el thinking ampliado |
| Thinking desactivado | `thinking: {"type": "disabled"}` | Desactiva el thinking ampliado |
| Predeterminado (sin definir) | *omitido* | Valor predeterminado de la API: thinking desactivado |

En Configuración, Poverello muestra un conmutador Thinking/Normal para MiniMax-M3. Los modelos MiniMax-M2.x (por ejemplo, `MiniMax-M2.7-highspeed`) no permiten desactivar thinking y permanecen en el modo exclusivo de thinking.

## Configuración de Laguna S/XS 2.1 (OpenRouter)

De forma predeterminada, Poverello dirige `claude-opus-5` y `claude-sonnet-5` de OpenRouter a `poolside/laguna-s-2.1`, y `claude-haiku-4-5` a `poolside/laguna-xs-2.1`.

### Comportamiento de thinking

Pruebas de terceros observaron que la estructura del prompt del sistema puede influir en el thinking de Laguna S 2.1. Los informes indican que una persona profesional clara y criterios de aceptación explícitos redujeron considerablemente el razonamiento innecesario. Por ello, el valor predeterminado de Laguna Opus (`claude-opus-5`) cambió a Normal (thinking desactivado); se puede volver a activar Thinking en Configuración.

### Limitación conocida: sin salida utilizable al alcanzar el límite de tokens

En ciertas condiciones —especialmente si el modelo alcanza el límite de tokens por turno durante el razonamiento— Laguna S 2.1 puede devolver solo contenido de razonamiento, sin texto ni llamada a herramientas, con `stop_reason: "max_tokens"`. Poverello detecta esta situación y registra una advertencia. Si aparece repetidamente “Reasoning-only response reached the per-turn token limit”, considere aumentar el máximo de tokens de salida o desactivar Thinking para ese modelo.

Estas observaciones proceden de pruebas comunitarias de terceros; los resultados pueden variar según la configuración del proveedor y la revisión del modelo.

## Notas

Poverello es una puerta de enlace local no oficial compatible con Anthropic.

No está afiliado con Anthropic, Moon Bridge o ningún proveedor de modelos upstream.

Claude Desktop / Cowork on 3P se configura a través de la configuración de inferencia de terceros de Claude. Las etiquetas de menú y los campos de configuración pueden cambiar a medida que Anthropic actualiza Claude Desktop.
