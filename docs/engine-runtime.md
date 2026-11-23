# Contrato runtime de motores instalables

AEXT puede incluir código Kotlin de un motor instalado desde un repositorio
público dentro de la extensión generada. El generador **no conoce** la lógica
del motor: sólo copia sus archivos, publica un contrato estable y registra la
clase de entrada declarada.

## Versión del contrato

`aext-stream-resolver/1`. Es la única versión que el bundle actual sabe alojar.
El manifiesto debe declararla en `runtime.api`.

## Manifiesto

Publica `aext-engine.json` en la raíz del repo (rama `main` o `master`) con:

| Campo | Significado |
| --- | --- |
| `version` | `1`. |
| `id` | Identificador único en minúsculas, dígitos, `-` o `_`. |
| `kind` | `playback` o `extractor`. Un motor `catalog` no puede declarar runtime de streams. |
| `capabilities` | Etiquetas genéricas que el router compara con el análisis del sitio. |
| `license` | Licencia explícita (no `unknown`). |
| `provenance` | URL del repo. AEXT la fija al instalar. |
| `files` | Rutas relativas de los `.kt` a copiar. Debe haber al menos uno. |
| `runtime.api` | `aext-stream-resolver/1`. |
| `runtime.package` | Paquete Kotlin de la clase de entrada (al menos dos segmentos). |
| `runtime.entry_class` | Nombre de la clase que implementa `AextStreamResolver`. |
| `probes` | Sondas HTTP anónimas. Sin cookies, autorización ni marcadores de sesión. |

Hay un ejemplo completo en `examples/aext-engine.json`.

## Qué recibe el motor

El bundle genera un archivo público `AextEngineContract.kt` con:

```kotlin
data class AextVideoContext(
    val url: String,
    val html: String,
    val endpoints: List<String>,
    val streams: List<String>,
    val client: OkHttpClient,
    val headers: Headers,
)

interface AextStreamResolver {
    val id: String
    fun canResolve(ctx: AextVideoContext): Boolean
    fun resolve(ctx: AextVideoContext): List<Video>
}
```

La implementación devuelve `List<Video>` con `eu.kanade.tachiyomi.animesource.model.Video`.
El pipeline ya existente verifica cada candidato antes de entregarlo al
reproductor, así que un motor no puede saltarse la comprobación del stream.

Los resolutores declarados entran en `orderedStrategies()` con id `engine:<id>`
y prioridad inicial alta. Si `canResolve` es falso, el pipeline sigue con el
HTML; si devuelve candidatos inválidos, el contador de aciertos/fallos lo baja
de puesto en la siguiente reproducción.

## Alcance y límites actuales

- AEXT no usa WebView. El motor recibe `OkHttpClient` y `Headers`; puede hacer
  peticiones HTTP directas con sus propios parámetros, sin sesión del usuario.
- El bundle ya incluye OkHttp, Jsoup y `kotlinx-serialization-json`. El
  manifiesto todavía no puede declarar dependencias Gradle adicionales.
- Un motor `playback` resuelve streams; **no sustituye al catálogo**. El
  catálogo sigue viniendo de sondas anónimas verificadas por `run_engine_trials`.
- El contrato se emite sólo en módulos de anime. Los módulos manga no tienen
  el modelo `Video` ni las dependencias del reproductor.
- Los archivos instalados viven en `<storage>/engines/<id>/`, fuera del árbol
  que se borra al actualizar una extensión.

## Verificación

El workflow genera seis proyectos de muestra y los compila con Gradle. Uno de
ellos contiene un resolutor declarado por un manifiesto de prueba, así que un
fallo del contrato rompe el job `Generated Extension Compiles` antes de
publicar una versión de AEXT.
