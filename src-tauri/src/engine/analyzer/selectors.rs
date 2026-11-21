use scraper::{ElementRef, Html, Selector};
use url::Url;

use super::models::SiteAnalyzer;

impl SiteAnalyzer {

    pub(crate) fn same_origin_path(href: &str, origin: &str, host: &str) -> Option<String> {
        let h = href.trim();
        if h.is_empty()
            || h.starts_with('#')
            || h.starts_with("javascript:")
            || h.starts_with("mailto:")
            || h.starts_with("tel:")
            || h.starts_with("data:")
        {
            return None;
        }

        let u = if h.starts_with("//") {
            Url::parse(&format!("https:{}", h)).ok()?
        } else if h.starts_with('/') {
            Url::parse(&format!("{}{}", origin, h)).ok()?
        } else if h.starts_with("http://") || h.starts_with("https://") {
            Url::parse(h).ok()?
        } else {
            // Ruta relativa: se resuelve contra el origen.
            Url::parse(&format!("{}/{}", origin, h)).ok()?
        };

        if u.host_str().map(|d| !d.eq_ignore_ascii_case(host)).unwrap_or(true) {
            return None;
        }

        let mut path = u.path().to_string();
        if path.is_empty() {
            path.push('/');
        }
        if let Some(q) = u.query() {
            path.push('?');
            path.push_str(q);
        }
        Some(path)
    }

    pub(crate) fn is_internal_href(href: &str, host: &str) -> bool {
        let h = href.trim();
        if h.is_empty()
            || h.starts_with('#')
            || h.starts_with("javascript:")
            || h.starts_with("mailto:")
            || h.starts_with("tel:")
            || h.starts_with("data:")
        {
            return false;
        }
        if h.starts_with("//") {
            return h[2..]
                .split('/')
                .next()
                .map(|d| d.eq_ignore_ascii_case(host))
                .unwrap_or(false);
        }
        if h.starts_with("http://") || h.starts_with("https://") {
            return Url::parse(h)
                .ok()
                .and_then(|u| u.host_str().map(|d| d.eq_ignore_ascii_case(host)))
                .unwrap_or(false);
        }
        true
    }

    pub(crate) fn matches_any(hay: &str, keys: &[&str]) -> bool {
        keys.iter().any(|k| {
            if k.chars().count() <= 4 && !k.contains(' ') {
                hay.split(|c: char| !c.is_alphanumeric()).any(|t| t == *k)
            } else {
                hay.contains(k)
            }
        })
    }

    pub(crate) fn card_title(card: ElementRef<'_>, host: &str) -> Option<(String, String)> {
        let link_sel = Selector::parse("a[href]").ok()?;
        let link = card.select(&link_sel).find(|a| {
            Self::is_internal_href(a.value().attr("href").unwrap_or(""), host)
        })?;
        let href = link.value().attr("href")?.to_string();

        let title_sel = Selector::parse(
            "h1, h2, h3, h4, h5, .title, [class*=title], [class*=name], [itemprop=name]",
        )
        .ok()?;
        let title = card
            .select(&title_sel)
            .map(|e| e.text().collect::<String>().trim().to_string())
            .find(|t| !t.is_empty())
            .or_else(|| link.value().attr("title").map(|s| s.trim().to_string()))
            .or_else(|| {
                let img_sel = Selector::parse("img[alt]").ok()?;
                card.select(&img_sel)
                    .next()
                    .map(|i| i.value().attr("alt").unwrap_or("").trim().to_string())
            })
            .unwrap_or_default();

        Some((href, title))
    }

    pub(crate) fn infer_title_selector(cards: &[ElementRef<'_>]) -> String {
        let candidates = [
            "h3 a", "h2 a", "h4 a", "h3", "h2", "h4",
            "a[title]", ".title", "[class*=title]", "[class*=name]",
        ];
        for candidate in candidates {
            let Ok(sel) = Selector::parse(candidate) else { continue };
            let hits = cards
                .iter()
                .filter(|card| {
                    card.select(&sel)
                        .next()
                        .map(|e| !e.text().collect::<String>().trim().is_empty())
                        .unwrap_or(false)
                })
                .count();
            if !cards.is_empty() && hits * 2 >= cards.len() {
                return candidate.to_string();
            }
        }
        "a".to_string()
    }

    pub(crate) fn detect_pagination(&self, doc: &Html, origin: &str, host: &str) -> (String, String) {
        let Ok(sel) = Selector::parse("a[href]") else {
            return (String::new(), "none".to_string());
        };

        let mut style = "none".to_string();
        let mut found = false;

        for a in doc.select(&sel) {
            let href = a.value().attr("href").unwrap_or("");
            let rel = a.value().attr("rel").unwrap_or("").to_lowercase();
            let class = a.value().attr("class").unwrap_or("").to_lowercase();
            let aria = a.value().attr("aria-label").unwrap_or("").to_lowercase();
            let text = a.text().collect::<String>().trim().to_lowercase();

            let is_next = rel.split_whitespace().any(|r| r == "next")
                || text == "next"
                || text == "siguiente"
                || text == "»"
                || text == "›"
                || text.contains("siguiente")
                || class.contains("next")
                || class.contains("siguiente")
                || aria.contains("next")
                || aria.contains("siguiente");

            if !is_next {
                continue;
            }
            let Some(path) = Self::same_origin_path(href, origin, host) else { continue };
            if path.contains("page=") || path.contains("paged=") || path.contains("p=") {
                style = "query".to_string();
            } else if path.contains("/page/")
                || path.contains("/pagina/")
                || path.contains("/p/")
            {
                style = "path".to_string();
            }
            found = true;
            break;
        }

        // Si no hay enlace "siguiente", usar la paginación numérica como pista.
        if !found {
            for a in doc.select(&sel) {
                if a.text().collect::<String>().trim() != "2" {
                    continue;
                }
                let href = a.value().attr("href").unwrap_or("");
                let Some(path) = Self::same_origin_path(href, origin, host) else { continue };
                if path.contains("page=2") || path.contains("paged=2") {
                    style = "query".to_string();
                    found = true;
                    break;
                }
                if path.contains("/page/2") || path.contains("/pagina/2") {
                    style = "path".to_string();
                    found = true;
                    break;
                }
            }
        }

        let selector = if found {
            "a[rel=next], a.next, a[class*=next], a[class*=siguiente]".to_string()
        } else {
            String::new()
        };
        (selector, style)
    }
}
