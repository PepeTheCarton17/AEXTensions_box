use scraper::{Html, Selector};

use super::catalog::CatalogInfo;
use super::models::SiteAnalyzer;

impl SiteAnalyzer {

    pub(crate) fn detect_section_paths(
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

    pub(crate) fn detect_search(
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
}
