use regex::Regex;
use std::collections::HashMap;

use super::filters::CANONICAL_FILTER_NAMES;
use super::models::{SiteAnalyzer, SiteFilterParam};

impl SiteAnalyzer {

    /// Parámetros de filtro declarados en bundles JS del mismo origen. No se
    /// adivina nada: sólo se aceptan Arrays de identificadores con nombres
    /// reconocibles de parámetro (year, genreIds, sortBy...) y pares
    /// `value/label` reales del propio bundle para enumerar los valores.
    pub(crate) fn detect_filters_from_js(text: &str, search_param: Option<&str>) -> Vec<SiteFilterParam> {
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
                .filter(|t| Self::is_filter_like_token(t, search_param))
                .count();
            if hits < 2 {
                continue;
            }
            // El array puede mezclar filtros reales con claves de configuración
            // (tiendas, esquemas, timeouts, eventos...). Un `hits >= 2` sólo
            // demuestra que el array parece una lista de filtros; NO autoriza a
            // publicar todos sus tokens. Se acepta cada token por separado.
            for token in &tokens {
                if Self::is_filter_like_token(token, search_param) {
                    add_param(token);
                }
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

        // Los SPAs no declaran `<select>`: construyen la URL del catálogo con
        // query strings (`/catalog?genreIds=1`) y con helpers tipo
        // `paramsToQuery({categoryIds: ...})`. Son evidencia del parámetro real
        // que acepta el sitio, así que se añaden aunque no haya arrays de
        // opciones: el generador los ofrecerá como texto libre.
        for (param, value) in Self::detect_query_string_params(text, search_param) {
            Self::push_js_param(&mut result, &param, &value, search_param);
        }
        for param in Self::detect_param_object_keys(text, search_param) {
            Self::push_js_param(&mut result, &param, "", search_param);
        }
        result
    }

    /// Añade un parámetro detectado en un bundle sin duplicar la combinación
    /// exacta `param/valor`. Un filtro `text` existente gana sobre uno `select`
    /// del mismo parámetro para no estrechar un campo libre sin evidencia.
    fn push_js_param(
        out: &mut Vec<SiteFilterParam>,
        param: &str,
        value: &str,
        search_param: Option<&str>,
    ) {
        if Self::ignored_filter_param(param, search_param) {
            return;
        }
        if out.iter().any(|old| {
            old.param == param && (old.value == value || (old.kind == "text" && value.is_empty()))
        }) {
            return;
        }
        out.push(SiteFilterParam {
            name: Self::canonical_filter_name(param),
            param: param.to_string(),
            value: value.to_string(),
            kind: if value.is_empty() { "text" } else { "select" }.to_string(),
        });
    }

    /// Parámetros de filtro presentes en URLs literales del bundle. Sólo se
    /// aceptan nombres reconocibles (`genreIds`, `year`, `sortBy`...) y valores
    /// simples: un id de sesión o una ruta con puntos no es una opción real.
    pub(crate) fn detect_query_string_params(
        text: &str,
        search_param: Option<&str>,
    ) -> Vec<(String, String)> {
        let Ok(re) = Regex::new(r#"[?&]([A-Za-z][A-Za-z0-9_]{1,39})=([^&"'\s\\]{0,40})"#) else {
            return Vec::new();
        };
        let mut out: Vec<(String, String)> = Vec::new();
        for cap in re.captures_iter(text) {
            let param = cap[1].trim();
            let lower = param.to_lowercase();
            // `?id=` es un identificador de ficha, no un filtro de catálogo.
            let recognised = CANONICAL_FILTER_NAMES
                .iter()
                .any(|(key, label)| *key == lower.as_str() && *label != "Search")
                || lower.ends_with("ids")
                || lower.ends_with("_id")
                || lower.ends_with("by");
            if !recognised || !Self::is_filter_like_token(param, search_param) {
                continue;
            }
            let raw = cap[2]
                .trim()
                .trim_end_matches(|c| matches!(c, ',' | ')' | '}'));
            let value = if Self::plain_js_value(raw) {
                raw.to_string()
            } else {
                String::new()
            };
            if !out.iter().any(|(p, v)| p == param && v == &value) {
                out.push((param.to_string(), value));
            }
        }
        out
    }

    /// Claves de filtro pasadas a un constructor de query strings del propio
    /// sitio (`paramsToQuery`, `buildQuery`, `toQueryString`). El valor se
    /// calcula en tiempo de ejecución, así que sólo se guarda la clave.
    pub(crate) fn detect_param_object_keys(
        text: &str,
        search_param: Option<&str>,
    ) -> Vec<String> {
        let Ok(call_re) = Regex::new(
            r#"(?i)\b(?:paramsToQuery|buildQuery|toQueryString|queryParams)\s*\(\s*\{([^{}]{1,400})\}\s*\)"#,
        ) else {
            return Vec::new();
        };
        let Ok(key_re) = Regex::new(r#"([A-Za-z][A-Za-z0-9_]{1,39})\s*:"#) else {
            return Vec::new();
        };
        let mut out: Vec<String> = Vec::new();
        for call in call_re.captures_iter(text) {
            for key in key_re.captures_iter(&call[1]) {
                let param = key[1].trim();
                if Self::is_filter_like_token(param, search_param)
                    && !out.iter().any(|p| p == param)
                {
                    out.push(param.to_string());
                }
            }
        }
        out
    }

    /// Un literal simple y completo: ids numéricos, slugs y enumeraciones.
    /// Rechaza plantillas, puntos (identificadores compuestos) y comandos JS.
    fn plain_js_value(raw: &str) -> bool {
        !raw.is_empty()
            && raw.len() <= 30
            && raw
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            && !raw.chars().all(|c| c.is_ascii_digit() && c == '0')
    }

    pub(crate) fn js_array_tokens(
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

    /// Un token sólo es un filtro de catálogo si su nombre es reconocible como
    /// parámetro (year, genreIds, sortBy, status...) y no es un dato interno.
    /// Evita que un array de configuración se convierta en campos raros como
    /// "Appdeeplinkscheme" o "Authenticationqrcooldown" en la extensión.
    pub(crate) fn is_filter_like_token(param: &str, search_param: Option<&str>) -> bool {
        if Self::ignored_filter_param(param, search_param) {
            return false;
        }
        let lower = param.to_lowercase();
        // Un sufijo `Id` no convierte una clave interna en filtro: los bundles
        // mezclan `clickMerchantId`, `paymeMerchantId`, etc. con parámetros
        // reales. El contexto (array de opciones) no basta para publicarlos.
        const CONFIG_LIKE_SUBSTRINGS: &[&str] = &[
            "merchant", "service", "scheme", "deeplink", "cooldown", "expire",
            "gtm", "modal", "icon", "render", "parse", "resolve", "appstore",
            "googleplay", "siteurl", "cardtype", "feature", "event",
            // Los helpers `paramsToQuery` mezclan filtros con claves de sesión,
            // perfil, notificaciones y pago. Ninguna es un filtro de catálogo.
            "profile", "request", "device", "token", "user", "avatar",
            "history", "favourite", "favorite", "wallet", "promo",
            "subscription", "notification", "comment", "person", "franchise",
            "banner", "setting", "payment", "billing", "region", "locale",
        ];
        if CONFIG_LIKE_SUBSTRINGS.iter().any(|key| lower.contains(key)) {
            return false;
        }
        if CANONICAL_FILTER_NAMES.iter().any(|(key, _)| *key == lower.as_str()) {
            return true;
        }
        lower.ends_with("id") || lower.ends_with("ids") || lower.ends_with("by")
    }
}
