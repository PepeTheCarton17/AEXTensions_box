use scraper::{Html, Selector};
use std::collections::HashSet;
use url::Url;

use super::models::{SiteAnalyzer, SiteFilterParam};

/// Equivalencia entre el nombre real del parámetro del sitio y la etiqueta
/// canónica que muestran las extensiones de la comunidad (Genre, Year, ...).
/// Se usa tanto para nombrar el filtro como para decidir qué tokens de un
/// bundle son filtros de catálogo y cuáles son simple configuración interna.
pub(crate) const CANONICAL_FILTER_NAMES: &[(&str, &str)] = &[
    ("year", "Year"),
    ("years", "Year"),
    ("releaseyear", "Year"),
    ("country", "Country"),
    ("countryid", "Country"),
    ("countryids", "Country"),
    ("countries", "Country"),
    ("genre", "Genre"),
    ("genres", "Genre"),
    ("genreid", "Genre"),
    ("genreids", "Genre"),
    ("category", "Category"),
    ("categories", "Category"),
    ("categoryid", "Category"),
    ("categoryids", "Category"),
    ("audio", "Audio Language"),
    ("audiolanguage", "Audio Language"),
    ("audiolanguageid", "Audio Language"),
    ("audiolanguageids", "Audio Language"),
    ("audio_language", "Audio Language"),
    ("quality", "Quality"),
    ("qualities", "Quality"),
    ("format", "Format"),
    ("formats", "Format"),
    ("season", "Season"),
    ("seasons", "Season"),
    ("copyrightholderids", "Copyright Holder"),
    ("copyrightholder", "Copyright Holder"),
    ("isframedub", "Dubbed"),
    ("dub", "Dubbed"),
    ("dubbed", "Dubbed"),
    ("isdub", "Dubbed"),
    ("sortby", "Sort By"),
    ("sort_by", "Sort By"),
    ("sorting", "Sort By"),
    ("status", "Status"),
    ("cues", "Subtitles"),
    ("subtitle", "Subtitles"),
    ("subtitles", "Subtitles"),
    ("sub_language", "Subtitles"),
    ("display", "Display"),
    ("view", "Display"),
    ("name", "Search"),
    ("q", "Search"),
    ("query", "Search"),
    ("search", "Search"),
];

impl SiteAnalyzer {
    /// Filtros declarados en el estado hidratado de una SPA. Sólo se aceptan
    /// estructuras explícitas (`param`/`value`/`values`); un diccionario de
    /// traducciones bajo `filter` no es una lista de parámetros.
    pub(crate) fn detect_payload_filters(doc: &Html, search_param: Option<&str>) -> Vec<SiteFilterParam> {
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

    pub(crate) fn collect_payload_filters(
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

    pub(crate) fn parse_param_definition(
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

    pub(crate) fn ignored_filter_param(key: &str, search_param: Option<&str>) -> bool {
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
    pub(crate) fn canonical_filter_name(param: &str) -> String {
        let key = param.to_lowercase();
        if let Some((_, label)) = CANONICAL_FILTER_NAMES.iter().find(|(name, _)| *name == key.as_str()) {
            return (*label).to_string();
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
    pub(crate) fn detect_filter_params(
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
}
