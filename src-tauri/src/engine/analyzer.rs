use serde::{Deserialize, Serialize};
use scraper::{ElementRef, Html, Selector};
use std::collections::{HashMap, HashSet};
use regex::Regex;
use url::Url;

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
struct ApiCandidates {
    bases: Vec<String>,
    paths: Vec<String>,
    version: Option<String>,
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
    client: reqwest::Client,
}

/// Resultado de abrir una página de detalle de ejemplo.
///
/// El generador usa selectores genéricos para detalle y episodios. Este sondeo
/// no los mejora: sirve para saber si el sitio expone episodios en el HTML y
/// avisar en la UI cuando no, en lugar de dar por hecho que la extensión
/// funcionará.
struct DetailProbe {
    url: String,
    fetched: bool,
    episode_links: usize,
    has_embed: bool,
    /// Selector CSS que sí encuentra los episodios en esa ficha.
    episode_selector: Option<String>,
    /// HTML de la ficha; permite detectar capacidades detrás del catálogo.
    html: String,
}

/// Resultado de abrir un máster HLS de muestra. Sólo se marca como verificado
/// si el cuerpo es una playlist real; de lo contrario AEXT no ofrece SUB/DUB.
#[derive(Debug, Default)]
struct MasterTrackProbe {
    attempted: bool,
    verified: bool,
    detail: String,
    audio_languages: Vec<String>,
    subtitle_languages: Vec<String>,
}

impl SiteAnalyzer {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            // Sin límite, un sitio lento deja el análisis colgado en la UI; el
            // sondeo de la ficha añade una petición más.
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self { client }
    }

    pub async fn analyze(&self, url: &str) -> Result<AnalysisResult, String> {
        let (mut result, demo_href) = self.analyze_page(url).await?;

        // Muchos reproductores (SPAs) sólo mencionan sus endpoints en bundles
        // JS. Los leemos aparte: `Html` no es Send y no puede cruzar un await
        // dentro de `analyze_page`, pero aquí sólo guardamos cadenas.
        let mut js_api_candidates = ApiCandidates::default();
        if let Some((js_caps, js_filters, api_candidates)) = self
            .scan_scripts(&result.entry_url, result.search_param.as_deref())
            .await
        {
            Self::merge_capabilities(&mut result.capabilities, &js_caps);
            js_api_candidates = api_candidates;
            let mut added: Vec<String> = Vec::new();
            for filter in js_filters {
                let duplicate = result.filter_params.iter().any(|old| {
                    old.param == filter.param
                        && (old.value == filter.value || filter.kind == "text")
                });
                if duplicate {
                    continue;
                }
                if !added.contains(&filter.param) {
                    added.push(filter.param.clone());
                }
                result.filter_params.push(filter);
            }
            if !added.is_empty() {
                result.warnings.push(format!(
                    "Filtros encontrados en los bundles JS del sitio: {}. Sus nombres se han normalizado al inglés; los valores no enumerados se ofrecen como texto libre.",
                    added.join(", ")
                ));
            }
        }

        // Sondeo genérico de APIs REST: los bundles declaran la base y las rutas,
        // pero AEXT sólo guarda lo que responde JSON real con un GET público.
        result.api_bases = js_api_candidates.bases.clone();
        if !js_api_candidates.bases.is_empty() && !js_api_candidates.paths.is_empty() {
            let site_host = Url::parse(&result.entry_url)
                .ok()
                .and_then(|u| u.host_str().map(|s| s.to_string()))
                .unwrap_or_default();
            result.api_lists = self
                .probe_api_lists(&js_api_candidates, &site_host)
                .await;
            if !result.api_lists.is_empty() {
                let verified = result
                    .api_lists
                    .iter()
                    .map(|api| format!("{} ({})", api.resource, api.record_count))
                    .collect::<Vec<_>>()
                    .join(", ");
                result.warnings.push(format!(
                    "Endpoints REST verificados con JSON real: {}. Sus datos se usan para filtros y catálogo; nunca se inventan valores.",
                    verified
                ));
            }
        }

        // El sondeo del detalle se hace aquí, fuera de `analyze_page`, porque
        // `scraper::Html` no es `Send` y no puede sobrevivir a un `await`.
        // Si el usuario analizó directamente una ficha, la propia URL ya trae
        // el contentId; si analizó la portada, se añade la tarjeta de muestra.
        let mut sample_hints: Vec<String> = vec![result.entry_url.clone()];
        if let Some(path) = &demo_href {
            sample_hints.push(path.clone());
            let probe = self
                .probe_detail(&format!("{}{}", result.url, path))
                .await;
            if let Some(selector) = &probe.episode_selector {
                result.episode_selector = selector.clone();
            }
            if !probe.html.is_empty() {
                let detail_caps = Self::detect_capabilities(&probe.html);
                Self::merge_capabilities(&mut result.capabilities, &detail_caps);
                if let Some(hint) = Self::content_id_from_hint(&probe.html) {
                    sample_hints.push(hint.1);
                }
            }
            if !probe.fetched {
                result.warnings.push(format!(
                    "No se pudo abrir una página de detalle de ejemplo ({}) para comprobar episodios.",
                    probe.url
                ));
            } else if probe.episode_links == 0 {
                result.warnings.push(format!(
                    "La página de detalle de ejemplo ({}) no contiene enlaces de episodios: la ficha y la lista de episodios generadas son genéricas y necesitarán ajustes manuales.",
                    probe.url
                ));
            } else if !probe.has_embed {
                result.warnings.push(format!(
                    "La página de detalle de ejemplo tiene {} enlaces de episodios, pero el HTML no incluye ningún reproductor (.m3u8/iframe): los servidores pueden cargarse por JavaScript y la reproducción no está verificada.",
                    probe.episode_links
                ));
            }
        }

        // Verificación real contra un máster de muestra. Sin una playlist HLS
        // abierta y leída, AEXT NO ofrecerá el apartado SUB/DUB en la extensión.
        let master = self.probe_master_tracks(&result, &sample_hints).await;
        if master.attempted {
            if master.verified {
                result.capabilities.master_verified = true;
                result.capabilities.audio_languages = master.audio_languages.clone();
                result.capabilities.subtitle_languages = master.subtitle_languages.clone();
                result.capabilities.sub_verified = !master.subtitle_languages.is_empty()
                    || Self::label_mentions(&master.audio_languages, &["sub", "legend"]);
                result.capabilities.dub_verified =
                    Self::label_mentions(&master.audio_languages, &["dub", "dobl", "latino", "cast"]);
                if !result.capabilities.sub_verified && !result.capabilities.dub_verified {
                    let tracks = if master.audio_languages.is_empty() {
                        "sin pistas de audio declaradas".to_string()
                    } else {
                        format!("solo {}", master.audio_languages.join(", "))
                    };
                    result.warnings.push(format!(
                        "Máster HLS verificado ({}): {}, sin subtítulos ni doblaje confirmados. La extensión no ofrecerá el apartado SUB/DUB.",
                        master.detail, tracks
                    ));
                }
            } else {
                result.warnings.push(format!(
                    "No se pudo verificar un máster HLS de ejemplo ({}). La extensión no afirmará pistas SUB/DUB ni calidades reales.",
                    master.detail
                ));
            }
        }
        result.capabilities.playback_methods = Self::playback_methods(&result.capabilities);

        // Avisos finales sobre capacidades, ya con la ficha de ejemplo mezclada.
        if result.capabilities.stream_formats.is_empty() {
            result.capabilities.unverified.push(
                "No se detectó ningún formato de stream (HLS/DASH/MP4/iframe): la reproducción no está verificada.".to_string(),
            );
        }
        if result.capabilities.qualities.is_empty() {
            result.capabilities.unverified.push(
                "No se detectaron calidades reales: el reproductor decidirá automáticamente.".to_string(),
            );
        }
        let capability_notes = result.capabilities.unverified.clone();
        for note in &capability_notes {
            result.warnings.push(note.clone());
        }
        if result.capabilities.player_cases.iter().any(|c| c == "drm-widevine") {
            result.warnings.push(
                "Se detectó DRM (Widevine/PlayReady/FairPlay): el reproductor de Nekoyomi no puede descifrarlo. Se priorizarán variantes HLS sin DRM; si no existen, la reproducción fallará.".to_string(),
            );
        }
        if result.capabilities.player_cases.iter().any(|c| c == "cipher-signature") {
            result.warnings.push(
                "Se detectó cifrado/ofuscación de URLs de reproducción (firmas tipo Innertube / n-parameter). AEXT no resuelve esos esquemas: solo usa HLS/MP4/iframe detectados directamente; este sitio puede no reproducir.".to_string(),
            );
        }
        if result.capabilities.player_cases.iter().any(|c| c == "extension-proxy") {
            result.warnings.push(
                "El sitio parece exigir un proxy de extensión/CORS. Los módulos generados hacen peticiones directas: AEXT no actúa como proxy ni reenvía tráfico de terceros.".to_string(),
            );
        }

        Ok(result)
    }

    /// Analiza el HTML de la portada. Devuelve además la ruta de una ficha de
    /// ejemplo (si hay catálogo) para sondearla después.
    async fn analyze_page(&self, url: &str) -> Result<(AnalysisResult, Option<String>), String> {
        let parsed = Url::parse(url.trim()).map_err(|e| format!("URL inválida '{}': {}", url, e))?;
        if parsed.scheme() != "http" && parsed.scheme() != "https" {
            return Err(format!("Solo se admiten URLs http/https: {}", url));
        }

        // El `baseUrl` generado debe ser SIEMPRE el origen. Usar una ruta de la
        // portada (o inventar `/animes`, `/recientes`) provoca redirecciones a
        // páginas que no son catálogo, como ocurre con pluto.tv.
        let origin = parsed.origin().ascii_serialization();
        if origin == "null" || origin.is_empty() {
            return Err(format!("No se pudo determinar el origen de {}", url));
        }
        let host = parsed.host_str().unwrap_or("").to_string();
        let entry_url = parsed.to_string();

        let resp = self.client.get(&entry_url)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .send()
            .await
            .map_err(|e| format!("Error conectando con {}: {}", url, e))?;

        let _status = resp.status();
        let headers = resp.headers().clone();
        let final_url = resp.url().clone();
        let body = resp.text().await.map_err(|e| format!("Error leyendo HTML: {}", e))?;

        // Las SPAs suelen declarar catálogo/SearchAction sólo en la portada.
        // Si el usuario pegó una ruta interna, se pide la raíz ANTES de crear
        // cualquier `Html`: ese tipo no es Send y no puede vivir sobre un await.
        let root_body = if parsed.path() != "/" {
            match self.client.get(format!("{origin}/")).send().await {
                Ok(r) if r.status().is_success() => r.text().await.ok(),
                _ => None,
            }
        } else {
            None
        };

        // A partir de aquí no hay llamadas asíncronas (.await), por lo que Html nunca cruza un punto de suspensión.
        let doc = Html::parse_document(&body);

        // 1. Detección de Nombre y Título
        let name = self.detect_site_name(&doc, &origin);

        // 2. Detección de Favicon
        let favicon_url = self.detect_favicon(&doc, &origin);

        // 3. Detección de Protecciones
        let mut protections = Vec::new();
        if body.contains("challenges.cloudflare.com/turnstile") || body.contains("cf-turnstile") {
            protections.push("Cloudflare Turnstile".to_string());
        }
        if body.contains("anubis") || headers.get("X-Anubis-Challenge").is_some() {
            protections.push("Anubis PoW Challenge".to_string());
        }
        if headers.get("server").and_then(|s| s.to_str().ok()).map(|s| s.contains("cloudflare")).unwrap_or(false) {
            protections.push("Cloudflare WAF".to_string());
        }

        // 4. Detección de CMS, Tipo y catálogo real
        let (cms, ext_type) = self.detect_cms_and_type(&body);
        let catalog = self.detect_catalog(&doc, &origin, &host);

        // 5. Rutas reales de listado, recientes y búsqueda
        let (popular_path, latest_path) = self.detect_section_paths(&doc, &origin, &host, &catalog);
        let (mut search_path, mut search_param) = self.detect_search(&doc, &origin, &host, &cms);

        // 5.5 Evidencia real de catálogo/búsqueda/filtros en el HTML. Nada de
        // esto se inventa: si el sitio no lo declara, el generador lo dice.
        let mut site_locales = Self::detect_site_locales(&doc);
        let mut detected_language = Self::detect_language(&doc);
        if detected_language.is_empty() {
            detected_language = site_locales.first().cloned().unwrap_or_default();
        }
        let (mut ld_records, mut ld_search) = Self::detect_structured_data(&doc, &origin, &host);
        let mut filter_params = Self::detect_filter_params(&doc, &origin, &host, search_param.as_deref());
        for f in Self::detect_payload_filters(&doc, search_param.as_deref()) {
            if !filter_params.iter().any(|old| old.param == f.param && old.value == f.value) {
                filter_params.push(f);
            }
        }

        // Muchas SPAs declaran el catálogo y el SearchAction sólo en la
        // portada. Si el usuario pegó una ficha/ruta interna, se consulta la
        // raíz del mismo sitio antes de declarar que no hay catálogo.
        if ld_records.is_empty() && catalog.count < 4 {
            if let Some(root_html) = root_body.as_deref() {
                let root_doc = Html::parse_document(root_html);
                let (root_records, root_action) =
                    Self::detect_structured_data(&root_doc, &origin, &host);
                if !root_records.is_empty() {
                    ld_records = root_records;
                    ld_search = root_action;
                    for f in Self::detect_filter_params(&root_doc, &origin, &host, None) {
                        if !filter_params.iter().any(|old| old.param == f.param && old.value == f.value) {
                            filter_params.push(f);
                        }
                    }
                    if detected_language.is_empty() {
                        detected_language = Self::detect_language(&root_doc);
                    }
                    for locale in Self::detect_site_locales(&root_doc) {
                        if !site_locales.contains(&locale) {
                            site_locales.push(locale);
                        }
                    }
                    if detected_language.is_empty() {
                        detected_language = site_locales.first().cloned().unwrap_or_default();
                    }
                }
            }
        }
        if detected_language.is_empty() {
            detected_language = site_locales.first().cloned().unwrap_or_default();
        }

        if let Some(action) = &ld_search {
            if let Some((path, param)) = Self::search_template_to_path(&action.url_template, &origin, &host) {
                // La acción declarada por schema.org es más fiable que un input
                // suelto: gana siempre que apunte al mismo origen.
                search_path = Some(path);
                search_param = Some(param);
            }
            if let Some(param) = &action.query_param {
                filter_params.retain(|f| f.param != *param);
            }
        }

        // 6. Servidores de video y SUB/DUB (síncrona)
        let (detected_hosts, has_sub_dub) = self.detect_servers_and_dub(&body, &origin);

        // 7. Selectores: los del CMS solo si encajan; si no, los detectados.
        let (cms_card, cms_title, cms_next) = self.detect_selectors(&cms);
        let card_selector = if catalog.count >= 4 {
            catalog.card_selector.clone()
        } else {
            cms_card
        };
        let title_selector = if catalog.count >= 4 {
            catalog.title_selector.clone()
        } else {
            cms_title
        };
        let next_page_selector = if !catalog.next_page_selector.is_empty() {
            catalog.next_page_selector.clone()
        } else if catalog.page_style == "none" {
            String::new()
        } else {
            cms_next
        };

        let catalog_detected = catalog.count >= 4;
        // Un ItemList real con más títulos que las tarjetas HTML encontradas
        // gana: en SPAs el HTML suele contener sólo navegación y banners.
        let prefer_jsonld = ld_records.len() >= 4 && catalog.count < ld_records.len();
        let catalog_source = if prefer_jsonld {
            "jsonld".to_string()
        } else if catalog_detected {
            "html".to_string()
        } else if !ld_records.is_empty() {
            "jsonld".to_string()
        } else {
            "none".to_string()
        };
        let page_style = if catalog.page_style != "none" {
            catalog.page_style.clone()
        } else if matches!(cms.as_str(), "dooplay" | "madara" | "mangathemesia") {
            "path".to_string()
        } else {
            "none".to_string()
        };

        // 7.5 Ficha de ejemplo: el generador usa selectores genéricos para el
        //     detalle, así que se guarda una URL para sondearla en `analyze`.
        let demo_href = if catalog_detected {
            Self::first_card_href(&doc, &card_selector, &origin, &host)
        } else {
            None
        };

        // 8. Avisos honestos: mejor advertir que generar rutas falsas.
        let mut warnings = Vec::new();
        let looks_spa = body.contains("__NEXT_DATA__")
            || body.contains("__NUXT__")
            || body.contains("data-reactroot")
            || body.contains("id=\"app\"")
            || body.contains("id='app'");
        if prefer_jsonld {
            warnings.push(format!(
                "Se usarán los {} títulos reales del ItemList (JSON-LD) en lugar de {} elementos HTML de navegación/header/footer.",
                ld_records.len(),
                catalog.count
            ));
        } else if !catalog_detected && catalog_source == "jsonld" {
            warnings.push(format!(
                "El HTML estático no expone tarjetas; se usarán {} títulos reales declarados en JSON-LD (schema.org ItemList).",
                ld_records.len()
            ));
        } else if !catalog_detected {
            if looks_spa {
                warnings.push(
                    "La portada parece una SPA (React/Next/Nuxt): el HTML estático no expone el catálogo y no hay datos estructurados. El listado puede quedar vacío.".to_string(),
                );
            } else {
                warnings.push(
                    "No se detectaron tarjetas de catálogo en la portada; los selectores pueden ser incorrectos.".to_string(),
                );
            }
        }
        if popular_path.is_none() {
            warnings.push(
                "No se detectó una ruta de catálogo/populares: se usará la URL de entrada tal cual.".to_string(),
            );
        } else if !catalog_detected {
            warnings.push(format!(
                "La ruta de listado {} se tomó de un enlace del menú, pero no se pudo verificar que sea un catálogo de tarjetas.",
                popular_path.as_deref().unwrap_or("/")
            ));
        }
        if latest_path.is_none() {
            warnings.push(
                "No se detectó una ruta de 'recientes'; la pestaña de últimos quedará desactivada.".to_string(),
            );
        }
        if search_path.is_none() || search_param.is_none() {
            warnings.push(
                "No se detectó búsqueda ni en formularios ni en datos estructurados: las consultas se filtrarán sobre el catálogo local.".to_string(),
            );
        } else if ld_search.is_some() {
            warnings.push(
                "La URL de búsqueda se tomó de un SearchAction declarado por el sitio (JSON-LD).".to_string(),
            );
        }
        if !filter_params.is_empty() {
            warnings.push(format!(
                "Filtros encontrados en enlaces/formularios/datos del sitio: {}. Sólo se generarán esos, no géneros inventados.",
                filter_params.iter().map(|f| f.param.as_str()).collect::<HashSet<_>>().into_iter().collect::<Vec<_>>().join(", ")
            ));
        }
        if site_locales.len() > 1 {
            warnings.push(format!(
                "El sitio declara varios idiomas ({}): el paquete se marcará como 'all' y los textos de la extensión serán ingleses. No se elige un idioma por query param.",
                site_locales.join(", ")
            ));
        }
        if !protections.is_empty() {
            warnings.push(format!(
                "Protección detectada ({}). El cliente usará Cloudflare/WebView cuando haga falta.",
                protections.join(", ")
            ));
        }
        if final_url.path() != parsed.path() && !final_url.path().is_empty() {
            warnings.push(format!(
                "La URL de entrada redirigió a {}. Comprueba que sea el catálogo correcto.",
                final_url
            ));
        }

        // Capacidades según lo que este HTML expone realmente. La ficha de
        // ejemplo puede añadir más en `analyze()`.
        let capabilities = Self::detect_capabilities(&body);
        let sample_items = if prefer_jsonld {
            ld_records.iter().take(6).map(|r| r.title.clone()).collect()
        } else {
            catalog.samples
        };

        Ok((
            AnalysisResult {
                url: origin,
                entry_url,
                name,
                lang: "es".to_string(),
                ext_type,
                cms,
                favicon_url,
                detected_hosts,
                has_sub_dub,
                protections,
                card_selector,
                title_selector,
                next_page_selector,
                catalog_source,
                catalog_records: ld_records,
                search_action: ld_search,
                filter_params,
                api_bases: Vec::new(),
                api_lists: Vec::new(),
                detected_language,
                site_locales,
                popular_path,
                latest_path,
                search_path,
                search_param,
                page_style,
                catalog_detected,
                episode_selector: String::new(),
                sample_items,
                warnings,
                capabilities,
            },
            demo_href,
        ))
    }

    /// Primer enlace interno de una tarjeta del catálogo. Se usa solo como
    /// muestra para sondeos; nunca se escribe en la extensión.
    fn first_card_href(doc: &Html, card_selector: &str, origin: &str, host: &str) -> Option<String> {
        let sel = Selector::parse(card_selector).ok()?;
        let anchor_sel = Selector::parse("a[href]").ok()?;
        for card in doc.select(&sel) {
            let anchor = if card.value().name() == "a" {
                Some(card)
            } else {
                card.select(&anchor_sel).next()
            };
            let Some(anchor) = anchor else { continue };
            let href = anchor.value().attr("href").unwrap_or("");
            if let Some(path) = Self::same_origin_path(href, origin, host) {
                if path != "/" {
                    return Some(path);
                }
            }
        }
        None
    }

    /// Extrae scripts del mismo origen. Los externos se descartan: no podremos
    /// distinguir un endpoint del sitio del de un proveedor de analítica.
    fn script_urls(body: &str, origin: &str, limit: usize) -> Vec<String> {
        let doc = Html::parse_document(body);
        let selector = match Selector::parse("script[src]") {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let base = Url::parse(origin).ok();
        let mut out: Vec<String> = Vec::new();
        for el in doc.select(&selector) {
            let src = el.value().attr("src").unwrap_or("").trim();
            if src.is_empty() {
                continue;
            }
            let absolute = Url::parse(src)
                .ok()
                .or_else(|| base.as_ref().and_then(|b| b.join(src).ok()));
            let Some(url) = absolute else { continue };
            if url.origin().ascii_serialization() != origin {
                continue;
            }
            let value = url.to_string();
            if !out.contains(&value) {
                out.push(value);
            }
        }
        // En Next.js el código del reproductor vive en `pages/_app`, que suele
        // ir AFTER de varios chunks de analytics. Priorizarlo evita analizar
        // sólo librerías de terceros y no ver ningún método de reproducción.
        out.sort_by_key(|url| {
            let lower = url.to_lowercase();
            if lower.contains("pages/_app") || lower.contains("/_app-") {
                0
            } else if lower.contains("/chunks/main") {
                1
            } else if lower.contains("/chunks/webpack") {
                2
            } else if lower.contains("/static/chunks/") {
                3
            } else {
                4
            }
        });
        out.truncate(limit);
        out
    }

    /// Lee unos pocos bundles propios y detecta capacidades y parámetros de
    /// filtro en su texto. Se analiza cada bundle completo (no un recorte):
    /// los hashes y el token de sesión pueden estar al final de un bundle de
    /// 2 MB. Los filtros encontrados son reales (los declara el propio sitio),
    /// pero nunca se inventan valores: si no se pueden enumerar se ofrecen
    /// como campo de texto libre.
    async fn scan_scripts(
        &self,
        entry_url: &str,
        search_param: Option<&str>,
    ) -> Option<(SiteCapabilities, Vec<SiteFilterParam>, ApiCandidates)> {
        let html = self.client.get(entry_url).send().await.ok()?.text().await.ok()?;
        let origin = Url::parse(entry_url).ok()?.origin().ascii_serialization();
        let urls = Self::script_urls(&html, &origin, 8);
        if urls.is_empty() {
            return None;
        }

        let mut aggregate = SiteCapabilities::default();
        let mut filters: Vec<SiteFilterParam> = Vec::new();
        let mut api_candidates = ApiCandidates::default();
        let mut found_any = false;
        for url in urls {
            if let Ok(resp) = self.client.get(&url).send().await {
                if let Ok(text) = resp.text().await {
                    if text.is_empty() {
                        continue;
                    }
                    found_any = true;
                    let caps = Self::detect_capabilities(&text);
                    Self::merge_capabilities(&mut aggregate, &caps);
                    for filter in Self::detect_filters_from_js(&text, search_param) {
                        let duplicate = filters.iter().any(|old| {
                            old.param == filter.param
                                && (old.value == filter.value || filter.kind == "text")
                        });
                        if !duplicate {
                            filters.push(filter);
                        }
                    }
                    let candidates = Self::detect_api_candidates(&text);
                    for base in candidates.bases {
                        if !api_candidates.bases.contains(&base) {
                            api_candidates.bases.push(base);
                        }
                    }
                    for path in candidates.paths {
                        if !api_candidates.paths.contains(&path) {
                            api_candidates.paths.push(path);
                        }
                    }
                    if api_candidates.version.is_none() {
                        api_candidates.version = candidates.version;
                    }
                    let has_session = aggregate.player_cases.iter().any(|c| c == "session-bootstrap");
                    let has_graphql = aggregate.player_cases.iter().any(|c| c == "graphql-persisted");
                    if has_session && has_graphql {
                        break;
                    }
                }
            }
        }
        if !found_any {
            return None;
        }
        Some((aggregate, filters, api_candidates))
    }

    /// Extrae un contentId de muestra de una ruta o de un estado hidratado.
    fn content_id_from_hint(hint: &str) -> Option<(String, String)> {
        let flat = hint.replace("\\\"", "\"");
        if let Ok(re) = Regex::new(r"(?i)(episodes?|movies?)/([0-9a-f]{24})") {
            if let Some(cap) = re.captures(&flat) {
                let raw = cap[1].to_lowercase();
                let kind = if raw.starts_with("movie") { "movie" } else { "episode" };
                return Some((kind.to_string(), cap[2].to_string()));
            }
        }
        let re = Regex::new(r#"(?i)contentId\s*[:=]\s*["']([0-9a-f]{24})["']"#).ok()?;
        let id = re.captures(&flat)?[1].to_string();
        let kind = if flat.to_lowercase().contains("/movie") { "movie" } else { "episode" };
        Some((kind.to_string(), id))
    }

    /// Identificador con forma de UUID para el sondeo. Pluto no valida que
    /// corresponda a una sesión de navegador real, sólo que tenga ese formato.
    fn probe_client_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let pid = std::process::id() as u64;
        let tail = (nanos ^ pid.wrapping_mul(0x9e37_79b9_7f4a_7c15)) & 0x0fff_ffff_ffff;
        format!(
            "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
            (nanos ^ (pid << 32)) as u32,
            ((nanos >> 32) & 0xffff) as u16,
            ((nanos >> 12) & 0x0fff) as u16,
            ((nanos >> 40) & 0x0fff) as u16,
            tail
        )
    }

    /// Comprueba de verdad un máster HLS de muestra. Es la única fuente de las
    /// banderas `master_verified`/`sub_verified`/`dub_verified`: un texto
    /// "SUB"/"DUB" en el DOM no cuenta como pista de audio o subtítulo.
    async fn probe_master_tracks(
        &self,
        result: &AnalysisResult,
        hints: &[String],
    ) -> MasterTrackProbe {
        let mut out = MasterTrackProbe::default();
        let uses_session = result
            .capabilities
            .player_cases
            .iter()
            .any(|c| c == "session-bootstrap")
            || result.capabilities.api_endpoints.iter().any(|e| {
                e.contains("/v4/start") || e.contains("boot.") || e.contains("/stitch/")
            });
        if !uses_session {
            return out;
        }
        let Some((kind, id)) = hints.iter().find_map(|h| Self::content_id_from_hint(h)) else {
            out.detail = "no se encontró un contenido de muestra".to_string();
            return out;
        };
        out.attempted = true;

        let app_version = result
            .capabilities
            .app_version
            .as_deref()
            .filter(|v| !v.is_empty())
            .unwrap_or("10.12.1096");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let boot_url = format!(
            "https://boot.pluto.tv/v4/start?appName=web&appVersion={}&deviceVersion=124.0.0.0&deviceModel=web&deviceMake=chrome&deviceType=web&clientID={}&clientModelNumber=1.0.0&serverSideAds=false&clientTime={}",
            app_version,
            Self::probe_client_id(),
            now
        );
        let Ok(boot) = self
            .client
            .get(&boot_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .header("Referer", "https://pluto.tv/")
            .header("Accept", "application/json")
            .send()
            .await
        else {
            out.detail = "boot/start no respondió".to_string();
            return out;
        };
        if !boot.status().is_success() {
            out.detail = format!("boot/start HTTP {}", boot.status());
            return out;
        }
        let Ok(boot_body) = boot.text().await else {
            out.detail = "boot/start sin cuerpo".to_string();
            return out;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&boot_body) else {
            out.detail = "boot/start no devolvió JSON".to_string();
            return out;
        };
        let token = json.get("sessionToken").and_then(|v| v.as_str()).unwrap_or("");
        let params = json.get("stitcherParams").and_then(|v| v.as_str()).unwrap_or("");
        let stitcher = json
            .get("stitcher")
            .and_then(|v| v.as_str())
            .unwrap_or("https://service-stitcher.clusters.pluto.tv");
        if token.is_empty() || params.is_empty() {
            out.detail = "boot/start sin sesión utilizable".to_string();
            return out;
        }

        let segment = if kind == "movie" { "movie" } else { "episode" };
        let master_url = format!(
            "{}/v2/stitch/hls/{}/{}/master.m3u8?{}&jwt={}&includeExtendedEvents=true&masterJWTPassthrough=true",
            stitcher.trim_end_matches('/'), segment, id, params, token
        );
        let Ok(master) = self
            .client
            .get(&master_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .header("Referer", "https://pluto.tv/")
            .header("Accept", "*/*")
            .send()
            .await
        else {
            out.detail = "máster HLS no respondió".to_string();
            return out;
        };
        let status = master.status();
        let Ok(body) = master.text().await else {
            out.detail = "máster HLS sin cuerpo".to_string();
            return out;
        };
        if !status.is_success() {
            out.detail = format!("máster HLS HTTP {}", status);
            if status.as_u16() == 403 {
                out.detail.push_str(" (token o región rechazados)");
            }
            return out;
        }
        if !body.contains("#EXTM3U")
            || !(body.contains("#EXT-X-STREAM-INF") || body.contains("#EXTINF"))
        {
            out.detail = format!("respuesta sin playlist ({} bytes)", body.len());
            return out;
        }

        out.verified = true;
        out.detail = format!("máster HLS ({} bytes)", body.len());
        if let Ok(re) = Regex::new(r#"#EXT-X-MEDIA:([^\r\n]+)"#) {
            for cap in re.captures_iter(&body) {
                let line = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                let attr = |name: &str| -> String {
                    Regex::new(&format!(r#"{}="([^"]*)""#, name))
                        .ok()
                        .and_then(|r| r.captures(line))
                        .map(|c| c[1].trim().to_string())
                        .unwrap_or_default()
                };
                let kind = attr("TYPE").to_uppercase();
                let lang = attr("LANGUAGE");
                let name = attr("NAME");
                let label = match (name.is_empty(), lang.is_empty() || lang == "und") {
                    (true, true) => String::new(),
                    (true, false) => lang,
                    (false, true) => name,
                    (false, false) => format!("{} ({})", name, lang),
                };
                if label.is_empty() {
                    continue;
                }
                let list = if kind == "AUDIO" {
                    &mut out.audio_languages
                } else if kind == "SUBTITLES" {
                    &mut out.subtitle_languages
                } else {
                    continue;
                };
                if !list.contains(&label) {
                    list.push(label);
                }
            }
        }
        out
    }

    fn label_mentions(labels: &[String], needles: &[&str]) -> bool {
        labels.iter().any(|label| {
            let lower = label.to_lowercase();
            needles.iter().any(|needle| lower.contains(needle))
        })
    }

    /// Nombres legibles de los métodos encontrados. Se muestran tal cual: si
    /// no hay evidencia, se dice expresamente que no hay método confirmado.
    pub fn playback_methods(caps: &SiteCapabilities) -> Vec<String> {
        let mut out = Vec::new();
        let cases = &caps.player_cases;
        if cases.iter().any(|c| c == "session-bootstrap") {
            out.push("Sesión web".to_string());
        }
        if cases.iter().any(|c| c == "graphql-persisted") {
            out.push("GraphQL interno".to_string());
        }
        if cases.iter().any(|c| c == "hls-master") {
            out.push("HLS".to_string());
        }
        if cases.iter().any(|c| c == "dash-mpd") {
            out.push("DASH".to_string());
        }
        if cases.iter().any(|c| c == "iframe-player")
            || caps.stream_formats.iter().any(|f| f == "iframe")
        {
            out.push("Reproductor incrustado".to_string());
        }
        if caps.stream_formats.iter().any(|f| f == "mp4") {
            out.push("MP4 directo".to_string());
        }
        if out.is_empty() {
            out.push("Sin método confirmado".to_string());
        }
        out
    }

    /// Detecta capacidades reales a partir del HTML disponible. Es
    /// determinista y conservador: sólo afirma lo que aparece en el cuerpo.
    fn detect_capabilities(body: &str) -> SiteCapabilities {
        let mut caps = SiteCapabilities::default();
        let lower = body.to_lowercase();

        fn push_unique(list: &mut Vec<String>, value: String) {
            if !value.is_empty() && !list.contains(&value) && list.len() < 12 {
                list.push(value);
            }
        }

        if lower.contains(".m3u8") || lower.contains("application/vnd.apple.mpegurl") || lower.contains("#extm3u") {
            caps.stream_formats.push("hls".to_string());
        }
        if lower.contains(".mpd") || lower.contains("application/dash+xml") || lower.contains("dash.js") {
            caps.stream_formats.push("dash".to_string());
        }
        if lower.contains(".mp4") {
            caps.stream_formats.push("mp4".to_string());
        }
        if lower.contains("<iframe") {
            caps.stream_formats.push("iframe".to_string());
        }

        // Caso del reproductor. Clasifica el sitio para que el generador pueda
        // ordenar las estrategias antes de tener aciertos/fallos reales, y para
        // avisar cuando el stream exige algo que Nekoyomi no puede hacer.
        let mut cases: Vec<String> = Vec::new();
        if lower.contains("stitcherparams") || lower.contains("sessiontoken")
            || (lower.contains("boot.") && lower.contains("/v4/start"))
        {
            push_unique(&mut cases, "session-bootstrap".to_string());
        }
        if (lower.contains("graphql") || lower.contains("apollo"))
            && (lower.contains("persisteddocumenthash") || lower.contains("tnpersisteddocumenthash"))
        {
            push_unique(&mut cases, "graphql-persisted".to_string());
        }
        if caps.stream_formats.iter().any(|f| f == "hls") {
            push_unique(&mut cases, "hls-master".to_string());
        }
        if caps.stream_formats.iter().any(|f| f == "dash") {
            push_unique(&mut cases, "dash-mpd".to_string());
        }
        if caps.stream_formats.iter().any(|f| f == "iframe") {
            push_unique(&mut cases, "iframe-player".to_string());
        }
        if lower.contains("widevine") || lower.contains("playready") || lower.contains("fairplay") {
            push_unique(&mut cases, "drm-widevine".to_string());
        }
        // Detección honesta de esquemas que AEXT NO resuelve. No generamos
        // código para ellos: solo los etiquetamos para avisar al usuario.
        if lower.contains("signaturecipher") || lower.contains("innertube")
            || lower.contains("player_response") || lower.contains("playerresponse")
            || lower.contains("descramble") || lower.contains("descipher")
            || (lower.contains("nsig") && lower.contains("player"))
        {
            push_unique(&mut cases, "cipher-signature".to_string());
        }
        if lower.contains("extension-proxy") || lower.contains("corsproxy")
            || lower.contains("cors-proxy") || lower.contains("proxy.php")
            || lower.contains("/proxy?")
        {
            push_unique(&mut cases, "extension-proxy".to_string());
        }
        if lower.contains("_ad/") || lower.contains("_ad%2f")
            || lower.contains("plutotv_filler") || lower.contains("dai.google.com")
        {
            push_unique(&mut cases, "ads-hls".to_string());
        }
        caps.player_cases = cases;

        // Calidades e idiomas de un máster HLS embebido en el propio HTML.
        if let Ok(re) = Regex::new(r#"RESOLUTION\s*=\s*\d+x(\d+)"#) {
            for cap in re.captures_iter(body) {
                if let Some(height) = cap.get(1) {
                    push_unique(&mut caps.qualities, format!("{}p", height.as_str()));
                }
            }
        }
        if let Ok(re) = Regex::new(r#"#EXT-X-MEDIA:([^\r\n]+)"#) {
            for cap in re.captures_iter(body) {
                let mut attrs = HashMap::new();
                // Los atributos HLS pueden ir con comillas (NAME="English") o
                // sin ellas (TYPE=AUDIO). Aceptar ambos es necesario para no
                // perder las pistas reales de un máster embebido en el HTML.
                if let Ok(pairs) = Regex::new(r#"([A-Za-z0-9-]+)\s*=\s*(?:"([^"]*)"|([^,\r\n]*))"#) {
                    for pair in pairs.captures_iter(cap.get(1).map(|m| m.as_str()).unwrap_or("")) {
                        let value = pair
                            .get(2)
                            .or_else(|| pair.get(3))
                            .map(|m| m.as_str().trim().to_string())
                            .unwrap_or_default();
                        attrs.insert(pair[1].to_uppercase(), value);
                    }
                }
                let kind = attrs.get("TYPE").map(|s| s.as_str()).unwrap_or("");
                let lang = attrs.get("LANGUAGE").cloned().unwrap_or_default();
                let name = attrs.get("NAME").cloned().unwrap_or_else(|| lang.clone());
                let label = if !lang.is_empty() && lang != "und" {
                    format!("{} ({})", name, lang)
                } else {
                    name
                };
                if kind.eq_ignore_ascii_case("AUDIO") {
                    push_unique(&mut caps.audio_languages, label);
                } else if kind.eq_ignore_ascii_case("SUBTITLES") {
                    push_unique(&mut caps.subtitle_languages, label);
                }
            }
        }

        // Endpoints API/GraphQL citados en HTML o JS embebido.
        if let Ok(re) = Regex::new(r#"https?://[^"'\s<>\\]+"#) {
            for cap in re.captures_iter(body) {
                let url = cap[0].trim_end_matches(|c: char| matches!(c, ',' | ';' | ')' | '}' | ']' | '`'));
                let lower_url = url.to_lowercase();
                // Los arranques de sesión de reproductores web suelen vivir en
                // `/v4/start` o en un host `boot.`, sin `/api/` en la ruta.
                if (lower_url.contains("/api/")
                    || lower_url.contains("/graphql")
                    || lower_url.contains("api.")
                    || lower_url.contains("/v4/start")
                    || lower_url.contains("/stitch/")
                    || lower_url.contains("boot."))
                    && url.len() < 200
                {
                    push_unique(&mut caps.api_endpoints, url.to_string());
                }
            }
        }

        // Versión declarada por el propio sitio, en meta tag o en JSON embebido.
        if caps.app_version.is_none() {
            for pattern in [
                r#"(?i)(?:globalAppVersion|appVersion|app_version)\s*[:=]\s*["']([0-9][0-9A-Za-z.\-]+)["']"#,
                r#"(?i)name=["'](?:appVersion|app_version|globalAppVersion)["'][^>]*content=["']([0-9][0-9A-Za-z.\-]+)["']"#,
            ] {
                if let Ok(re) = Regex::new(pattern) {
                    if let Some(cap) = re.captures(body) {
                        caps.app_version = Some(cap[1].to_string());
                        break;
                    }
                }
            }
        }

        // Los estados hidratados (Next.js/React Query) guardan la región real
        // resuelta por IP. Pluto la expone como
        // `"region":{"locale":"es-419","region":"MX"}`, a veces con comillas
        // escapadas dentro de un bloque RSC. Normalizamos esas comillas antes
        // de leerla para no depender del formato de serialización.
        let flat = body.replace("\\\"", "\"");
        if let Ok(re) = Regex::new(
            r#""region"\s*:\s*\{[^}]*?"locale"\s*:\s*"([^"]+)"[^}]*?"region"\s*:\s*"([^"]+)""#,
        ) {
            if let Some(cap) = re.captures(&flat) {
                caps.locale = Some(cap[1].to_string());
                caps.region = Some(cap[2].to_string());
            }
        }
        if caps.region.is_none() {
            if let Ok(re) = Regex::new(r#""region"\s*:\s*"([A-Za-z]{2})""#) {
                if let Some(cap) = re.captures(&flat) {
                    caps.region = Some(cap[1].to_uppercase());
                }
            }
        }
        if caps.locale.is_none() {
            if let Ok(re) = Regex::new(r#"translations-([a-z]{2}(?:-[A-Za-z0-9]+)?)"#) {
                if let Some(cap) = re.captures(&flat) {
                    caps.locale = Some(cap[1].to_string());
                }
            }
        }

        caps
    }

    /// Combina lo visto en la portada con lo visto en una ficha de ejemplo.
    fn merge_capabilities(base: &mut SiteCapabilities, extra: &SiteCapabilities) {
        fn merge(base: &mut Vec<String>, extra: &[String]) {
            for value in extra {
                if !base.contains(value) {
                    base.push(value.clone());
                }
            }
        }
        merge(&mut base.stream_formats, &extra.stream_formats);
        merge(&mut base.qualities, &extra.qualities);
        merge(&mut base.audio_languages, &extra.audio_languages);
        merge(&mut base.subtitle_languages, &extra.subtitle_languages);
        merge(&mut base.api_endpoints, &extra.api_endpoints);
        merge(&mut base.player_cases, &extra.player_cases);
        merge(&mut base.playback_methods, &extra.playback_methods);
        base.master_verified |= extra.master_verified;
        base.sub_verified |= extra.sub_verified;
        base.dub_verified |= extra.dub_verified;
        if base.app_version.is_none() {
            base.app_version = extra.app_version.clone();
        }
        if base.region.is_none() {
            base.region = extra.region.clone();
        }
        if base.locale.is_none() {
            base.locale = extra.locale.clone();
        }
    }

    /// Abre una ficha de ejemplo, cuenta enlaces de episodios y deduce un
    /// selector que sí los encuentre. No modifica el análisis: solo aporta
    /// evidencia para los avisos y para la lista de episodios generada.
    async fn probe_detail(&self, detail_url: &str) -> DetailProbe {
        let mut probe = DetailProbe {
            url: detail_url.to_string(),
            fetched: false,
            episode_links: 0,
            has_embed: false,
            episode_selector: None,
            html: String::new(),
        };

        let Ok(resp) = self.client.get(detail_url).send().await else {
            return probe;
        };
        if !resp.status().is_success() {
            return probe;
        }
        let Ok(body) = resp.text().await else {
            return probe;
        };
        probe.fetched = true;
        probe.has_embed = body.contains(".m3u8") || body.contains("<iframe");
        probe.html = body.clone();
        if let Some((selector, count)) = Self::detect_episode_selector(&body) {
            probe.episode_selector = Some(selector);
            probe.episode_links = count;
        }

        probe
    }

    /// Deduce un selector de episodios a partir de una ficha. Se agrupa por el
    /// segmento de ruta que delata un episodio; `watch` se queda fuera a
    /// propósito porque en Pluto TV es el menú de TV en directo.
    fn detect_episode_selector(body: &str) -> Option<(String, usize)> {
        let doc = Html::parse_document(body);
        let sel = Selector::parse("a[href]").ok()?;

        let hints = ["/episode/", "/episodio/", "/episodios/", "/capitulo/", "/capítulo/", "/ver/"];
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for anchor in doc.select(&sel) {
            let href = anchor.value().attr("href").unwrap_or("").to_lowercase();
            if !href.starts_with('/') && !href.starts_with("http") {
                continue;
            }
            for hint in hints {
                let Some(pos) = href.find(hint) else { continue };
                // Debe haber algo después del segmento (`/ver/` a secas es menú).
                if href[pos + hint.len()..].trim_matches('/').is_empty() {
                    continue;
                }
                *counts.entry(hint).or_default() += 1;
            }
        }

        let (hint, count) = counts
            .into_iter()
            .filter(|(_, count)| *count >= 2)
            .max_by_key(|(_, count)| *count)?;
        Some((format!("a[href*='{}']", hint), count))
    }

    fn same_origin_path(href: &str, origin: &str, host: &str) -> Option<String> {
        let h = href.trim();
        if h.is_empty()
            || h.starts_with('#')
            || h.starts_with("javascript:")
            || h.starts_with("mailto:")
            || h.starts_with("tel:")
            || h.starts_with("data:")
        {
            return None;
        }

        let u = if h.starts_with("//") {
            Url::parse(&format!("https:{}", h)).ok()?
        } else if h.starts_with('/') {
            Url::parse(&format!("{}{}", origin, h)).ok()?
        } else if h.starts_with("http://") || h.starts_with("https://") {
            Url::parse(h).ok()?
        } else {
            // Ruta relativa: se resuelve contra el origen.
            Url::parse(&format!("{}/{}", origin, h)).ok()?
        };

        if u.host_str().map(|d| !d.eq_ignore_ascii_case(host)).unwrap_or(true) {
            return None;
        }

        let mut path = u.path().to_string();
        if path.is_empty() {
            path.push('/');
        }
        if let Some(q) = u.query() {
            path.push('?');
            path.push_str(q);
        }
        Some(path)
    }

    fn is_internal_href(href: &str, host: &str) -> bool {
        let h = href.trim();
        if h.is_empty()
            || h.starts_with('#')
            || h.starts_with("javascript:")
            || h.starts_with("mailto:")
            || h.starts_with("tel:")
            || h.starts_with("data:")
        {
            return false;
        }
        if h.starts_with("//") {
            return h[2..]
                .split('/')
                .next()
                .map(|d| d.eq_ignore_ascii_case(host))
                .unwrap_or(false);
        }
        if h.starts_with("http://") || h.starts_with("https://") {
            return Url::parse(h)
                .ok()
                .and_then(|u| u.host_str().map(|d| d.eq_ignore_ascii_case(host)))
                .unwrap_or(false);
        }
        true
    }

    fn matches_any(hay: &str, keys: &[&str]) -> bool {
        keys.iter().any(|k| {
            if k.chars().count() <= 4 && !k.contains(' ') {
                hay.split(|c: char| !c.is_alphanumeric()).any(|t| t == *k)
            } else {
                hay.contains(k)
            }
        })
    }

    fn card_title(card: ElementRef<'_>, host: &str) -> Option<(String, String)> {
        let link_sel = Selector::parse("a[href]").ok()?;
        let link = card.select(&link_sel).find(|a| {
            Self::is_internal_href(a.value().attr("href").unwrap_or(""), host)
        })?;
        let href = link.value().attr("href")?.to_string();

        let title_sel = Selector::parse(
            "h1, h2, h3, h4, h5, .title, [class*=title], [class*=name], [itemprop=name]",
        )
        .ok()?;
        let title = card
            .select(&title_sel)
            .map(|e| e.text().collect::<String>().trim().to_string())
            .find(|t| !t.is_empty())
            .or_else(|| link.value().attr("title").map(|s| s.trim().to_string()))
            .or_else(|| {
                let img_sel = Selector::parse("img[alt]").ok()?;
                card.select(&img_sel)
                    .next()
                    .map(|i| i.value().attr("alt").unwrap_or("").trim().to_string())
            })
            .unwrap_or_default();

        Some((href, title))
    }

    fn infer_title_selector(cards: &[ElementRef<'_>]) -> String {
        let candidates = [
            "h3 a", "h2 a", "h4 a", "h3", "h2", "h4",
            "a[title]", ".title", "[class*=title]", "[class*=name]",
        ];
        for candidate in candidates {
            let Ok(sel) = Selector::parse(candidate) else { continue };
            let hits = cards
                .iter()
                .filter(|card| {
                    card.select(&sel)
                        .next()
                        .map(|e| !e.text().collect::<String>().trim().is_empty())
                        .unwrap_or(false)
                })
                .count();
            if !cards.is_empty() && hits * 2 >= cards.len() {
                return candidate.to_string();
            }
        }
        "a".to_string()
    }

    fn detect_pagination(&self, doc: &Html, origin: &str, host: &str) -> (String, String) {
        let Ok(sel) = Selector::parse("a[href]") else {
            return (String::new(), "none".to_string());
        };

        let mut style = "none".to_string();
        let mut found = false;

        for a in doc.select(&sel) {
            let href = a.value().attr("href").unwrap_or("");
            let rel = a.value().attr("rel").unwrap_or("").to_lowercase();
            let class = a.value().attr("class").unwrap_or("").to_lowercase();
            let aria = a.value().attr("aria-label").unwrap_or("").to_lowercase();
            let text = a.text().collect::<String>().trim().to_lowercase();

            let is_next = rel.split_whitespace().any(|r| r == "next")
                || text == "next"
                || text == "siguiente"
                || text == "»"
                || text == "›"
                || text.contains("siguiente")
                || class.contains("next")
                || class.contains("siguiente")
                || aria.contains("next")
                || aria.contains("siguiente");

            if !is_next {
                continue;
            }
            let Some(path) = Self::same_origin_path(href, origin, host) else { continue };
            if path.contains("page=") || path.contains("paged=") || path.contains("p=") {
                style = "query".to_string();
            } else if path.contains("/page/")
                || path.contains("/pagina/")
                || path.contains("/p/")
            {
                style = "path".to_string();
            }
            found = true;
            break;
        }

        // Si no hay enlace "siguiente", usar la paginación numérica como pista.
        if !found {
            for a in doc.select(&sel) {
                if a.text().collect::<String>().trim() != "2" {
                    continue;
                }
                let href = a.value().attr("href").unwrap_or("");
                let Some(path) = Self::same_origin_path(href, origin, host) else { continue };
                if path.contains("page=2") || path.contains("paged=2") {
                    style = "query".to_string();
                    found = true;
                    break;
                }
                if path.contains("/page/2") || path.contains("/pagina/2") {
                    style = "path".to_string();
                    found = true;
                    break;
                }
            }
        }

        let selector = if found {
            "a[rel=next], a.next, a[class*=next], a[class*=siguiente]".to_string()
        } else {
            String::new()
        };
        (selector, style)
    }

    fn normalize_locale(raw: &str) -> Option<String> {
        let value = raw.trim().trim_matches(['"', '\'', '{', '}']);
        if value.is_empty() || value.len() > 20 || value.chars().any(char::is_whitespace) {
            return None;
        }
        let base = value.split(['-', '_']).next().unwrap_or(value).to_lowercase();
        if base.is_empty() || !base.chars().all(|c| c.is_ascii_alphabetic()) {
            return None;
        }
        Some(base)
    }

    /// Idiomas que el sitio declara para su contenido o interfaz. Se leen de
    /// `<html lang>`, meta tags, JSON-LD y del estado hidratado de la SPA.
    /// Nunca se deduce un idioma a partir del texto visible.
    fn detect_site_locales(doc: &Html) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();

        if let Ok(sel) = Selector::parse("html[lang]") {
            if let Some(el) = doc.select(&sel).next() {
                Self::push_locale(&mut out, el.value().attr("lang").unwrap_or(""));
            }
        }
        if let Ok(sel) = Selector::parse("meta[property=og:locale], meta[http-equiv=content-language], meta[name=language]") {
            for el in doc.select(&sel) {
                Self::push_locale(&mut out, el.value().attr("content").unwrap_or(""));
            }
        }
        if let Ok(sel) = Selector::parse("script[type='application/ld+json'], script[type='application/json'], script#__NEXT_DATA__") {
            for el in doc.select(&sel) {
                let text = el.text().collect::<String>();
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(text.trim()) {
                    Self::collect_locale_values(&json, &mut out);
                }
            }
        }
        out
    }

    fn push_locale(out: &mut Vec<String>, raw: &str) {
        if let Some(locale) = Self::normalize_locale(raw) {
            if !out.contains(&locale) {
                out.push(locale);
            }
        }
    }

    /// Recorre un payload hidratado buscando `locale`, `defaultLocale`,
    /// `locales` e `inLanguage`. Los idiomas individuales se insertan antes
    /// que la lista para que el primero sea el idioma principal del sitio.
    fn collect_locale_values(value: &serde_json::Value, out: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(map) => {
                for key in ["locale", "defaultLocale", "inLanguage"] {
                    if let Some(v) = map.get(key) {
                        match v {
                            serde_json::Value::String(s) => {
                                if let Some(locale) = Self::normalize_locale(s) {
                                    if !out.contains(&locale) { out.push(locale); }
                                }
                            }
                            serde_json::Value::Array(items) => {
                                for item in items {
                                    if let Some(s) = item.as_str() {
                                        if let Some(locale) = Self::normalize_locale(s) {
                                            if !out.contains(&locale) { out.push(locale); }
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                if let Some(serde_json::Value::Array(items)) = map.get("locales") {
                    for item in items {
                        if let Some(s) = item.as_str() {
                            if let Some(locale) = Self::normalize_locale(s) {
                                if !out.contains(&locale) { out.push(locale); }
                            }
                        }
                    }
                }
                for (key, child) in map {
                    if !matches!(key.as_str(), "locale" | "defaultLocale" | "inLanguage" | "locales") {
                        Self::collect_locale_values(child, out);
                    }
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    Self::collect_locale_values(item, out);
                }
            }
            _ => {}
        }
    }

    /// Filtros declarados en el estado hidratado de una SPA. Sólo se aceptan
    /// estructuras explícitas (`param`/`value`/`values`); un diccionario de
    /// traducciones bajo `filter` no es una lista de parámetros.
    fn detect_payload_filters(doc: &Html, search_param: Option<&str>) -> Vec<SiteFilterParam> {
        let mut out: Vec<SiteFilterParam> = Vec::new();
        let Ok(sel) = Selector::parse("script[type='application/json'], script#__NEXT_DATA__") else {
            return out;
        };
        for el in doc.select(&sel) {
            let text = el.text().collect::<String>();
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(text.trim()) {
                Self::collect_payload_filters(&json, search_param, &mut out);
            }
        }
        out
    }

    fn collect_payload_filters(
        value: &serde_json::Value,
        search_param: Option<&str>,
        out: &mut Vec<SiteFilterParam>,
    ) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, child) in map {
                    if matches!(key.as_str(), "filterParams" | "queryParams" | "filters") {
                        Self::parse_param_definition(child, search_param, out);
                    }
                    Self::collect_payload_filters(child, search_param, out);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    Self::collect_payload_filters(item, search_param, out);
                }
            }
            _ => {}
        }
    }

    fn parse_param_definition(
        value: &serde_json::Value,
        search_param: Option<&str>,
        out: &mut Vec<SiteFilterParam>,
    ) {
        let mut definitions: Vec<&serde_json::Value> = Vec::new();
        match value {
            serde_json::Value::Array(items) => definitions.extend(items.iter()),
            other => definitions.push(other),
        }
        for definition in definitions {
            let Some(map) = definition.as_object() else { continue };
            let param = map
                .get("param")
                .or_else(|| map.get("filterParam"))
                .or_else(|| map.get("queryParam"))
                .or_else(|| map.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            if param.is_empty() || Self::ignored_filter_param(param, search_param) {
                continue;
            }
            let mut values: Vec<String> = Vec::new();
            for key in ["value", "values", "options"] {
                if let Some(v) = map.get(key) {
                    match v {
                        serde_json::Value::String(s) => values.push(s.clone()),
                        serde_json::Value::Array(items) => {
                            for item in items {
                                match item {
                                    serde_json::Value::String(s) => values.push(s.clone()),
                                    serde_json::Value::Object(o) => {
                                        if let Some(s) = o.get("value").and_then(|v| v.as_str()) {
                                            values.push(s.to_string());
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            let name = Self::canonical_filter_name(param);
            let mut seen: HashSet<String> = HashSet::new();
            for value in values {
                if value.is_empty() || !seen.insert(value.clone()) { continue; }
                out.push(SiteFilterParam {
                    name: name.clone(),
                    param: param.to_string(),
                    value,
                    kind: "select".to_string(),
                });
            }
            if seen.is_empty() {
                out.push(SiteFilterParam {
                    name,
                    param: param.to_string(),
                    value: String::new(),
                    kind: "text".to_string(),
                });
            }
        }
    }

    /// Idioma declarado por la propia página. Es una pista para elegir títulos,
    /// nunca una traducción: si no hay texto alternativo se usa el que haya.
    fn detect_language(doc: &Html) -> String {
        if let Ok(sel) = Selector::parse("html[lang]") {
            if let Some(v) = doc.select(&sel).next().and_then(|el| el.value().attr("lang")) {
                let v = v.trim();
                if !v.is_empty() {
                    return v.split(['-', '_']).next().unwrap_or(v).to_lowercase();
                }
            }
        }
        if let Ok(sel) = Selector::parse("meta[property=og:locale], meta[http-equiv=content-language]") {
            for el in doc.select(&sel) {
                let v = el.value().attr("content").unwrap_or("").trim();
                if !v.is_empty() {
                    return v.split(['-', '_']).next().unwrap_or(v).to_lowercase();
                }
            }
        }
        String::new()
    }

    fn ld_string(value: Option<&serde_json::Value>) -> Option<String> {
        value
            .and_then(|v| if v.is_string() { v.as_str() } else { v.get("name").and_then(|n| n.as_str()) })
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    fn ld_types(node: &serde_json::Value) -> Vec<String> {
        let Some(t) = node.get("@type") else { return Vec::new() };
        match t {
            serde_json::Value::String(s) => vec![s.to_lowercase()],
            serde_json::Value::Array(a) => a.iter().filter_map(|v| v.as_str()).map(|s| s.to_lowercase()).collect(),
            _ => Vec::new(),
        }
    }

    fn collect_ld_nodes<'a>(value: &'a serde_json::Value, out: &mut Vec<&'a serde_json::Value>) {
        match value {
            serde_json::Value::Array(items) => {
                for item in items {
                    Self::collect_ld_nodes(item, out);
                }
            }
            serde_json::Value::Object(_) => {
                if let Some(graph) = value.get("@graph") {
                    Self::collect_ld_nodes(graph, out);
                }
                out.push(value);
            }
            _ => {}
        }
    }

    fn ld_search_action(node: &serde_json::Value) -> Option<SiteSearchAction> {
        let actions = node.get("potentialAction")?;
        let candidates: Vec<&serde_json::Value> = match actions {
            serde_json::Value::Array(a) => a.iter().collect(),
            other => vec![other],
        };
        for action in candidates {
            let types = Self::ld_types(action);
            if !types.iter().any(|t| t == "searchaction") {
                continue;
            }
            let template = action
                .get("target")
                .and_then(|t| if t.is_string() { t.as_str() } else { t.get("urlTemplate").and_then(|u| u.as_str()) })
                .unwrap_or("")
                .trim()
                .to_string();
            if template.is_empty() || !template.contains("{search_term_string}") {
                continue;
            }
            let query_param = Url::parse(&template.replace("{search_term_string}", "AEXTQUERY"))
                .ok()
                .and_then(|u| u.query_pairs().find(|(_, v)| v.contains("AEXTQUERY")).map(|(k, _)| k.to_string()));
            return Some(SiteSearchAction { url_template: template, query_param });
        }
        None
    }

    /// Catálogo y búsqueda declarados con schema.org. Esto es lo que permite
    /// generar listados reales en SPAs donde el HTML estático está vacío.
    fn detect_structured_data(
        doc: &Html,
        origin: &str,
        _host: &str,
    ) -> (Vec<CatalogRecord>, Option<SiteSearchAction>) {
        let mut records = Vec::new();
        let mut seen = HashSet::new();
        let mut search_action = None;
        let Ok(sel) = Selector::parse("script[type='application/ld+json']") else {
            return (records, search_action);
        };
        for script in doc.select(&sel) {
            let raw = script.text().collect::<String>();
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else { continue };
            let mut nodes = Vec::new();
            Self::collect_ld_nodes(&json, &mut nodes);
            for node in nodes {
                let types = Self::ld_types(node);
                if types.iter().any(|t| t == "itemlist") {
                    if let Some(items) = node.get("itemListElement").and_then(|v| v.as_array()) {
                        for item in items {
                            let obj = item.get("item").filter(|v| v.is_object()).unwrap_or(item);
                            let url = Self::ld_string(obj.get("url")).unwrap_or_default();
                            if url.is_empty() || !url.starts_with(origin) || !seen.insert(url.clone()) {
                                continue;
                            }
                            let title = Self::ld_string(obj.get("name")).unwrap_or_default();
                            if title.is_empty() || records.len() >= 40 {
                                continue;
                            }
                            records.push(CatalogRecord {
                                title,
                                alternate_title: Self::ld_string(obj.get("alternateName")),
                                language: Self::ld_string(obj.get("inLanguage")),
                                image: Self::ld_string(obj.get("image")),
                                url,
                            });
                        }
                    }
                }
                if search_action.is_none() && types.iter().any(|t| t == "website") {
                    search_action = Self::ld_search_action(node);
                }
            }
        }
        (records, search_action)
    }

    /// Convierte `https://host/catalog?name={search_term_string}` en la ruta y
    /// el parámetro que el generador sabe usar. Devuelve None si el template
    /// apunta a otro dominio (no se siguen enlaces externos).
    fn search_template_to_path(template: &str, _origin: &str, _host: &str) -> Option<(String, String)> {
        let parsed = Url::parse(&template.replace("{search_term_string}", "AEXTQUERY")).ok()?;
        if parsed.host_str() != Some(_host) {
            return None;
        }
        let pairs: Vec<(String, String)> = parsed
            .query_pairs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let param = pairs
            .iter()
            .find(|(_, v)| v.contains("AEXTQUERY"))
            .map(|(k, _)| k.clone())?;
        let rest: Vec<(String, String)> = pairs.into_iter().filter(|(k, _)| k != &param).collect();
        let path = if rest.is_empty() {
            parsed.path().to_string()
        } else {
            let mut serializer = url::form_urlencoded::Serializer::new(String::new());
            for (k, v) in &rest {
                serializer.append_pair(k, v);
            }
            format!("{}?{}", parsed.path(), serializer.finish())
        };
        Some((path, param))
    }

    /// Extrae bases de API REST y rutas de recursos citadas en un bundle JS.
    /// Sólo acepta URLs http(s) como base y rutas que aparecen en llamadas
    /// `.get(...)` o concatenadas a `apiVersion`, para no confundir rutas de la
    /// interfaz (`/catalog`) con recursos de datos (`/genres`).
    fn detect_api_candidates(text: &str) -> ApiCandidates {
        let mut bases: Vec<String> = Vec::new();
        if let Ok(re) = Regex::new(r#"(?i)baseURL\s*:\s*["']([^"']{8,200})["']"#) {
            for cap in re.captures_iter(text) {
                let base = cap[1].trim().trim_end_matches('/');
                if (base.starts_with("https://") || base.starts_with("http://"))
                    && base.len() > 12
                    && !bases.iter().any(|b| b == base)
                {
                    bases.push(base.to_string());
                }
            }
        }

        // Rutas privadas que nunca son catálogo: perfil, sesión, pago, etc.
        const PRIVATE_PATHS: &[&str] = &[
            "auth", "token", "profile", "history", "payment", "notification",
            "settings", "login", "logout", "refresh", "device", "session", "admin",
        ];
        let patterns = [
            r#"(?i)\.get\(\s*["'](/[a-z][a-z0-9/_-]{1,60})["']"#,
            r#"(?i)\.get\(\s*[^,;]{0,50}\+\s*["'](/[a-z][a-z0-9/_-]{1,60})["']"#,
            r#"(?i)(?:apiVersion|apiPath|api_path)\s*\+\s*["'](/[a-z][a-z0-9/_-]{1,60})["']"#,
        ];
        let mut paths: Vec<String> = Vec::new();
        for pattern in patterns {
            if let Ok(re) = Regex::new(pattern) {
                for cap in re.captures_iter(text) {
                    let raw = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                    let path = raw.split('?').next().unwrap_or(raw).trim_end_matches('/');
                    if path.len() < 3 || path.matches('/').count() > 2 { continue; }
                    if path.contains('{') || path.contains('[') || path.contains('.') { continue; }
                    let low = path.to_lowercase();
                    if PRIVATE_PATHS.iter().any(|p| low.contains(p)) { continue; }
                    if !paths.iter().any(|old| old == path) {
                        paths.push(path.to_string());
                    }
                }
            }
        }
        let version = Self::detect_api_version(text);
        ApiCandidates { bases, paths, version }
    }

    /// Versión de API declarada por el propio bundle (`apiVersion = "v1"`,
    /// `apiVersion: 'v2'`, o la exportación minificada `apiVersion,()=>n` con
    /// `n="v1"`). Sin ella las URLs quedan en `.../api/genres` y el sitio
    /// responde 404 aunque el recurso exista en `.../api/v1/genres`.
    fn detect_api_version(text: &str) -> Option<String> {
        for pattern in [
            r#"(?i)apiVersion\s*[:=]\s*["']([^"']{1,20})["']"#,
            r#"(?i)["']apiVersion["']\s*,\s*\(\)\s*=>\s*["']([^"']{1,20})["']"#,
        ] {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(cap) = re.captures(text) {
                    let version = cap[1].trim();
                    if !version.is_empty() {
                        return Some(version.to_string());
                    }
                }
            }
        }
        // Turbopack exporta `apiVersion,()=>n`; resolver la variable local.
        if let Ok(re) = Regex::new(r#"apiVersion\s*,\s*\(\)\s*=>\s*([A-Za-z_$][\w$]*)"#) {
            if let Some(cap) = re.captures(text) {
                let variable = regex::escape(&cap[1]);
                let pattern = format!(r#"\b{}\s*=\s*["']([^"']{{1,20}})["']"#, variable);
                if let Ok(vre) = Regex::new(&pattern) {
                    if let Some(value) = vre.captures(text) {
                        return Some(value[1].to_string());
                    }
                }
            }
        }
        None
    }

    /// Convierte un JSON cualquiera en un `ApiListResult` cuando hay evidencia
    /// de una lista de registros. Sin array de objetos, no se guarda nada.
    fn parse_api_list(url: &str, json: &serde_json::Value) -> Option<ApiListResult> {
        let arr = json
            .as_array()
            .or_else(|| json.get("data").and_then(|v| v.as_array()))
            .or_else(|| json.get("items").and_then(|v| v.as_array()))
            .or_else(|| json.get("results").and_then(|v| v.as_array()))
            .or_else(|| json.get("content").and_then(|v| v.as_array()))?;
        if arr.is_empty() || arr.len() > 1000 || !arr.iter().all(|v| v.is_object()) {
            return None;
        }
        let resource = Url::parse(url)
            .ok()
            .and_then(|u| {
                u.path_segments()
                    .and_then(|s| s.filter(|p| !p.is_empty()).last().map(|p| p.to_string()))
            })
            .unwrap_or_default();
        let options: Vec<ApiOption> = arr
            .iter()
            .filter_map(|v| {
                let obj = v.as_object()?;
                let value = Self::json_scalar(obj, &["id", "value", "key", "slug"])?;
                let label = Self::json_scalar(obj, &["name", "label", "title"])?;
                Some(ApiOption { value, label })
            })
            .collect();
        let first = arr[0].as_object()?;
        let title_field = Self::detect_json_field(first, &["title", "name", "original_title", "originalName"]);
        let image_field = Self::detect_json_field(
            first,
            &["poster_link", "poster", "image", "thumbnail", "cover", "img", "backdrop_path"],
        );
        let url_field = Self::detect_json_field(first, &["permalink", "url", "slug", "id"]);
        let sample_json = serde_json::to_string(&arr[..arr.len().min(3)]).unwrap_or_default();
        Some(ApiListResult {
            url: url.to_string(),
            resource,
            options,
            sample_json,
            title_field,
            image_field,
            url_field,
            record_count: arr.len(),
        })
    }

    fn json_scalar(obj: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<String> {
        for key in keys {
            if let Some(value) = obj.get(*key) {
                if let Some(text) = value.as_str() {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
                if let Some(number) = value.as_u64() {
                    return Some(number.to_string());
                }
                if let Some(number) = value.as_i64() {
                    return Some(number.to_string());
                }
            }
        }
        None
    }

    fn detect_json_field(obj: &serde_json::Map<String, serde_json::Value>, candidates: &[&str]) -> String {
        for key in candidates {
            if obj.contains_key(*key) {
                return (*key).to_string();
            }
        }
        String::new()
    }

    /// Sólo se sondean hosts del propio sitio o de su API (p. ej. `frame.uz` y
    /// `api.frame.uz`). Evita convertir al analizador en un proxy de terceros.
    fn host_related_to(url: &str, site_host: &str) -> bool {
        let Ok(parsed) = Url::parse(url) else { return false };
        let Some(host) = parsed.host_str() else { return false };
        let site = site_host.trim_start_matches("www.").trim().to_lowercase();
        let candidate = host.trim_start_matches("www.").trim().to_lowercase();
        if site.is_empty() || candidate.is_empty() {
            return false;
        }
        candidate == site
            || candidate.ends_with(&format!(".{}", site))
            || site.ends_with(&format!(".{}", candidate))
    }

    /// Prioriza recursos que suelen alimentar catálogo y filtros, para que un
    /// bundle con decenas de rutas no desplace a los endpoints útiles.
    fn api_path_priority(path: &str) -> u8 {
        const PREFERRED: &[&str] = &[
            "genres", "categories", "countries", "languages", "tags", "contents",
            "content", "catalog", "movies", "films", "series", "shows", "list",
            "collections", "top", "new", "popular", "search",
        ];
        let low = path.trim_start_matches('/').to_lowercase();
        PREFERRED
            .iter()
            .position(|name| low == *name || low.starts_with(&format!("{}s", name)))
            .map(|index| index as u8)
            .unwrap_or(u8::MAX)
    }

    /// Sondea de forma genérica los recursos detectados. Nunca envía credenciales
    /// ni cookies: si el endpoint exige sesión, no responde JSON y se descarta.
    async fn probe_api_lists(
        &self,
        candidates: &ApiCandidates,
        site_host: &str,
    ) -> Vec<ApiListResult> {
        let mut ordered: Vec<String> = candidates.paths.to_vec();
        ordered.sort_by_key(|path| Self::api_path_priority(path));
        let mut urls: Vec<String> = Vec::new();
        for base in &candidates.bases {
            for path in &ordered {
                let root = base.trim_end_matches('/');
                let mut variants = Vec::new();
                if let Some(version) = &candidates.version {
                    let version = version.trim().trim_start_matches('/').trim_end_matches('/');
                    if !version.is_empty() {
                        variants.push(format!("{}/{}{}", root, version, path));
                    }
                }
                variants.push(format!("{}{}", root, path));
                for full in variants {
                    if !urls.contains(&full) {
                        urls.push(full);
                    }
                }
            }
        }
        let mut out = Vec::new();
        for url in urls.into_iter().take(8) {
            if !Self::host_related_to(&url, site_host) {
                continue;
            }
            let Ok(response) = self
                .client
                .get(&url)
                .header("Accept", "application/json")
                .header("Accept-Language", "en")
                .timeout(std::time::Duration::from_secs(8))
                .send()
                .await
            else {
                continue;
            };
            if !response.status().is_success() {
                continue;
            }
            let Ok(text) = response.text().await else { continue };
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            if let Some(result) = Self::parse_api_list(&url, &json) {
                out.push(result);
            }
        }
        out
    }

    /// Parámetros de filtro declarados en bundles JS del mismo origen. No se
    /// adivina nada: sólo se aceptan Arrays de identificadores con nombres
    /// reconocibles de parámetro (year, genreIds, sortBy...) y pares
    /// `value/label` reales del propio bundle para enumerar los valores.
    fn detect_filters_from_js(text: &str, search_param: Option<&str>) -> Vec<SiteFilterParam> {
        let Ok(assign_re) = Regex::new(r#"\b(?:let|var|const)\s+([A-Za-z_$][\w$]*)\s*=\s*\[([^\]]{0,1500})\]"#) else { return Vec::new() };
        let Ok(any_assign_re) = Regex::new(r#"\b([A-Za-z_$][\w$]*)\s*=\s*\[([^\]]{0,1500})\]"#) else { return Vec::new() };
        let Ok(plain_re) = Regex::new(r"\[([^\]]{0,1500})\]") else { return Vec::new() };
        let Ok(quoted_re) = Regex::new(r#""([A-Za-z][A-Za-z0-9_]{1,39})""#) else { return Vec::new() };
        let Ok(spread_re) = Regex::new(r"\.\.\.([A-Za-z_$][\w$]*)") else { return Vec::new() };
        let Ok(value_re) = Regex::new(r#"value\s*:\s*"([^"]{1,60})"\s*,\s*label\s*:"#) else { return Vec::new() };

        let mut var_arrays: HashMap<String, Vec<String>> = HashMap::new();
        for cap in assign_re.captures_iter(text) {
            let tokens = Self::js_array_tokens(&cap[2], &quoted_re, &spread_re, &var_arrays);
            if !tokens.is_empty() {
                var_arrays.insert(cap[1].to_string(), tokens);
            }
        }

        let mut params: Vec<String> = Vec::new();
        let mut add_param = |raw: &str| {
            let param = raw.trim();
            if param.is_empty()
                || Self::ignored_filter_param(param, search_param)
                || matches!(param.to_lowercase().as_str(), "value" | "label" | "key" | "title" | "description" | "image" | "url" | "index" | "item" | "items" | "children" | "class" | "style" | "data" | "props" | "state" | "event")
                || params.iter().any(|p| p == param)
            {
                return;
            }
            params.push(param.to_string());
        };

        let mut raw_arrays: Vec<String> = Vec::new();
        for cap in any_assign_re.captures_iter(text) {
            raw_arrays.push(cap[2].to_string());
        }
        for cap in plain_re.captures_iter(text) {
            raw_arrays.push(cap[1].to_string());
        }
        let known = [
            "year", "years", "status", "quality", "qualities", "cues", "display", "dub",
            "sortby", "sort_by", "genre", "genres", "country", "countries", "category",
            "categories", "audio", "name", "format", "season", "view", "subtitle",
            "sub_language", "audiolanguage", "audiolanguageids",
        ];
        for raw in raw_arrays {
            // Los Arrays de opciones son objetos `{value, label}`, no params.
            if raw.contains("value:") || raw.contains("label:") {
                continue;
            }
            let tokens = Self::js_array_tokens(&raw, &quoted_re, &spread_re, &var_arrays);
            if tokens.len() < 3 || tokens.len() > 60 {
                continue;
            }
            let identifiers = tokens.iter().all(|t| {
                t.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
                    && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            });
            if !identifiers {
                continue;
            }
            let hits = tokens
                .iter()
                .filter(|t| {
                    let lower = t.to_lowercase();
                    known.contains(&lower.as_str())
                        || lower.ends_with("ids")
                        || lower.ends_with("id")
                        || lower.ends_with("by")
                })
                .count();
            if hits < 2 {
                continue;
            }
            for token in &tokens {
                add_param(token);
            }
        }

        let aliases: Vec<(String, Vec<String>)> = params
            .iter()
            .map(|param| {
                let mut names = vec![param.to_lowercase()];
                let base = param
                    .strip_suffix("Ids")
                    .or_else(|| param.strip_suffix("ID"))
                    .or_else(|| param.strip_suffix("By"))
                    .unwrap_or(param);
                if base != param.as_str() {
                    names.push(base.to_lowercase());
                }
                if let Some(first) = base.chars().next() {
                    names.push(
                        format!("on{}{}", first.to_uppercase(), &base[first.len_utf8()..])
                            .to_lowercase(),
                    );
                }
                (param.clone(), names)
            })
            .collect();
        let valid_value = |value: &str| {
            value.len() >= 2
                && value.len() <= 30
                && value.chars().next().map(|c| c.is_ascii_lowercase()).unwrap_or(false)
                && value.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        };
        let Ok(options_re) = Regex::new(r#"([A-Za-z_$][\w$]*)\s*=\s*\([^\[]{0,200}\[\s*\{([^\]]{0,5000})\]\s*,"#) else {
            return Vec::new();
        };

        // Cada Array de opciones se guarda en una variable que después se pasa
        // como `options:<var>` al componente. El contexto siguiente a ese uso
        // (`filter.sort`, `catalog-status`, ...) es lo que asocia los valores
        // con su parámetro, en vez de suponer que el orden de los Arrays coincide.
        let mut values_by_param: HashMap<String, Vec<String>> = HashMap::new();
        for cap in options_re.captures_iter(text) {
            let var = cap[1].to_string();
            let mut values: Vec<String> = Vec::new();
            for value_cap in value_re.captures_iter(&cap[2]) {
                if let Some(raw) = value_cap.get(1) {
                    let value = raw.as_str().trim();
                    if valid_value(value) && !values.iter().any(|v| v.as_str() == value) && values.len() < 12 {
                        values.push(value.to_string());
                    }
                }
            }
            if values.is_empty() {
                continue;
            }
            let needle = format!("options:{}", var);
            let Some(pos) = text.find(&needle) else { continue };
            let start = pos + needle.len();
            let mut end = (start + 700).min(text.len());
            while end > start && !text.is_char_boundary(end) {
                end -= 1;
            }
            let context = text[start..end].to_lowercase();
            let mut best: Option<(&str, usize)> = None;
            for (param, names) in &aliases {
                for alias in names {
                    if let Some(found) = context.find(alias.as_str()) {
                        if best.map(|(_, previous)| found < previous).unwrap_or(true) {
                            best = Some((param.as_str(), found));
                        }
                    }
                }
            }
            if let Some((param, _)) = best {
                let entry = values_by_param.entry(param.to_string()).or_default();
                for value in values {
                    if !entry.contains(&value) {
                        entry.push(value);
                    }
                }
            }
        }

        let mut result: Vec<SiteFilterParam> = Vec::new();
        for param in &params {
            let name = Self::canonical_filter_name(param);
            let values = values_by_param.get(param).cloned().unwrap_or_default();
            if values.is_empty() {
                result.push(SiteFilterParam {
                    name,
                    param: param.clone(),
                    value: String::new(),
                    kind: "text".to_string(),
                });
            } else {
                for value in values {
                    result.push(SiteFilterParam {
                        name: name.clone(),
                        param: param.clone(),
                        value,
                        kind: "select".to_string(),
                    });
                }
            }
        }
        result
    }

    fn js_array_tokens(
        raw: &str,
        quoted_re: &Regex,
        spread_re: &Regex,
        var_arrays: &HashMap<String, Vec<String>>,
    ) -> Vec<String> {
        let mut tokens: Vec<String> = Vec::new();
        for cap in quoted_re.captures_iter(raw) {
            let token = cap[1].to_string();
            if !tokens.contains(&token) {
                tokens.push(token);
            }
        }
        for cap in spread_re.captures_iter(raw) {
            if let Some(expanded) = var_arrays.get(&cap[1]) {
                for token in expanded {
                    if !tokens.contains(token) {
                        tokens.push(token.clone());
                    }
                }
            }
        }
        tokens
    }

    fn ignored_filter_param(key: &str, search_param: Option<&str>) -> bool {
        let k = key.to_lowercase();
        if k.starts_with("utm_")
            || matches!(
                k.as_str(),
                "page" | "paged" | "p" | "fbclid" | "gclid" | "ref" | "sort" | "order" | "limit" | "offset"
            )
            || search_param.map(|s| s == key).unwrap_or(false)
        {
            return true;
        }

        // Las constantes de configuración de un bundle son MAYUSCULAS_CON_GUION:
        // claves de localStorage/entorno, nunca parámetros de una URL de catálogo.
        if key.len() > 2
            && key.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            && key.contains('_')
        {
            return true;
        }

        // Credenciales, sesión y campos internos de cuenta se filtran en los
        // bundles como arrays de nombres de petición. Ninguno es un filtro real
        // del catálogo y exponerlos como filtros de texto sonaba a capacidades
        // que el sitio no ofrece.
        const NON_FILTER_PARAMS: &[&str] = &[
            "password", "passwd", "pwd", "token", "secret", "authorization", "auth",
            "bearer", "session", "sessionid", "session_id", "sid", "jwt", "otp",
            "email", "emailaddress", "referer", "referrer", "credential", "credentials",
            "cookie", "apikey", "api_key", "apiversion", "access_token", "accesstoken",
            "refresh_token", "refreshtoken", "id_token", "idtoken", "adsessionid",
            "signature", "sig", "userid", "ipaddress", "code", "userregid", "usertype",
            "userstatus", "userprofileid", "userprofilerefid", "userprofilecategory",
            "userprofilemaster", "packagesource", "packagestatus",
        ];
        if NON_FILTER_PARAMS.contains(&k.as_str()) {
            return true;
        }

        // Prefijos de claves internas conocidas (localStorage, constantes de
        // fecha, ajustes temporales y de instalación).
        const NON_FILTER_PREFIXES: &[&str] = &["ls_", "default_", "temp_", "installment_", "next_"];
        if NON_FILTER_PREFIXES.iter().any(|p| k.starts_with(p)) {
            return true;
        }

        false
    }

    /// Etiqueta canónica en inglés para un parámetro real del sitio. El texto
    /// visible del enlace puede estar en cualquier idioma (o ser un banner),
    /// así que se ignora: la equivalencia se hace por nombre de parámetro.
    fn canonical_filter_name(param: &str) -> String {
        let key = param.to_lowercase();
        let known = match key.as_str() {
            "year" | "years" | "releaseyear" => "Year",
            "country" | "countryid" | "countryids" | "countries" => "Country",
            "genre" | "genres" | "genreid" | "genreids" => "Genre",
            "category" | "categories" | "categoryid" | "categoryids" => "Category",
            "audio" | "audiolanguage" | "audiolanguageid" | "audiolanguageids" | "audio_language" => "Audio Language",
            "quality" | "qualities" => "Quality",
            "copyrightholderids" | "copyrightholder" => "Copyright Holder",
            "isframedub" | "dub" | "dubbed" | "isdub" => "Dubbed",
            "sortby" | "sort_by" | "sorting" => "Sort By",
            "status" => "Status",
            "cues" | "subtitle" | "subtitles" | "sub_language" => "Subtitles",
            "display" | "view" => "Display",
            "name" | "q" | "query" | "search" => "Search",
            _ => "",
        };
        if !known.is_empty() {
            return known.to_string();
        }
        let without_suffix = key
            .strip_suffix("ids")
            .or_else(|| key.strip_suffix("id"))
            .unwrap_or(&key);
        let spaced = without_suffix.replace(['_', '-'], " ");
        let mut words = String::new();
        let mut prev_lower = false;
        for c in spaced.chars() {
            if c.is_uppercase() && prev_lower {
                words.push(' ');
            }
            words.push(c);
            prev_lower = c.is_lowercase() || c.is_ascii_digit();
        }
        words
            .split_whitespace()
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Parámetros de filtro realmente presentes en enlaces o formularios.
    fn detect_filter_params(
        doc: &Html,
        origin: &str,
        host: &str,
        search_param: Option<&str>,
    ) -> Vec<SiteFilterParam> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        let mut add = |param: &str, value: &str| {
            if param.is_empty() || value.is_empty() || Self::ignored_filter_param(param, search_param) {
                return;
            }
            if seen.insert(format!("{param}={value}")) && result.len() < 40 {
                result.push(SiteFilterParam {
                    name: Self::canonical_filter_name(param),
                    param: param.to_string(),
                    value: value.to_string(),
                    kind: "check".to_string(),
                });
            }
        };

        if let Ok(sel) = Selector::parse("a[href]") {
            for a in doc.select(&sel) {
                let Some(path) = Self::same_origin_path(a.value().attr("href").unwrap_or(""), origin, host) else { continue };
                let Ok(url) = Url::parse(&format!("{origin}{path}")) else { continue };
                for (k, v) in url.query_pairs() {
                    add(k.as_ref(), v.as_ref());
                }
            }
        }
        if let Ok(sel) = Selector::parse("form input[name]") {
            for input in doc.select(&sel) {
                let key = input.value().attr("name").unwrap_or("");
                let value = input.value().attr("value").unwrap_or("");
                add(key, value);
            }
        }
        result
    }

    /// Descarta menús, cabeceras, pies, migas de pan, banners y paginación.
    /// Estos bloques usan las mismas clases (`item`, `card`, `link`) que las
    /// tarjetas reales y son la causa más frecuente de catálogos basura.
    fn is_navigation_element(el: ElementRef<'_>) -> bool {
        const NAV_TOKENS: &[&str] = &[
            "nav", "menu", "footer", "header", "breadcrumb", "sidebar", "dropdown",
            "promo", "banner", "pagination", "pager", "navbar", "toolbar",
        ];
        let own = format!(
            "{} {}",
            el.value().attr("class").unwrap_or(""),
            el.value().attr("id").unwrap_or("")
        )
        .to_lowercase();
        if Self::matches_any(&own, NAV_TOKENS) {
            return true;
        }
        let mut current = el.parent();
        while let Some(parent) = current {
            if let Some(parent_el) = parent.value().as_element() {
                if matches!(parent_el.name(), "nav" | "header" | "footer" | "aside") {
                    return true;
                }
                let hay = format!(
                    "{} {}",
                    parent_el.attr("class").unwrap_or(""),
                    parent_el.attr("id").unwrap_or("")
                )
                .to_lowercase();
                if Self::matches_any(&hay, NAV_TOKENS) {
                    return true;
                }
            }
            current = parent.parent();
        }
        false
    }

    fn detect_catalog(&self, doc: &Html, origin: &str, host: &str) -> CatalogInfo {
        let candidates = [
            "article",
            "li.item, li.card, div.item, div.card",
            "div[class*=card], div[class*=item], div[class*=poster], div[class*=cover]",
            "div.page-item-detail, div.c-tabs-item__content",
            "article.item-movies, article.item",
        ];

        let mut best = CatalogInfo {
            card_selector: String::new(),
            title_selector: String::new(),
            next_page_selector: String::new(),
            page_style: "none".to_string(),
            count: 0,
            samples: Vec::new(),
        };

        for sel_str in candidates {
            let Ok(sel) = Selector::parse(sel_str) else { continue };
            let cards: Vec<ElementRef<'_>> = doc
                .select(&sel)
                .filter(|el| !Self::is_navigation_element(*el))
                .collect();
            if cards.len() < 4 {
                continue;
            }

            let mut valid = 0;
            let mut samples: Vec<String> = Vec::new();
            for card in &cards {
                if Self::is_navigation_element(*card) {
                    continue;
                }
                if let Some((_href, title)) = Self::card_title(*card, host) {
                    valid += 1;
                    if samples.len() < 6 && !title.is_empty() && !samples.contains(&title) {
                        samples.push(title);
                    }
                }
            }

            let ratio = valid as f32 / cards.len() as f32;
            if valid < 4 || ratio < 0.6 || valid <= best.count {
                continue;
            }

            let (next_page_selector, page_style) =
                self.detect_pagination(doc, origin, host);
            best = CatalogInfo {
                card_selector: sel_str.to_string(),
                title_selector: Self::infer_title_selector(&cards),
                next_page_selector,
                page_style,
                count: valid,
                samples,
            };
        }

        // Portadas hechas con React/Next/Vue no usan `article`/`div.card`: sus
        // tarjetas son enlaces con una clase generada (p. ej. `a.TileLink__x`).
        // Si las listas fijas no dieron nada, se agrupa por token de clase.
        if best.count < 4 {
            if let Some(fallback) = self.detect_repeated_links(doc, origin, host) {
                if fallback.count > best.count {
                    best = fallback;
                }
            }
        }

        best
    }

    /// Detecta una rejilla de tarjetas sin clases conocidas.
    ///
    /// Agrupa los enlaces internos por cada token de su atributo `class` y se
    /// queda con el grupo que más se repite entre enlaces distintos que además
    /// contienen una imagen. El requisito de la imagen es lo que separa una
    /// tarjeta de catálogo de un menú de navegación.
    fn detect_repeated_links(&self, doc: &Html, origin: &str, host: &str) -> Option<CatalogInfo> {
        let link_sel = Selector::parse("a[href]").ok()?;
        let img_sel = Selector::parse("img").ok()?;

        let mut groups: HashMap<String, Vec<ElementRef<'_>>> = HashMap::new();
        for a in doc.select(&link_sel) {
            if Self::is_navigation_element(a) {
                continue;
            }
            let href = a.value().attr("href").unwrap_or("").trim();
            let Some(path) = Self::same_origin_path(href, origin, host) else { continue };
            if path == "/" || path.is_empty() {
                continue;
            }
            // Sin imagen no es una tarjeta (descarta menús, breadcrumbs, footer).
            if a.select(&img_sel).next().is_none() {
                continue;
            }
            let class = a.value().attr("class").unwrap_or("");
            for token in class.split_whitespace() {
                if token.len() < 4 {
                    continue;
                }
                groups.entry(token.to_string()).or_default().push(a);
            }
        }

        let mut best: Option<(String, usize, Vec<String>)> = None;
        for (token, cards) in groups {
            if cards.len() < 6 {
                continue;
            }

            let mut hrefs = HashSet::new();
            let mut samples: Vec<String> = Vec::new();
            let mut with_alt = 0usize;
            for card in &cards {
                let href = card.value().attr("href").unwrap_or("").trim();
                hrefs.insert(href.to_string());
                let alt = card
                    .select(&img_sel)
                    .next()
                    .map(|img| img.value().attr("alt").unwrap_or("").trim().to_string())
                    .unwrap_or_default();
                if !alt.is_empty() {
                    with_alt += 1;
                    if samples.len() < 6 && !samples.contains(&alt) {
                        samples.push(alt);
                    }
                }
            }

            // Enlaces repetidos de verdad y con texto legible en la mayoría.
            if hrefs.len() < 6 || with_alt * 2 < cards.len() {
                continue;
            }
            let better = best.as_ref().map(|(_, count, _)| cards.len() > *count).unwrap_or(true);
            if better {
                best = Some((token, cards.len(), samples));
            }
        }

        let (token, count, samples) = best?;
        let plain_identifier = token
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            && !token.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(true);
        let card_selector = if plain_identifier {
            format!("a.{}", token)
        } else {
            format!("a[class*=\"{}\"]", token)
        };

        let (next_page_selector, page_style) = self.detect_pagination(doc, origin, host);

        Some(CatalogInfo {
            card_selector,
            // La tarjeta es el propio enlace: el título está en el `alt` de la
            // imagen o en el atributo `title`. `img` evita un selector vacío.
            title_selector: "img".to_string(),
            next_page_selector,
            page_style,
            count,
            samples,
        })
    }

    fn detect_section_paths(
        &self,
        doc: &Html,
        origin: &str,
        host: &str,
        _catalog: &CatalogInfo,
    ) -> (Option<String>, Option<String>) {
        let Ok(sel) = Selector::parse("a[href]") else {
            return (None, None);
        };

        let mut popular: Option<String> = None;
        let mut latest: Option<String> = None;
        let mut listing: Option<String> = None;

        for (index, a) in doc.select(&sel).enumerate() {
            // Las primeras ~500 anclas cubren header/nav/portada; evita recorrer
            // listados gigantes y confundir títulos con rutas de sección.
            if index > 500 {
                break;
            }
            let href = a.value().attr("href").unwrap_or("");
            let Some(path) = Self::same_origin_path(href, origin, host) else { continue };
            let text = a.text().collect::<String>().trim().to_lowercase();
            let hay = format!("{} {}", text, path.to_lowercase());

            if latest.is_none()
                && Self::matches_any(
                    &hay,
                    &[
                        "reciente", "recientes", "ultimo", "ultimos", "último", "últimos",
                        "latest", "updated", "actualizado", "estrenos", "novedades",
                        "new releases", "recien agregado", "recién agregado",
                    ],
                )
            {
                latest = Some(path.clone());
            }

            if popular.is_none()
                && Self::matches_any(
                    &hay,
                    &[
                        "popular", "populares", "tendencia", "tendencias", "trending",
                        "mas visto", "más visto", "top", "destacado", "destacados",
                        "featured", "ranking",
                    ],
                )
            {
                popular = Some(path.clone());
            }

            if listing.is_none()
                && Self::matches_any(
                    &hay,
                    &[
                        "anime", "animes", "series", "serie", "catalogo", "catálogo",
                        "catalog", "explorar", "browse", "biblioteca", "library",
                        "todas", "todos", "all",
                    ],
                )
            {
                listing = Some(path.clone());
            }
        }

        // El listado general es una alternativa razonable a "populares",
        // pero solo si el enlace existía de verdad en la página.
        if popular.is_none() {
            popular = listing;
        }

        (popular, latest)
    }

    fn detect_search(
        &self,
        doc: &Html,
        origin: &str,
        host: &str,
        cms: &str,
    ) -> (Option<String>, Option<String>) {
        let Ok(form_sel) = Selector::parse("form") else {
            return (None, None);
        };
        let Ok(input_sel) = Selector::parse("input") else {
            return (None, None);
        };

        for form in doc.select(&form_sel) {
            let mut param: Option<String> = None;
            for input in form.select(&input_sel) {
                let ty = input.value().attr("type").unwrap_or("text").to_lowercase();
                let name = input.value().attr("name").unwrap_or("").trim();
                if name.is_empty() {
                    continue;
                }
                let lower = name.to_lowercase();
                let looks_like_search = ty == "search"
                    || matches!(
                        lower.as_str(),
                        "q" | "s" | "query" | "search" | "buscar" | "keyword" | "keywords" | "term" | "k"
                    );
                if looks_like_search {
                    param = Some(name.to_string());
                    if ty == "search" {
                        break;
                    }
                }
            }

            let Some(param) = param else { continue };
            let action = form.value().attr("action").unwrap_or("").trim();
            let path = if action.is_empty() || action == "#" {
                Some("/".to_string())
            } else {
                Self::same_origin_path(action, origin, host)
            };
            if let Some(path) = path {
                return (Some(path), Some(param));
            }
        }

        // WordPress (Dooplay/Madara) usa `?s=` en la raíz por convención del CMS.
        if matches!(cms, "dooplay" | "madara") {
            return (Some("/".to_string()), Some("s".to_string()));
        }

        (None, None)
    }

    fn detect_site_name(&self, doc: &Html, url: &str) -> String {
        // 1. El dominio es el identificador más estable. Evita nombres basura
        //    como "Bienvenido" o "Inicio" tomados del <title> de la portada.
        let domain_name = Self::extract_domain_label(url);
        if !domain_name.is_empty() && !Self::is_generic_title(&domain_name) {
            return domain_name;
        }

        // 2. Metadatos del sitio (og:site_name y equivalentes)
        let meta_selectors = [
            "meta[property='og:site_name']",
            "meta[name='application-name']",
            "meta[name='apple-mobile-web-app-title']",
        ];
        for sel_str in meta_selectors {
            if let Ok(sel) = Selector::parse(sel_str) {
                if let Some(el) = doc.select(&sel).next() {
                    if let Some(val) = el.value().attr("content") {
                        let clean = val.trim();
                        if Self::is_valid_name(clean) {
                            return Self::title_case_name(clean);
                        }
                    }
                }
            }
        }

        // 3. Revisar <title> buscando partes no genéricas
        let title_sel = Selector::parse("title").unwrap();
        if let Some(el) = doc.select(&title_sel).next() {
            let t = el.text().collect::<String>();
            let parts: Vec<&str> = t.split(&['|', '-', '–', '—', ':', '·', '•'][..]).map(|s| s.trim()).collect();
            for part in &parts {
                if Self::is_valid_name(part) {
                    return Self::title_case_name(part);
                }
            }
        }

        if !domain_name.is_empty() {
            return domain_name;
        }

        "AnimeExtension".to_string()
    }

    fn is_valid_name(s: &str) -> bool {
        let clean = s.trim();
        !clean.is_empty() && clean.chars().count() <= 42 && !Self::is_generic_title(clean)
    }

    fn is_generic_title(s: &str) -> bool {
        let lower = s.trim().to_lowercase();
        if lower.is_empty() {
            return true;
        }

        let generics = [
            "bienvenido", "bienvenida", "bienvenidos", "bienvenidas", "welcome", "home",
            "inicio", "index", "catalog", "catalogo", "catálogo", "search", "buscar",
            "anime", "animes", "manga", "mangas", "series", "serie", "peliculas",
            "películas", "movies", "home page", "pagina principal", "página principal",
            "principal", "portada", "not found", "404", "error", "no encontrado",
            "loading", "cargando", "redirecting", "redireccionando", "anime online",
            "manga online", "ver anime", "watch anime", "leer manga", "read manga",
            "default", "untitled", "sin titulo", "sin título", "just a moment",
            "attention required", "access denied", "cloudflare", "ddos-guard",
            "ver series", "watch series", "hentai", "raw", "scan", "scans",
        ];
        if generics.contains(&lower.as_str()) {
            return true;
        }

        if lower.chars().count() <= 2 {
            return true;
        }

        let prefixes = [
            "bienvenid", "welcome to", "inicio |", "inicio -", "home |", "home -",
            "cargando", "loading", "redirect", "redirig", "404", "not found",
            "pagina no", "página no", "access denied", "just a moment",
            "attention required", "cloudflare", "ddos-guard",
        ];
        prefixes.iter().any(|p| lower.starts_with(p))
    }

    fn title_case_name(s: &str) -> String {
        let words: Vec<String> = s
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    Some(first) => {
                        let mut out = String::new();
                        out.push(first.to_uppercase().collect::<String>().chars().next().unwrap_or(first));
                        out.push_str(&chars.as_str().to_lowercase());
                        out
                    }
                    None => String::new(),
                }
            })
            .filter(|w| !w.is_empty())
            .collect();

        if words.is_empty() {
            s.trim().to_string()
        } else {
            words.join(" ")
        }
    }

    fn extract_domain_label(url: &str) -> String {
        let without_proto = url.trim_start_matches("https://").trim_start_matches("http://");
        let host = without_proto.split('/').next().unwrap_or(without_proto);
        let host = host.split(':').next().unwrap_or(host);
        let labels: Vec<&str> = host.split('.').filter(|s| !s.is_empty()).collect();
        if labels.is_empty() {
            return String::new();
        }

        // Direcciones IP y hosts sin TLD: no sirven como nombre bonito.
        if labels.iter().all(|l| l.chars().all(|c| c.is_ascii_digit())) {
            return String::new();
        }

        let two_level = ["com", "co", "net", "org", "gob", "edu", "ac", "gov", "nic", "mil"];
        let mut idx = labels.len();
        if idx >= 1 {
            idx -= 1; // quitar TLD
        }
        if idx >= 2 {
            let mid = labels[idx - 1].to_ascii_lowercase();
            if two_level.contains(&mid.as_str()) && labels[idx].chars().count() <= 2 {
                idx -= 1; // quitar segundo nivel (co.uk, com.ar, ...)
            }
        }

        if idx == 0 {
            return String::new();
        }

        let skipped = [
            "www", "www1", "www2", "www3", "www4", "m", "mobile", "app", "api",
            "cdn", "beta", "new", "web", "ver", "watch", "online", "site",
        ];
        let mut pos = idx - 1;
        while pos > 0 && skipped.contains(&labels[pos].to_ascii_lowercase().as_str()) {
            pos -= 1;
        }

        let label = labels[pos];
        if label.is_empty()
            || label.chars().count() <= 1
            || skipped.contains(&label.to_ascii_lowercase().as_str())
            || two_level.contains(&label.to_ascii_lowercase().as_str())
        {
            return String::new();
        }

        let mut chars = label.chars();
        match chars.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    fn detect_favicon(&self, doc: &Html, base_url: &str) -> Option<String> {
        let selectors = [
            "link[rel*='icon'][sizes='192x192']",
            "link[rel*='apple-touch-icon']",
            "link[rel*='icon']",
            "link[rel='shortcut icon']"
        ];
        for sel_str in selectors {
            if let Ok(sel) = Selector::parse(sel_str) {
                if let Some(el) = doc.select(&sel).next() {
                    if let Some(href) = el.value().attr("href") {
                        if href.starts_with("http") {
                            return Some(href.to_string());
                        } else if href.starts_with("//") {
                            return Some(format!("https:{}", href));
                        } else {
                            return Some(format!("{}/{}", base_url, href.trim_start_matches('/')));
                        }
                    }
                }
            }
        }
        Some(format!("{}/favicon.ico", base_url))
    }

    fn detect_cms_and_type(&self, body: &str) -> (String, String) {
        let body_lower = body.to_lowercase();
        let is_manga = body_lower.contains("wp-manga")
            || body_lower.contains("madara")
            || body_lower.contains("mangathemesia")
            || body_lower.contains("theme-mangathemesia")
            || body_lower.contains("manhwa")
            || body_lower.contains("scanlation")
            || (body_lower.contains("manga") && !body_lower.contains("anime") && !body_lower.contains("video"));

        let cms = if body.contains("__sveltekit") || body.contains("data-sveltekit") || body.contains("/_app/immutable") {
            "sveltekit"
        } else if body.contains("dooplay") || body.contains("wp-content/themes/dooplay") {
            "dooplay"
        } else if body.contains("wp-manga") || body.contains("madara") {
            "madara"
        } else if body.contains("mangathemesia") || body.contains("theme-mangathemesia") {
            "mangathemesia"
        } else {
            "generic"
        };

        let ext_type = if is_manga { "manga" } else { "anime" };
        (cms.to_string(), ext_type.to_string())
    }

    fn detect_servers_and_dub(&self, body: &str, _base_url: &str) -> (Vec<String>, bool) {
        let mut hosts = HashSet::new();
        let known_hosts = [
            ("MP4Upload", "mp4upload.com"),
            ("Voe", "voe.sx"),
            ("StreamWish", "streamwish"),
            ("Byse", "byse"),
            ("Zilla", "player.zilla-networks.com"),
            ("Filemoon", "filemoon"),
            ("DoodStream", "dood"),
            ("UPNShare", "uns.bio"),
            ("Mega", "mega.nz"),
        ];

        for (label, pattern) in known_hosts {
            if body.contains(pattern) {
                hosts.insert(label.to_string());
            }
        }

        let has_sub_dub = body.contains("SUB:[") || body.contains("DUB:[") || body.contains("LAT:[")
            || body.contains("audio=dub") || body.contains("latino");

        (hosts.into_iter().collect(), has_sub_dub)
    }

    fn detect_selectors(&self, cms: &str) -> (String, String, String) {
        match cms {
            "dooplay" => (
                "article.item-movies, article.item, div.poster".to_string(),
                "div.data a, h3 a".to_string(),
                "div.pagination a.current + a".to_string(),
            ),
            "madara" => (
                "div.page-item-detail, div.c-tabs-item__content".to_string(),
                "div.post-title a, h3 a".to_string(),
                "div.nav-previous a, a.next".to_string(),
            ),
            "sveltekit" => (
                "article".to_string(),
                "h3, div[class*=text-2xs]".to_string(),
                "a:contains(»)".to_string(),
            ),
            _ => (
                "article, div.item, div.card".to_string(),
                "h3 a, h2 a, .title a".to_string(),
                "a.next, a[rel=next]".to_string(),
            ),
        }
    }
}

struct CatalogInfo {
    card_selector: String,
    title_selector: String,
    next_page_selector: String,
    page_style: String,
    count: usize,
    samples: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOG_HTML: &str = r#"<!doctype html>
<html><body>
<nav>
  <a href="/explorar">Explorar</a>
  <a href="/recientes">Recientes</a>
  <a href="/populares">Populares</a>
</nav>
<form action="/buscar"><input type="search" name="q"></form>
<main>
  <article><a href="/anime/1"><img alt="Uno"><h3>Uno</h3></a></article>
  <article><a href="/anime/2"><img alt="Dos"><h3>Dos</h3></a></article>
  <article><a href="/anime/3"><img alt="Tres"><h3>Tres</h3></a></article>
  <article><a href="/anime/4"><img alt="Cuatro"><h3>Cuatro</h3></a></article>
  <article><a href="/anime/5"><img alt="Cinco"><h3>Cinco</h3></a></article>
  <article><a href="/anime/6"><img alt="Seis"><h3>Seis</h3></a></article>
  <a rel="next" href="/explorar?page=2">Siguiente</a>
</main>
</body></html>"#;

    #[test]
    fn same_origin_path_only_accepts_internal_urls() {
        let origin = "https://example.com";
        assert_eq!(
            SiteAnalyzer::same_origin_path("/animes?page=2", origin, "example.com").as_deref(),
            Some("/animes?page=2")
        );
        assert_eq!(
            SiteAnalyzer::same_origin_path("animes", origin, "example.com").as_deref(),
            Some("/animes")
        );
        assert_eq!(
            SiteAnalyzer::same_origin_path("https://otro.com/x", origin, "example.com"),
            None
        );
        assert_eq!(
            SiteAnalyzer::same_origin_path("javascript:void(0)", origin, "example.com"),
            None
        );
    }

    #[test]
    fn detects_real_catalog_paths_and_search() {
        let analyzer = SiteAnalyzer::new();
        let doc = Html::parse_document(CATALOG_HTML);
        let origin = "https://example.com";
        let host = "example.com";

        let catalog = analyzer.detect_catalog(&doc, origin, host);
        assert_eq!(catalog.count, 6);
        assert_eq!(catalog.card_selector, "article");
        assert_eq!(catalog.page_style, "query");
        assert!(!catalog.next_page_selector.is_empty());
        assert!(catalog.samples.contains(&"Uno".to_string()));

        let (popular, latest) = analyzer.detect_section_paths(&doc, origin, host, &catalog);
        assert_eq!(popular.as_deref(), Some("/populares"));
        assert_eq!(latest.as_deref(), Some("/recientes"));

        let (search_path, search_param) = analyzer.detect_search(&doc, origin, host, "generic");
        assert_eq!(search_path.as_deref(), Some("/buscar"));
        assert_eq!(search_param.as_deref(), Some("q"));
    }

    #[test]
    fn detects_cards_from_repeated_link_classes() {
        let analyzer = SiteAnalyzer::new();
        let doc = Html::parse_document(
            r#"<!doctype html><html><body>
<nav>
  <a class="vds-nav-item" href="/latam/live-tv/">TV</a>
  <a class="vds-nav-item" href="/latam/movies/">Películas</a>
  <a class="vds-nav-item" href="/latam/series/">Series</a>
  <a class="vds-nav-item" href="/latam/kids/">Kids</a>
  <a class="vds-nav-item" href="/latam/sports/">Deportes</a>
  <a class="vds-nav-item" href="/latam/news/">Noticias</a>
</nav>
<main>
  <a class="TileLink_tileLink__pIMWG" href="/latam/shows/the-nanny-ptv3/"><img alt="The Nanny" src="/a.jpg"></a>
  <a class="TileLink_tileLink__pIMWG" href="/latam/shows/csi-miami-ptv1/"><img alt="CSI: Miami" src="/b.jpg"></a>
  <a class="TileLink_tileLink__pIMWG" href="/latam/shows/friends-ptv2/"><img alt="Friends" src="/c.jpg"></a>
  <a class="TileLink_tileLink__pIMWG" href="/latam/shows/seinfeld-ptv4/"><img alt="Seinfeld" src="/d.jpg"></a>
  <a class="TileLink_tileLink__pIMWG" href="/latam/shows/frasier-ptv5/"><img alt="Frasier" src="/e.jpg"></a>
  <a class="TileLink_tileLink__pIMWG" href="/latam/shows/cheers-ptv6/"><img alt="Cheers" src="/f.jpg"></a>
</main>
</body></html>"#,
        );

        let catalog = analyzer.detect_catalog(&doc, "https://pluto.tv", "pluto.tv");
        assert_eq!(catalog.count, 6);
        assert_eq!(catalog.card_selector, "a.TileLink_tileLink__pIMWG");
        assert_eq!(catalog.title_selector, "img");
        assert!(catalog.samples.contains(&"Friends".to_string()));
        // La barra de navegación no tiene imágenes: no debe ganar el grupo.
        assert!(!catalog.card_selector.contains("vds-nav-item"));
    }

    #[test]
    fn detects_episode_selector_ignoring_live_tv_menu() {
        let html = r#"<!doctype html><html><body>
<nav><a href="/latam/watch/live-tv/">TV en directo</a></nav>
<a class="EpisodeLink" href="/latam/shows/970009058/episode/6adb6f6e73415cc4ae5a4168/">T1 E1</a>
<a class="EpisodeLink" href="/latam/shows/970009058/episode/6adb6f7073415cc4ae5ab46a/">T1 E2</a>
<a class="EpisodeLink" href="/latam/shows/970009058/episode/6ada68747102b85b829f2ecb/">T1 E3</a>
<a href="/latam/shows/970009058/season/1/">Temporada 1</a>
</body></html>"#;

        let (selector, count) = SiteAnalyzer::detect_episode_selector(html).unwrap();
        assert_eq!(selector, "a[href*='/episode/']");
        assert_eq!(count, 3);
    }

    #[test]
    fn no_episode_selector_when_only_menu_links() {
        let html = r#"<!doctype html><html><body>
<a href="/ver/">Ver</a>
<a href="/latam/watch/live-tv/">TV en directo</a>
<a href="/series/">Series</a>
</body></html>"#;

        assert!(SiteAnalyzer::detect_episode_selector(html).is_none());
    }

    #[test]
    fn does_not_fabricate_paths_or_search() {
        let analyzer = SiteAnalyzer::new();
        let doc = Html::parse_document(
            r#"<!doctype html><html><body><div id="app"></div></body></html>"#,
        );
        let origin = "https://spa.example.com";
        let host = "spa.example.com";

        let catalog = analyzer.detect_catalog(&doc, origin, host);
        assert_eq!(catalog.count, 0);
        assert!(catalog.next_page_selector.is_empty());

        let (popular, latest) = analyzer.detect_section_paths(&doc, origin, host, &catalog);
        assert_eq!(popular, None);
        assert_eq!(latest, None);

        let (search_path, search_param) = analyzer.detect_search(&doc, origin, host, "generic");
        assert_eq!(search_path, None);
        assert_eq!(search_param, None);
    }

    #[test]
    fn wordpress_search_fallback_is_cms_specific() {
        let analyzer = SiteAnalyzer::new();
        let doc = Html::parse_document("<html><body></body></html>");
        let (path, param) =
            analyzer.detect_search(&doc, "https://wp.example.com", "wp.example.com", "dooplay");
        assert_eq!(path.as_deref(), Some("/"));
        assert_eq!(param.as_deref(), Some("s"));
    }

    #[test]
    fn script_urls_are_limited_and_same_origin_only() {
        let html = r#"<html><head>
          <script src="/_next/static/chunks/main.js"></script>
          <script src="https://cdn.therd.site/player.js"></script>
          <script src="/_next/static/chunks/player.js"></script>
          <script src="/_next/static/chunks/heavy.js"></script>
        </head></html>"#;

        let urls = SiteAnalyzer::script_urls(html, "https://site.example", 2);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://site.example/_next/static/chunks/main.js");
        assert!(urls.iter().all(|u| u.starts_with("https://site.example/")));
    }

    #[test]
    fn app_bundle_is_scanned_before_analytics_chunks() {
        let html = r#"<html><head>
          <script src="/scripts/tn/dit.svc.datahub.js"></script>
          <script src="/scripts/tn/adobe.min.js"></script>
          <script src="/_next/static/chunks/webpack.js"></script>
          <script src="/_next/static/chunks/pages/_app-33c.js"></script>
        </head></html>"#;
        let urls = SiteAnalyzer::script_urls(html, "https://site.example", 2);
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("pages/_app"), "el bundle del reproductor debe ir primero: {:?}", urls);
    }

    #[test]
    fn content_id_and_playback_methods_are_concrete() {
        let (kind, id) = SiteAnalyzer::content_id_from_hint("/latam/movies/693ace640c94b06078d2a33f/")
            .expect("contentId de ejemplo");
        assert_eq!(kind, "movie");
        assert_eq!(id, "693ace640c94b06078d2a33f");
        let caps = SiteCapabilities {
            player_cases: vec!["session-bootstrap".to_string(), "hls-master".to_string()],
            ..Default::default()
        };
        let methods = SiteAnalyzer::playback_methods(&caps);
        assert_eq!(methods, vec!["Sesión web".to_string(), "HLS".to_string()]);
        assert_eq!(SiteAnalyzer::playback_methods(&SiteCapabilities::default()), vec!["Sin método confirmado".to_string()]);
    }

    #[test]
    fn detects_real_stream_capabilities_from_a_master_hls() {
        let html = r#"<html><body><script>
          const master = "https://cdn.example.com/hls/master.m3u8";
          const gql = "https://example.com/api/tn/app-shell/graphql/";
          //#EXT-X-MEDIA:TYPE=AUDIO,LANGUAGE="en",NAME="English [Original]"
          //#EXT-X-MEDIA:TYPE=SUBTITLES,LANGUAGE="es",NAME="Español"
          //#EXT-X-STREAM-INF:BANDWIDTH=1042180,RESOLUTION=1280x720
        </script></body></html>"#;

        let caps = SiteAnalyzer::detect_capabilities(html);
        assert!(caps.stream_formats.contains(&"hls".to_string()));
        assert!(caps.qualities.contains(&"720p".to_string()));
        assert!(caps.audio_languages.iter().any(|v| v.contains("English")));
        assert!(caps.subtitle_languages.iter().any(|v| v.contains("es")));
        assert!(caps.api_endpoints.iter().any(|v| v.contains("/graphql")));
    }

    #[test]
    fn detects_session_boot_capability_without_an_api_path() {
        let html = r#"<html><head>
          <meta name="globalAppVersion" content="10.12.1096">
        </head><body><script>
          const boot = "https://boot.pluto.tv/v4/start";
          const stitcher = "https://cfd-v4-service-channel-stitcher.prd.pluto.tv/v2/stitch/hls/x/master.m3u8";
        </script></body></html>"#;

        let caps = SiteAnalyzer::detect_capabilities(html);
        assert_eq!(caps.app_version.as_deref(), Some("10.12.1096"));
        assert!(caps.api_endpoints.iter().any(|v| v.contains("/v4/start")));
        assert!(caps.stream_formats.contains(&"hls".to_string()));
        assert!(caps.player_cases.iter().any(|c| c == "session-bootstrap"));
        assert!(caps.player_cases.iter().any(|c| c == "hls-master"));
    }

    #[test]
    fn classifies_graphql_drm_and_ad_cases_from_js_payloads() {
        let html = r#"<html><body><script>
          const ext = {"tnPersistedDocumentHash":"abc"};
          fetch("https://site.example/api/tn/app-shell/graphql/");
          const license = "com.widevine.alpha";
          const filler = "plutotv_filler";
          const mpd = "https://cdn.example.com/stream.mpd";
        </script></body></html>"#;

        let caps = SiteAnalyzer::detect_capabilities(html);
        assert!(caps.player_cases.iter().any(|c| c == "graphql-persisted"));
        assert!(caps.player_cases.iter().any(|c| c == "drm-widevine"));
        assert!(caps.player_cases.iter().any(|c| c == "ads-hls"));
        assert!(caps.player_cases.iter().any(|c| c == "dash-mpd"));
    }

    #[test]
    fn detects_region_and_locale_from_hydrated_state() {
        let html = r#"<html><body><script id="__NEXT_DATA__" type="application/json">
          {"props":{"pageProps":{"dehydratedState":{"queries":[{"queryKey":["region"],
          "state":{"data":{"region":{"locale":"es-419","region":"MX"}}}}]}}}}}
        </script></body></html>"#;

        let caps = SiteAnalyzer::detect_capabilities(html);
        assert_eq!(caps.region.as_deref(), Some("MX"));
        assert_eq!(caps.locale.as_deref(), Some("es-419"));
    }

    #[test]
    fn detects_region_from_escaped_rsc_payload() {
        let html = r#"<script>self.__next_f.push([1,"{\"queryKey\":[\"region\"],\"state\":{\"data\":{\"region\":{\"locale\":\"es-419\",\"region\":\"MX\"}}}}"])</script>"#;

        let caps = SiteAnalyzer::detect_capabilities(html);
        assert_eq!(caps.region.as_deref(), Some("MX"));
        assert_eq!(caps.locale.as_deref(), Some("es-419"));
    }

    #[test]
    fn capabilities_do_not_claim_formats_that_are_not_in_the_html() {
        let caps = SiteAnalyzer::detect_capabilities("<html><body>hola</body></html>");
        assert!(caps.stream_formats.is_empty());
        assert!(caps.qualities.is_empty());
        assert!(caps.api_endpoints.is_empty());
        assert!(caps.region.is_none());
        assert!(caps.locale.is_none());
    }

    #[test]
    fn structured_data_gives_real_catalog_search_and_language() {
        let html = r#"<html lang="en"><head>
<script type="application/ld+json">{"@context":"https://schema.org","@type":"WebSite","inLanguage":"en","potentialAction":{"@type":"SearchAction","target":{"@type":"EntryPoint","urlTemplate":"https://frame.example/catalog?name={search_term_string}"},"query-input":"required name=search_term_string"}}</script>
<script type="application/ld+json">{"@context":"https://schema.org","@type":"ItemList","name":"Hafta Topi","itemListElement":[{"@type":"ListItem","position":1,"url":"https://frame.example/cinema/one","name":"Titulo uno","image":"https://st.frame.example/one.webp"},{"@type":"ListItem","position":2,"url":"https://frame.example/cinema/two","name":"Titulo dos","alternateName":"English two","inLanguage":"uz"}]}</script>
</head><body>
<div id="app"></div>
<a href="/catalog?genreIds=1">Accion</a>
<a href="/catalog?genreIds=2">Drama</a>
<a href="/catalog?name=xxx">Buscar</a>
<a href="/catalog?page=2">2</a>
</body></html>"#;
        let doc = Html::parse_document(html);
        let origin = "https://frame.example";
        let host = "frame.example";

        assert_eq!(SiteAnalyzer::detect_language(&doc), "en");
        let (records, action) = SiteAnalyzer::detect_structured_data(&doc, origin, host);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].title, "Titulo uno");
        assert_eq!(records[1].alternate_title.as_deref(), Some("English two"));
        assert!(records[0].image.as_deref().unwrap_or("").ends_with("one.webp"));
        let action = action.expect("SearchAction declarado");
        assert_eq!(action.query_param.as_deref(), Some("name"));
        assert_eq!(
            SiteAnalyzer::search_template_to_path(&action.url_template, origin, host),
            Some(("/catalog".to_string(), "name".to_string()))
        );

        let filters = SiteAnalyzer::detect_filter_params(&doc, origin, host, Some("name"));
        assert_eq!(filters.len(), 2, "sólo los valores reales de genreIds: {:?}", filters);
        assert!(filters.iter().all(|f| f.param == "genreIds"));
        assert!(filters.iter().all(|f| f.param != "page"));
    }

    #[test]
    fn search_template_from_another_host_is_not_used() {
        assert_eq!(
            SiteAnalyzer::search_template_to_path(
                "https://other.example/search?q={search_term_string}",
                "https://frame.example",
                "frame.example",
            ),
            None
        );
    }

    #[test]
    fn navigation_items_are_not_catalog_cards() {
        let html = r#"<!doctype html><html><body>
<header><nav class="header-nav">
  <div class="header-nav__item"><a href="/">Home</a></div>
  <div class="header-nav__item"><a href="/catalog">Catalog</a></div>
  <div class="header-nav__item"><a href="/mini-series">Mini series</a></div>
  <div class="header-nav__item"><a href="/tv">TV</a></div>
  <div class="header-nav__item"><a href="/tv-order">Buy a TV</a></div>
</nav></header>
<main><div class="swiper__movie"></div><div class="main__cards"></div></main>
<footer><div class="footer__item"><a href="/help">Help</a></div></footer>
</body></html>"#;
        let doc = Html::parse_document(html);
        let catalog = SiteAnalyzer::new().detect_catalog(&doc, "https://site.example", "site.example");
        assert_eq!(catalog.count, 0, "el menú no es un catálogo: {:?}", catalog.samples);
        assert!(catalog.card_selector.is_empty());
    }

    #[test]
    fn spa_payload_gives_locales_and_explicit_filters() {
        let html = r#"<!doctype html><html lang="uz"><head>
<script id="__NEXT_DATA__" type="application/json">{"locale":"uz","locales":["uz","ru","en"],"defaultLocale":"uz","props":{"filters":{"filterParams":[{"param":"sortBy","values":["publish_date","imdb_rating"]},{"param":"status","values":["announcement","release"]}]}},"messages":{"filter":{"publish_date":"Saralash","sort_by":"Saralash turi"}}}</script>
</head><body><div id="__next"></div></body></html>"#;
        let doc = Html::parse_document(html);
        assert_eq!(
            SiteAnalyzer::detect_site_locales(&doc),
            vec!["uz".to_string(), "ru".to_string(), "en".to_string()]
        );
        let filters = SiteAnalyzer::detect_payload_filters(&doc, None);
        assert_eq!(filters.len(), 4, "sólo los pares explícitos: {:?}", filters);
        assert!(filters.iter().all(|f| f.kind == "select"));
        assert!(filters.iter().any(|f| f.param == "sortBy" && f.value == "publish_date"));
        assert!(filters.iter().any(|f| f.param == "status" && f.value == "release"));
        assert!(filters.iter().all(|f| f.name.chars().all(|c| c.is_ascii())));
    }

    #[test]
    fn js_bundles_give_real_filter_params_and_values() {
        let js = r#"let nm=["year","countryIds","genreIds","audioLanguageIds","quality"],nh=[...nm,"categoryIds","CopyrightHolderIds","isFrameDub","name","sortBy","status"],nv="publish_date";
const H=(0,a.useMemo)(()=>[{value:"publish_date",label:S("filter.publish_date")},{value:"imdb_rating",label:S("filter.imdb_rating")},{value:"kinopoisk_rating",label:S("filter.kinopoisk_rating")}],[S]),$=(0,a.useMemo)(()=>[{value:"announcement",label:S("filter.announcement")},{value:"release",label:S("filter.release")}],[S]);
render(nf,{instanceId:"catalog-sort",options:H,placeholder:S("filter.sort")});
render(nf,{instanceId:"catalog-status",options:$,placeholder:S("filter.status")});"#;
        let filters = SiteAnalyzer::detect_filters_from_js(js, Some("name"));
        let params: Vec<&str> = filters.iter().map(|f| f.param.as_str()).collect();
        for expected in ["year", "countryIds", "genreIds", "audioLanguageIds", "quality", "categoryIds", "CopyrightHolderIds", "isFrameDub", "sortBy", "status"] {
            assert!(params.contains(&expected), "falta {expected} en {:?}", params);
        }
        assert!(!params.contains(&"name"), "el parámetro de búsqueda no es un filtro");
        assert!(!params.contains(&"publish_date"), "los valores no son parámetros");
        assert!(filters.iter().any(|f| f.param == "sortBy" && f.value == "imdb_rating"));
        assert!(filters.iter().any(|f| f.param == "status" && f.value == "announcement"));
        assert!(filters.iter().any(|f| f.param == "genreIds" && f.kind == "text"));
        assert!(filters.iter().all(|f| f.name.chars().all(|c| c.is_ascii())));
    }

    #[test]
    fn js_bundles_never_expose_credentials_or_internal_constants_as_filters() {
        let js = r#"let fields=["year","genreIds","userRegId","password","token","bearer","jwt","cookie","sessionId","apiKey","LS_ACCESS","DEFAULT_DATE_FORMAT","TEMP_DRAW_SUB_AMOUNT"],opts=[...fields,"status","sortBy"];
render(nf,{options:opts,instanceId:"catalog"});"#;
        let filters = SiteAnalyzer::detect_filters_from_js(js, Some("name"));
        let params: Vec<&str> = filters.iter().map(|f| f.param.as_str()).collect();
        for bogus in [
            "userRegId", "password", "token", "bearer", "jwt", "cookie", "sessionId",
            "apiKey", "LS_ACCESS", "DEFAULT_DATE_FORMAT", "TEMP_DRAW_SUB_AMOUNT",
        ] {
            assert!(!params.contains(&bogus), "parámetro interno expuesto como filtro: {bogus}");
        }
        assert!(params.contains(&"year"), "los filtros reales deben conservarse: {:?}", params);
        assert!(params.contains(&"genreIds"));
        assert!(params.contains(&"status"));
        assert!(params.contains(&"sortBy"));
    }

    #[test]
    fn js_bundles_reveal_rest_api_bases_and_resource_paths() {
        let js = r#"let apiVersion="v1";let f=axios.create({baseURL:"https://api.example.com/user/api/"});
genres:{getGenres:async()=>await client.get(apiVersion+"/genres")},
categories:{getCategories:async()=>await client.get("/categories")},
getHistory:async()=>await client.get(apiVersion+"/profile-histories")},
render(nf,{href:"/catalog?genreIds=1"});"#;
        let candidates = SiteAnalyzer::detect_api_candidates(js);
        assert_eq!(candidates.bases, vec!["https://api.example.com/user/api"]);
        assert_eq!(candidates.version.as_deref(), Some("v1"));
        assert!(candidates.paths.contains(&"/genres".to_string()), "{:?}", candidates.paths);
        assert!(candidates.paths.contains(&"/categories".to_string()), "{:?}", candidates.paths);
        assert!(!candidates.paths.iter().any(|p| p.contains("profile")), "las rutas privadas no son recursos: {:?}", candidates.paths);
        assert!(!candidates.paths.iter().any(|p| p == "/catalog"), "una ruta de UI fuera de .get() no es un recurso API: {:?}", candidates.paths);
    }

    #[test]
    fn api_lists_only_store_real_json_arrays_with_names() {
        let json: serde_json::Value = serde_json::from_str(
            r#"[{"id":1,"name":"Action"},{"id":4,"name":"Comedy"},{"id":9,"name":"Horror"}]"#,
        )
        .unwrap();
        let result = SiteAnalyzer::parse_api_list("https://api.example.com/user/api/v1/genres", &json).unwrap();
        assert_eq!(result.resource, "genres");
        assert_eq!(result.record_count, 3);
        assert_eq!(result.options.len(), 3);
        assert_eq!(result.options[1].value, "4");
        assert_eq!(result.options[1].label, "Comedy");
        assert_eq!(result.title_field, "name");
        assert!(result.sample_json.contains("Comedy"));

        let wrapped: serde_json::Value = serde_json::from_str(r#"{"data":[{"slug":" movies ","title":"Movie"}]}"#).unwrap();
        let wrapped = SiteAnalyzer::parse_api_list("https://api.example.com/v1/contents", &wrapped).unwrap();
        assert_eq!(wrapped.resource, "contents");
        assert_eq!(wrapped.options[0].value, "movies");
        assert_eq!(wrapped.title_field, "title");

        let not_a_list: serde_json::Value = serde_json::from_str(r#"{"error":"unauthorized"}"#).unwrap();
        assert!(SiteAnalyzer::parse_api_list("https://api.example.com/v1/genres", &not_a_list).is_none());
    }

    #[test]
    fn minified_bundle_version_is_resolved_from_its_variable() {
        let js = r#"41423,e=>{"use strict";e.s(["LS_ACCESS",()=>o,"apiVersion",()=>n]);let n="v1",r="id-r",o="ac-s";}"#;
        assert_eq!(SiteAnalyzer::detect_api_version(js).as_deref(), Some("v1"));
    }

    #[test]
    fn api_path_priority_puts_catalog_resources_first() {
        let genres = SiteAnalyzer::api_path_priority("/genres");
        let categories = SiteAnalyzer::api_path_priority("/categories");
        let other = SiteAnalyzer::api_path_priority("/banners");
        assert!(genres < other);
        assert!(categories < other);
        assert_eq!(genres, SiteAnalyzer::api_path_priority("genres"));
    }

    #[test]
    fn api_probes_stay_on_the_sites_host_family() {
        assert!(SiteAnalyzer::host_related_to("https://api.frame.uz/user/api/v1/genres", "frame.uz"));
        assert!(SiteAnalyzer::host_related_to("https://pluto.tv/api/list", "www.pluto.tv"));
        assert!(!SiteAnalyzer::host_related_to("https://third-party.example/genres", "frame.uz"));
    }
}
