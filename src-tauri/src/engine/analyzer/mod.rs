use scraper::Html;
use std::collections::HashSet;
use url::Url;

mod api;
mod capabilities;
mod catalog;
mod filters;
mod filters_form;
mod filters_js;
mod filters_paths;
mod identity;
mod models;
mod playback;
mod protections;
mod scripts;
mod search;
mod selectors;
mod structured_data;

#[cfg(test)]
mod tests;

pub use models::{
    AnalysisResult, ApiListResult, ApiOption, CatalogRecord, SiteAnalyzer, SiteCapabilities,
    SiteFilterParam, SitePathFilter, SiteSearchAction,
};
pub(crate) use models::ApiCandidates;

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
                    "Filtros encontrados en los bundles JS del sitio: {}. Sus etiquetas se muestran en el idioma de la extensión; los valores no enumerados se ofrecen como texto libre.",
                    added.join(", ")
                ));
            }
            Self::normalize_filter_params(&mut result.filter_params);
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
                // Si el propio JSON declara acceso restringido, decirlo es más
                // honesto que generar un reproductor que siempre dará error.
                let gated = result.api_lists.iter().any(|api| {
                    let Ok(serde_json::Value::Array(records)) =
                        serde_json::from_str::<serde_json::Value>(&api.sample_json)
                    else {
                        return false;
                    };
                    records.iter().any(|record| {
                        let availability = record
                            .get("availability")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_lowercase();
                        availability.contains("subscription")
                            || availability.contains("premium")
                            || record.get("has_access").and_then(|v| v.as_bool()) == Some(false)
                    })
                });
                if gated {
                    result.warnings.push(
                        "El catálogo declara títulos de suscripción o sin acceso público. AEXT no puede verificar esos streams sin sesión: no genera pistas SUB/DUB ni servidores inventados.".to_string(),
                    );
                }
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
                if let Some(url) = Self::direct_master_url(&probe.html) {
                    sample_hints.push(url);
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
                "Se detectó cifrado/ofuscación de URLs de reproducción (firmas de cliente / n-parameter). AEXT no genera ese esquema por sí solo: hace falta un motor instalable que lo implemente.".to_string(),
            );
        }
        if result.catalog_source == "none" || !result.catalog_detected {
            result.warnings.push(
                "Sin catálogo HTML verificado: AEXT no inventa rutas ni tarjetas. El sitio necesita un motor que devuelva contenido real; la extensión generada no se considera funcional hasta que un motor produzca resultados.".to_string(),
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
        // páginas que no son catálogo, como una SPA sin tarjetas.
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
        let protections = protections::detect_protections(&body, &headers);

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
        let mut path_filters = Self::detect_path_filters(&doc, &origin, &host);
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
                // El footer puede declarar categorías aunque la portada no
                // tenga JSON-LD: no las condicionamos a que haya tarjetas.
                for f in Self::detect_path_filters(&root_doc, &origin, &host) {
                    if !path_filters.iter().any(|old| old.base_path == f.base_path && old.segment == f.segment) {
                        path_filters.push(f);
                    }
                }
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
        Self::normalize_filter_params(&mut filter_params);

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
        // Etiquetas reales de las tarjetas: sin esto la lista negra sólo podría
        // ocultar por título, nunca por género/categoría del catálogo.
        let genre_selector = if catalog.count >= 4 {
            Self::infer_genre_selector(&doc, &card_selector)
        } else {
            String::new()
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
        if !path_filters.is_empty() {
            warnings.push(format!(
                "Categorías declaradas como rutas del sitio: {}. Se ofrecerán como filtros que cambian la URL del catálogo.",
                path_filters
                    .iter()
                    .map(|g| format!("{} ({})", g.segment, g.base_path))
                    .collect::<Vec<_>>()
                    .join(", ")
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
                genre_selector,
                next_page_selector,
                catalog_source,
                catalog_records: ld_records,
                search_action: ld_search,
                filter_params,
                path_filters,
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
                engine_trials: Vec::new(),
            },
            demo_href,
        ))
    }

}
