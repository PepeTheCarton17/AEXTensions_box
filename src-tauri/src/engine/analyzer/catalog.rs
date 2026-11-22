use scraper::{ElementRef, Html, Selector};
use std::collections::{HashMap, HashSet};

use super::models::SiteAnalyzer;

impl SiteAnalyzer {

    pub(crate) fn first_card_href(doc: &Html, card_selector: &str, origin: &str, host: &str) -> Option<String> {
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

    /// Deduce un selector de episodios a partir de una ficha. Se agrupa por el
    /// segmento de ruta que delata un episodio; `watch` se queda fuera porque
    /// suele ser un menú, no una lista de episodios.
    pub(crate) fn detect_episode_selector(body: &str) -> Option<(String, usize)> {
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

    /// Coincidencia por token de clase, no por substring. Un contenedor como
    /// `stickyFooterLayout` incluye `footer` en el texto: buscar por substring
    /// descartaría todas sus tarjetas como navegación.
    fn matches_nav_class_token(hay: &str, tokens: &[&str]) -> bool {
        hay.split(|c: char| !c.is_alphanumeric())
            .any(|token| !token.is_empty() && tokens.iter().any(|t| token.eq_ignore_ascii_case(t)))
    }

    /// Descarta menús, cabeceras, pies, migas de pan, banners y paginación.
    /// Estos bloques usan las mismas clases (`item`, `card`, `link`) que las
    /// tarjetas reales y son la causa más frecuente de catálogos basura.
    pub(crate) fn is_navigation_element(el: ElementRef<'_>) -> bool {
        const NAV_TOKENS: &[&str] = &[
            "nav", "menu", "footer", "header", "breadcrumb", "sidebar", "dropdown",
            "promo", "banner", "pagination", "pager", "navbar", "toolbar",
        ];
        // En un ancestro, el token suelto `nav` es demasiado amplio: los
        // carruseles usan clases como `container--nav-ready` y eso no convierte
        // sus tarjetas en navegación. Sólo los tokens explícitamente de
        // navegación pueden descartar un elemento por su contenedor.
        const ANCESTOR_NAV_TOKENS: &[&str] = &[
            "menu", "footer", "header", "breadcrumb", "sidebar", "dropdown",
            "promo", "banner", "pagination", "pager", "navbar", "toolbar",
        ];
        let own = format!(
            "{} {}",
            el.value().attr("class").unwrap_or(""),
            el.value().attr("id").unwrap_or("")
        )
        .to_lowercase();
        if Self::matches_nav_class_token(&own, NAV_TOKENS) {
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
                if Self::matches_nav_class_token(&hay, ANCESTOR_NAV_TOKENS) {
                    return true;
                }
            }
            current = parent.parent();
        }
        false
    }

    pub(crate) fn detect_catalog(&self, doc: &Html, origin: &str, host: &str) -> CatalogInfo {
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
    pub(crate) fn detect_repeated_links(&self, doc: &Html, origin: &str, host: &str) -> Option<CatalogInfo> {
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
}

pub(crate) struct CatalogInfo {
    pub(crate) card_selector: String,
    pub(crate) title_selector: String,
    pub(crate) next_page_selector: String,
    pub(crate) page_style: String,
    pub(crate) count: usize,
    pub(crate) samples: Vec<String>,
}
