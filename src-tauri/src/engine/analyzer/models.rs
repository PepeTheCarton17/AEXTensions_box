use serde::{Deserialize, Serialize};

/// Un título real encontrado en datos estructurados (schema.org ItemList).
/// Se usa cuando el catálogo HTML está vacío por tratarse de una SPA.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CatalogRecord {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub alternate_title: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub image: Option<String>,
}

/// Acción de búsqueda declarada por el sitio (schema.org SearchAction).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SiteSearchAction {
    #[serde(default)]
    pub url_template: String,
    #[serde(default)]
    pub query_param: Option<String>,
}

/// Parámetro de filtro realmente expuesto por el sitio en un enlace o formulario.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SiteFilterParam {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub param: String,
    #[serde(default)]
    pub value: String,
    /// `select` cuando hay varios valores; `check` cuando sólo uno.
    #[serde(default)]
    pub kind: String,
}

/// Filtro declarado por el sitio como una ruta de catálogo en vez de un query
/// param (`/latam/movies/category/action-adventure/`). La etiqueta y la ruta
/// son texto real del HTML; AEXT no inventa categorías ni sus nombres.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SitePathFilter {
    /// Nombre canónico en inglés (`Category`, `Genre`...), traducido al generar.
    #[serde(default)]
    pub name: String,
    /// Segmento de ruta que precede al valor (`category`, `genre`...).
    #[serde(default)]
    pub segment: String,
    /// Prefijo compartido por las opciones (`/latam/movies/`). Sirve para
    /// distinguir grupos con el mismo segmento en secciones distintas.
    #[serde(default)]
    pub base_path: String,
    /// Pares `{etiqueta visible, ruta absoluta del sitio}`.
    #[serde(default)]
    pub values: Vec<ApiOption>,
}

/// Un par `{id, name}` leído de una respuesta JSON real del sitio. Se usa para
/// poblar filtros con valores del propio sitio (géneros, categorías, países...),
/// nunca con listas inventadas por AEXT.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiOption {
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub label: String,
}

/// Bases, rutas y versión de API citadas en los bundles de un sitio.
#[derive(Debug, Clone, Default)]
pub(crate) struct ApiCandidates {
    pub(crate) bases: Vec<String>,
    pub(crate) paths: Vec<String>,
    pub(crate) version: Option<String>,
}

/// Un endpoint REST detectado en los bundles y verificado con un GET real.
/// Guarda tanto las opciones `{id, name}` como una muestra recortada del JSON y
/// los nombres de campo detectados, para que el generador construya catálogo y
/// fichas sin suponer un formato concreto.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiListResult {
    /// URL completa que respondió 200 con JSON.
    #[serde(default)]
    pub url: String,
    /// Último segmento de la ruta (`genres`, `categories`, `contents`...).
    #[serde(default)]
    pub resource: String,
    /// Pares `{id, name}` cuando la respuesta es una lista de opciones.
    #[serde(default)]
    pub options: Vec<ApiOption>,
    /// Muestra del JSON (recortada) para detectar campos de título/imagen/url.
    #[serde(default)]
    pub sample_json: String,
    /// Campo detectado para el título (`title`, `name`, ...).
    #[serde(default)]
    pub title_field: String,
    /// Campo detectado para la imagen (`poster_link`, `image`, ...).
    #[serde(default)]
    pub image_field: String,
    /// Campo detectado para la url/identificador (`permalink`, `id`, ...).
    #[serde(default)]
    pub url_field: String,
    /// Número de registros de la muestra, para saber si parece un catálogo.
    #[serde(default)]
    pub record_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Origen del sitio (esquema + host, sin ruta). Es lo que se usa como `baseUrl`.
    pub url: String,
    /// URL exacta que se analizó (puede incluir ruta).
    pub entry_url: String,
    pub name: String,
    pub lang: String,
    pub ext_type: String, // "anime" | "manga"
    pub cms: String,      // "sveltekit" | "dooplay" | "madara" | "mangathemesia" | "generic"
    pub favicon_url: Option<String>,
    pub detected_hosts: Vec<String>,
    pub has_sub_dub: bool,
    pub protections: Vec<String>,
    pub card_selector: String,
    pub title_selector: String,
    pub next_page_selector: String,
    /// `html` cuando hay tarjetas reales; `jsonld` cuando sólo hay datos
    /// estructurados; `none` cuando no hay evidencia de catálogo.
    #[serde(default)]
    pub catalog_source: String,
    /// Títulos reales leídos de JSON-LD. Nunca se inventan.
    #[serde(default)]
    pub catalog_records: Vec<CatalogRecord>,
    /// Búsqueda declarada por el sitio, si existe.
    #[serde(default)]
    pub search_action: Option<SiteSearchAction>,
    /// Filtros de consulta encontrados en enlaces/formularios del sitio.
    #[serde(default)]
    pub filter_params: Vec<SiteFilterParam>,
    /// Filtros declarados como rutas (categorías del footer/nav), no query params.
    #[serde(default)]
    pub path_filters: Vec<SitePathFilter>,
    /// Bases de API REST declaradas en los bundles (`baseURL: "https://..."`).
    #[serde(default)]
    pub api_bases: Vec<String>,
    /// Endpoints REST verificados con un GET real (listas `{id, name}`,
    /// catálogos...). Nunca se inventan: si no responden JSON, no se guardan.
    #[serde(default)]
    pub api_lists: Vec<ApiListResult>,
    /// Idioma principal declarado por la página (`<html lang>`, og:locale...).
    #[serde(default)]
    pub detected_language: String,
    /// Todos los idiomas que el sitio declara para su contenido/interfaz.
    /// Si hay más de uno, el paquete se marca como `all`: asignar `es` a un
    /// sitio multilingüe sería una afirmación falsa.
    #[serde(default)]
    pub site_locales: Vec<String>,
    /// Ruta detectada del listado/populares (p. ej. `/animes`). Nunca se inventa.
    pub popular_path: Option<String>,
    /// Ruta detectada de "últimos/recientes". Si no existe, `supportsLatest = false`.
    pub latest_path: Option<String>,
    /// Ruta del formulario de búsqueda detectado.
    pub search_path: Option<String>,
    /// Nombre del parámetro de búsqueda detectado (p. ej. `s`, `q`).
    pub search_param: Option<String>,
    /// Cómo pagina el sitio: `query`, `path` o `none`.
    pub page_style: String,
    /// True si la portada contiene tarjetas de catálogo reconocibles.
    pub catalog_detected: bool,
    /// Selector de episodios detectado en una ficha de ejemplo. Vacío si no se
    /// pudo comprobar; el generador usa entonces el selector genérico.
    #[serde(default)]
    pub episode_selector: String,
    /// Muestra de títulos encontrados en la portada (para confirmar en la UI).
    pub sample_items: Vec<String>,
    /// Avisos honestos sobre lo que no se pudo detectar o verificar.
    pub warnings: Vec<String>,
    /// Capacidades reales del sitio (formatos, calidades, idiomas, APIs).
    /// Se obtienen del HTML de entrada y de la ficha de ejemplo sondeada.
    #[serde(default)]
    pub capabilities: SiteCapabilities,
}

/// Lo que el sitio *realmente* expone para reproducción, no lo que la plantilla
/// supone. Se consume en el Kotlin generado para ordenar la verificación y en
/// la UI para dejar de afirmar capacidades que no se han visto.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SiteCapabilities {
    /// `hls` | `dash` | `mp4` | `iframe`, según aparezca en el HTML.
    #[serde(default)]
    pub stream_formats: Vec<String>,
    /// Calidades reales leídas de un máster HLS embebido (p. ej. `720p`).
    #[serde(default)]
    pub qualities: Vec<String>,
    /// Pistas de audio detectadas (`#EXT-X-MEDIA:TYPE=AUDIO`), con idioma.
    #[serde(default)]
    pub audio_languages: Vec<String>,
    /// Pistas de subtítulos detectadas (`TYPE=SUBTITLES`), con idioma.
    #[serde(default)]
    pub subtitle_languages: Vec<String>,
    /// Endpoints API/GraphQL encontrados en HTML/JS del sitio.
    #[serde(default)]
    pub api_endpoints: Vec<String>,
    /// Versión de app que el sitio declara (p. ej. `10.12.1096`). La estrategia
    /// de sesión la necesita para reproducir el arranque del reproductor web.
    #[serde(default)]
    pub app_version: Option<String>,
    /// Región y locale declarados por el sitio en su estado hidratado (p. ej.
    /// `MX` / `es-419`). Pluto condiciona catálogo, audio y subtítulos a la IP:
    /// se muestra como dato verificado, nunca como selector falso, porque
    /// `boot/start` ignora `country`, `region`, `deviceLat` y `deviceLon`.
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
    /// Verificación real contra un máster HLS de ejemplo. Sin esto, AEXT no
    /// debería ofrecer un selector SUB/DUB: no sabe si existe doblaje.
    #[serde(default)]
    pub master_verified: bool,
    #[serde(default)]
    pub sub_verified: bool,
    #[serde(default)]
    pub dub_verified: bool,
    /// Métodos de reproducción realmente encontrados, con nombres legibles.
    /// Se derivan de `player_cases` y `api_endpoints`; no son una API pública.
    #[serde(default)]
    pub playback_methods: Vec<String>,
    /// Caso de reproductor detectado, que decide el orden inicial de las
    /// estrategias: `session-bootstrap`, `graphql-persisted`, `hls-master`,
    /// `dash-mpd`, `iframe-player`, `drm-widevine`, `ads-hls`.
    #[serde(default)]
    pub player_cases: Vec<String>,
    /// Lo que no se pudo confirmar; se muestra como aviso, no como capacidad.
    #[serde(default)]
    pub unverified: Vec<String>,
}

pub struct SiteAnalyzer {
    pub(crate) client: reqwest::Client,
}

/// Resultado de abrir una página de detalle de ejemplo.
///
/// El generador usa selectores genéricos para detalle y episodios. Este sondeo
/// no los mejora: sirve para saber si el sitio expone episodios en el HTML y
/// avisar en la UI cuando no, en lugar de dar por hecho que la extensión
/// funcionará.
pub(crate) struct DetailProbe {
    pub(crate) url: String,
    pub(crate) fetched: bool,
    pub(crate) episode_links: usize,
    pub(crate) has_embed: bool,
    /// Selector CSS que sí encuentra los episodios en esa ficha.
    pub(crate) episode_selector: Option<String>,
    /// HTML de la ficha; permite detectar capacidades detrás del catálogo.
    pub(crate) html: String,
}

/// Resultado de abrir un máster HLS de muestra. Sólo se marca como verificado
/// si el cuerpo es una playlist real; de lo contrario AEXT no ofrece SUB/DUB.
#[derive(Debug, Default)]
pub(crate) struct MasterTrackProbe {
    pub(crate) attempted: bool,
    pub(crate) verified: bool,
    pub(crate) detail: String,
    pub(crate) audio_languages: Vec<String>,
    pub(crate) subtitle_languages: Vec<String>,
}
