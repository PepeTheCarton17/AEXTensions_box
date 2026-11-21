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
        let paired_options: Vec<ApiOption> = arr
            .iter()
            .filter_map(|v| {
                let obj = v.as_object()?;
                let value = Self::json_scalar(obj, &["id", "value", "key", "slug"])?;
                let label = Self::json_scalar(obj, &["name", "label", "title"])?;
                Some(ApiOption { value, label })
            })
            .collect();
        // Muchas listas de filtros no traen `{id,name}` sino un único campo
        // escalar: `[{"year":2026}]`, `[{"quality":"Sd"}]`, `[{"company_name":"..."}]`.
        // Ese campo es a la vez valor y etiqueta: es un dato real del sitio.
        let scalar_options: Vec<ApiOption> = arr
            .iter()
            .filter_map(Self::scalar_api_option)
            .collect();
        // Nunca se sustituye un par `id/nombre` válido por el fallback: una
        // lista de un solo registro con `slug/title` sigue siendo una opción.
        let options = if scalar_options.len() > paired_options.len() {
            scalar_options
        } else {
            paired_options
        };
        let first = arr[0].as_object()?;
        let mut title_field = Self::detect_json_field(first, &["title", "name", "original_title", "originalName"]);
        if title_field.is_empty() && options.len() >= 2 {
            title_field = first
                .keys()
                .find(|key| {
                    !matches!(
                        key.as_str(),
                        "id" | "value" | "key" | "slug" | "sort_order"
                            | "is_popular" | "is_mini" | "year" | "quality"
                    )
                })
                .cloned()
                .unwrap_or_default();
        }
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

    /// Opción de una lista cuyo registro es un único campo escalar. Se ignoran
    /// metadatos y claves de presentación para no confundir un `sort_order` con
    /// un valor de filtro.
    fn scalar_api_option(value: &serde_json::Value) -> Option<ApiOption> {
        const IGNORED: &[&str] = &[
            "id", "value", "key", "slug", "name", "label", "title", "sort_order",
            "sortorder", "is_popular", "ispopular", "is_mini", "ismini", "active",
            "created_at", "updated_at",
        ];
        let obj = value.as_object()?;
        let mut fields: Vec<(&String, String)> = obj
            .iter()
            .filter(|(key, _)| !IGNORED.contains(&key.to_lowercase().as_str()))
            .filter_map(|(key, value)| {
                let text = match value {
                    serde_json::Value::String(s) => s.trim().to_string(),
                    serde_json::Value::Number(n) => n.to_string(),
                    _ => return None,
                };
                if text.is_empty() {
                    None
                } else {
                    Some((key, text))
                }
            })
            .collect();
        if fields.len() != 1 {
            return None;
        }
        let (_, text) = fields.remove(0);
        Some(ApiOption {
            value: text.clone(),
            label: text,
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
        // Los endpoints `/filters/*` alimentan directamente los selectores:
        // van antes que catálogos y mucho antes que banners o publicidad.
        if low == "filters" || low.starts_with("filters/") {
            return 0;
        }
        PREFERRED
            .iter()
            .position(|name| low == *name || low.starts_with(&format!("{}s", name)))
            .map(|index| index as u8 + 1)
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
        let mut bases: Vec<String> = candidates.bases.clone();
        // Un sitio puede declarar `auth/api` y `user/api`; sondear primero la
        // de autenticación agota el presupuesto con rutas privadas que nunca
        // devuelven el catálogo público.
        bases.sort_by_key(|b| if b.to_lowercase().contains("/auth") { 1 } else { 0 });
        let mut urls: Vec<String> = Vec::new();
        for base in &bases {
            let root = base.trim_end_matches('/');
            for path in ordered {
                // Se prueba la ruta con la versión declarada y, sólo después,
                // sin ella. Así cada base tiene una oportunidad real antes de
                // gastar el presupuesto en fallbacks.
                if let Some(v) = version {
                    let full = format!("{}/{}{}", root, v, path);
                    if !urls.contains(&full) {
                        urls.push(full);
                    }
                }
                let plain = format!("{}{}", root, path);
                if !urls.contains(&plain) {
                    urls.push(plain);
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
        for url in urls.into_iter().take(32) {
            if !Self::host_related_to(&url, site_host) {
                continue;
            }
            let Ok(response) = self
                .client
                .get(&url)
                .header("Accept", "application/json")
                .header("Accept-Language", "en")
                .timeout(std::time::Duration::from_secs(5))
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
