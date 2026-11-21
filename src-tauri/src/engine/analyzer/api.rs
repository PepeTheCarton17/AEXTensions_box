use url::Url;

use super::models::{ApiCandidates, ApiListResult, ApiOption, SiteAnalyzer};

impl SiteAnalyzer {

    /// Convierte un JSON cualquiera en un `ApiListResult` cuando hay evidencia
    /// de una lista de registros. Sin array de objetos, no se guarda nada.
    pub(crate) fn parse_api_list(url: &str, json: &serde_json::Value) -> Option<ApiListResult> {
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
        // Hasta 30 registros: suficientes para poblar un catálogo real sin
        // convertir el análisis en un volcado infinito de la API del sitio.
        let sample_json = serde_json::to_string(&arr[..arr.len().min(30)]).unwrap_or_default();
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

    pub(crate) fn json_scalar(obj: &serde_json::Map<String, serde_json::Value>, keys: &[&str]) -> Option<String> {
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

    pub(crate) fn detect_json_field(obj: &serde_json::Map<String, serde_json::Value>, candidates: &[&str]) -> String {
        for key in candidates {
            if obj.contains_key(*key) {
                return (*key).to_string();
            }
        }
        String::new()
    }

    /// Sólo se sondean hosts del propio sitio o de su API (p. ej. `frame.uz` y
    /// `api.frame.uz`). Evita convertir al analizador en un proxy de terceros.
    pub(crate) fn host_related_to(url: &str, site_host: &str) -> bool {
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
    pub(crate) fn api_path_priority(path: &str) -> u8 {
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

    /// Ordena las URLs a sondear: primero las rutas con más probabilidad de ser
    /// catálogo/filtros y, dentro de cada ruta, todas las bases declaradas. Un
    /// bundle puede crear varios clientes (`/auth/api`, `/user/api`...); darle
    /// todas las peticiones a la primera base dejaría sin probar la útil.
    pub(crate) fn api_probe_urls(candidates: &ApiCandidates, ordered: &[String]) -> Vec<String> {
        let version = candidates
            .version
            .as_deref()
            .map(|v| v.trim().trim_start_matches('/').trim_end_matches('/'))
            .filter(|v| !v.is_empty());
        let mut urls: Vec<String> = Vec::new();
        for with_version in [true, false] {
            if with_version && version.is_none() {
                continue;
            }
            for path in ordered {
                for base in &candidates.bases {
                    let root = base.trim_end_matches('/');
                    let full = match (with_version, version) {
                        (true, Some(v)) => format!("{}/{}{}", root, v, path),
                        _ => format!("{}{}", root, path),
                    };
                    if !urls.contains(&full) {
                        urls.push(full);
                    }
                }
            }
        }
        urls
    }

    /// Sondea de forma genérica los recursos detectados. Nunca envía credenciales
    /// ni cookies: si el endpoint exige sesión, no responde JSON y se descarta.
    pub(crate) async fn probe_api_lists(
        &self,
        candidates: &ApiCandidates,
        site_host: &str,
    ) -> Vec<ApiListResult> {
        let mut ordered: Vec<String> = candidates.paths.to_vec();
        ordered.sort_by_key(|path| Self::api_path_priority(path));
        let urls = Self::api_probe_urls(candidates, &ordered);
        let mut out = Vec::new();
        for url in urls.into_iter().take(12) {
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
}
