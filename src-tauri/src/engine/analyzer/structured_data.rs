use regex::Regex;
use scraper::{Html, Selector};
use std::collections::HashSet;
use url::Url;

use super::models::{CatalogRecord, SiteAnalyzer, SiteSearchAction};

impl SiteAnalyzer {

    pub(crate) fn normalize_locale(raw: &str) -> Option<String> {
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
    pub(crate) fn detect_site_locales(doc: &Html) -> Vec<String> {
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
        // Muchos sitios multilinguales no declaran sus idiomas en metadatos:
        // sólo los exponen en el conmutador de la interfaz (hreflang o un
        // parámetro lang/locale en los enlaces). Es evidencia real del sitio.
        Self::collect_locale_links(doc, &mut out);
        out
    }

    fn collect_locale_links(doc: &Html, out: &mut Vec<String>) {
        let Ok(param) = Regex::new(
            r"(?:[?&;])(?:[A-Za-z_][\w.-]*\.)?(?:lang|language|locale|hl|ui|mui|l)[=:]([A-Za-z][A-Za-z_-]{1,15})"
        ) else { return };
        let Ok(path) = Regex::new(
            r#"/(?:lang|locale|language)/([A-Za-z][A-Za-z_-]{1,15})(?:[/"'?#]|$)"#
        ) else { return };
        let Ok(sel) = Selector::parse("[href], [data-url], [hreflang], [data-lang]") else { return };

        for el in doc.select(&sel) {
            for key in ["hreflang", "data-lang"] {
                if let Some(value) = el.value().attr(key) {
                    Self::push_locale(out, value);
                }
            }
            for key in ["href", "data-url"] {
                let Some(raw) = el.value().attr(key) else { continue };
                for found in param.captures_iter(raw) {
                    Self::push_locale(out, &found[1]);
                }
                for found in path.captures_iter(raw) {
                    Self::push_locale(out, &found[1]);
                }
            }
        }
    }

    pub(crate) fn push_locale(out: &mut Vec<String>, raw: &str) {
        if let Some(locale) = Self::normalize_locale(raw) {
            if matches!(locale.as_str(), "all" | "auto" | "default" | "none" | "www") {
                return;
            }
            if !out.contains(&locale) {
                out.push(locale);
            }
        }
    }

    /// Recorre un payload hidratado buscando `locale`, `defaultLocale`,
    /// `locales` e `inLanguage`. Los idiomas individuales se insertan antes
    /// que la lista para que el primero sea el idioma principal del sitio.
    pub(crate) fn collect_locale_values(value: &serde_json::Value, out: &mut Vec<String>) {
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

    /// Idioma declarado por la propia página. Es una pista para elegir títulos,
    /// nunca una traducción: si no hay texto alternativo se usa el que haya.
    pub(crate) fn detect_language(doc: &Html) -> String {
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

    pub(crate) fn ld_string(value: Option<&serde_json::Value>) -> Option<String> {
        value
            .and_then(|v| if v.is_string() { v.as_str() } else { v.get("name").and_then(|n| n.as_str()) })
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub(crate) fn ld_types(node: &serde_json::Value) -> Vec<String> {
        let Some(t) = node.get("@type") else { return Vec::new() };
        match t {
            serde_json::Value::String(s) => vec![s.to_lowercase()],
            serde_json::Value::Array(a) => a.iter().filter_map(|v| v.as_str()).map(|s| s.to_lowercase()).collect(),
            _ => Vec::new(),
        }
    }

    pub(crate) fn collect_ld_nodes<'a>(value: &'a serde_json::Value, out: &mut Vec<&'a serde_json::Value>) {
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

    pub(crate) fn ld_search_action(node: &serde_json::Value) -> Option<SiteSearchAction> {
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
    pub(crate) fn detect_structured_data(
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
    pub(crate) fn search_template_to_path(template: &str, _origin: &str, _host: &str) -> Option<(String, String)> {
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
}
