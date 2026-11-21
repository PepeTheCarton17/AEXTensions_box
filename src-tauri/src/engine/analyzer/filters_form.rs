use scraper::{Html, Selector};
use std::collections::HashSet;
use url::Url;

use super::models::{SiteAnalyzer, SiteFilterParam};

impl SiteAnalyzer {
    /// Filtros declarados en enlaces y controles de formulario del HTML.
    /// Un `<select>` es la forma más clara de filtro que puede publicar un
    /// sitio: su `name` es el parámetro y cada `option` es un valor real.
    pub(crate) fn detect_filter_params(
        doc: &Html,
        origin: &str,
        host: &str,
        search_param: Option<&str>,
    ) -> Vec<SiteFilterParam> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();
        let mut add = |param: &str, value: &str, kind: &str| {
            if param.is_empty() || Self::ignored_filter_param(param, search_param) {
                return;
            }
            // Un parámetro de filtro sin valor también es evidencia: muchos
            // sitios enlazan `?genreIds=` y rellenan el valor en el cliente.
            if value.is_empty() && !Self::is_declared_filter_param(param) {
                return;
            }
            let key = format!("{param}\u{1}{value}\u{1}{kind}");
            if seen.insert(key) && result.len() < 80 {
                result.push(SiteFilterParam {
                    name: Self::canonical_filter_name(param),
                    param: param.to_string(),
                    value: value.to_string(),
                    kind: kind.to_string(),
                });
            }
        };

        if let Ok(sel) = Selector::parse("a[href]") {
            for a in doc.select(&sel) {
                let href = a.value().attr("href").unwrap_or("");
                let Some(path) = Self::same_origin_path(href, origin, host) else { continue };
                let Ok(url) = Url::parse(&format!("{origin}{path}")) else { continue };
                for (key, value) in url.query_pairs() {
                    let kind = if value.is_empty() { "text" } else { "check" };
                    add(key.as_ref(), value.as_ref(), kind);
                }
            }
        }

        if let Ok(sel) = Selector::parse("form input[name]") {
            for input in doc.select(&sel) {
                let key = input.value().attr("name").unwrap_or("");
                let value = input.value().attr("value").unwrap_or("").trim();
                let kind = if value.is_empty() { "text" } else { "check" };
                add(key, value, kind);
            }
        }

        if let Ok(sel) = Selector::parse("select[name]") {
            for select in doc.select(&sel) {
                let key = select.value().attr("name").unwrap_or("");
                if Self::ignored_filter_param(key, search_param) {
                    continue;
                }
                let mut options = 0;
                if let Ok(option_sel) = Selector::parse("option") {
                    for option in select.select(&option_sel) {
                        let value = option
                            .value()
                            .attr("value")
                            .map(str::trim)
                            .filter(|v| !v.is_empty())
                            .map(str::to_string)
                            .unwrap_or_else(|| option.text().collect::<String>().trim().to_string());
                        if value.is_empty() {
                            continue;
                        }
                        add(key, &value, "select");
                        options += 1;
                    }
                }
                if options == 0 && Self::is_declared_filter_param(key) {
                    add(key, "", "text");
                }
            }
        }

        result
    }
}
