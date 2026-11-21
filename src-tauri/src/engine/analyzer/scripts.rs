use regex::Regex;
use scraper::{Html, Selector};
use url::Url;

use super::models::{ApiCandidates, SiteAnalyzer, SiteCapabilities, SiteFilterParam};

impl SiteAnalyzer {

    /// Extrae scripts del mismo origen. Los externos se descartan: no podremos
    /// distinguir un endpoint del sitio del de un proveedor de analítica.
    pub(crate) fn script_urls(body: &str, origin: &str, limit: usize) -> Vec<String> {
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
    pub(crate) async fn scan_scripts(
        &self,
        entry_url: &str,
        search_param: Option<&str>,
    ) -> Option<(SiteCapabilities, Vec<SiteFilterParam>, ApiCandidates)> {
        let html = self.client.get(entry_url).send().await.ok()?.text().await.ok()?;
        let origin = Url::parse(entry_url).ok()?.origin().ascii_serialization();
        // Los primeros bundles suelen ser framework y analítica; en Next.js
        // las rutas de filtros y los parámetros del catálogo viven en chunks
        // posteriores. Se amplía el escaneo y se para en cuanto hay evidencia.
        let urls = Self::script_urls(&html, &origin, 24);
        if urls.is_empty() {
            return None;
        }

        let mut aggregate = SiteCapabilities::default();
        let mut filters: Vec<SiteFilterParam> = Vec::new();
        let mut api_candidates = ApiCandidates::default();
        let mut found_any = false;
        let mut checked = 0usize;
        for url in urls {
            checked += 1;
            if let Ok(resp) = self
                .client
                .get(&url)
                .timeout(std::time::Duration::from_secs(8))
                .send()
                .await
            {
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
            // Con filtros y recursos `/filters/*` ya verificados, seguir
            // descargando bundles sólo añade latencia sin cambiar el resultado.
            let distinct_params = filters
                .iter()
                .map(|f| f.param.as_str())
                .collect::<std::collections::HashSet<_>>()
                .len();
            let filter_paths = api_candidates
                .paths
                .iter()
                .filter(|p| p.starts_with("/filters/"))
                .count();
            if checked >= 8 && distinct_params >= 2 && filter_paths >= 2 {
                break;
            }
        }
        if !found_any {
            return None;
        }
        Some((aggregate, filters, api_candidates))
    }

    /// Extrae bases de API REST y rutas de recursos citadas en un bundle JS.
    /// Sólo acepta URLs http(s) como base y rutas que aparecen en llamadas
    /// `.get(...)` o concatenadas a `apiVersion`, para no confundir rutas de la
    /// interfaz (`/catalog`) con recursos de datos (`/genres`).
    pub(crate) fn detect_api_candidates(text: &str) -> ApiCandidates {
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
    pub(crate) fn detect_api_version(text: &str) -> Option<String> {
        // Un bundle minificado puede tener decenas de variables de un carácter;
        // sólo aceptar algo que parezca versión evita usar `n="function"` como
        // si fuera `v1` y sondear cientos de URLs inexistentes.
        fn looks_like_version(value: &str) -> bool {
            let v = value.trim().trim_start_matches('v').trim_start_matches('V');
            !v.is_empty()
                && v.split('.').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
        }
        for pattern in [
            r#"(?i)apiVersion\s*[:=]\s*["']([^"']{1,20})["']"#,
            r#"(?i)["']apiVersion["']\s*,\s*\(\)\s*=>\s*["']([^"']{1,20})["']"#,
        ] {
            if let Ok(re) = Regex::new(pattern) {
                for cap in re.captures_iter(text) {
                    let version = cap[1].trim();
                    if looks_like_version(version) {
                        return Some(version.to_string());
                    }
                }
            }
        }
        // Turbopack exporta `apiVersion,()=>n`; resolver la variable local.
        if let Ok(re) = Regex::new(r#"["']?apiVersion["']?\s*,\s*\(\)\s*=>\s*([A-Za-z_$][\w$]*)"#) {
            if let Some(cap) = re.captures(text) {
                let variable = regex::escape(&cap[1]);
                let pattern = format!(r#"\b{}\s*=\s*["']([^"']{{1,20}})["']"#, variable);
                if let Ok(vre) = Regex::new(&pattern) {
                    for value in vre.captures_iter(text) {
                        if looks_like_version(&value[1]) {
                            return Some(value[1].to_string());
                        }
                    }
                }
            }
        }
        None
    }
}
