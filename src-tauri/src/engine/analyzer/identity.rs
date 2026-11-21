use scraper::{Html, Selector};
use std::collections::HashSet;

use super::models::SiteAnalyzer;

impl SiteAnalyzer {

    pub(crate) fn detect_site_name(&self, doc: &Html, url: &str) -> String {
        // 1. El dominio es el identificador más estable. Evita nombres basura
        //    como "Bienvenido" o "Inicio" tomados del <title> de la portada.
        let domain_name = Self::extract_domain_label(url);
        if !domain_name.is_empty() && !Self::is_generic_title(&domain_name) {
            return domain_name;
        }

        // 2. Metadatos del sitio (og:site_name y equivalentes)
        let meta_selectors = [
            "meta[property='og:site_name']",
            "meta[name='application-name']",
            "meta[name='apple-mobile-web-app-title']",
        ];
        for sel_str in meta_selectors {
            if let Ok(sel) = Selector::parse(sel_str) {
                if let Some(el) = doc.select(&sel).next() {
                    if let Some(val) = el.value().attr("content") {
                        let clean = val.trim();
                        if Self::is_valid_name(clean) {
                            return Self::title_case_name(clean);
                        }
                    }
                }
            }
        }

        // 3. Revisar <title> buscando partes no genéricas
        let title_sel = Selector::parse("title").unwrap();
        if let Some(el) = doc.select(&title_sel).next() {
            let t = el.text().collect::<String>();
            let parts: Vec<&str> = t.split(&['|', '-', '–', '—', ':', '·', '•'][..]).map(|s| s.trim()).collect();
            for part in &parts {
                if Self::is_valid_name(part) {
                    return Self::title_case_name(part);
                }
            }
        }

        if !domain_name.is_empty() {
            return domain_name;
        }

        "AnimeExtension".to_string()
    }

    pub(crate) fn is_valid_name(s: &str) -> bool {
        let clean = s.trim();
        !clean.is_empty() && clean.chars().count() <= 42 && !Self::is_generic_title(clean)
    }

    pub(crate) fn is_generic_title(s: &str) -> bool {
        let lower = s.trim().to_lowercase();
        if lower.is_empty() {
            return true;
        }

        let generics = [
            "bienvenido", "bienvenida", "bienvenidos", "bienvenidas", "welcome", "home",
            "inicio", "index", "catalog", "catalogo", "catálogo", "search", "buscar",
            "anime", "animes", "manga", "mangas", "series", "serie", "peliculas",
            "películas", "movies", "home page", "pagina principal", "página principal",
            "principal", "portada", "not found", "404", "error", "no encontrado",
            "loading", "cargando", "redirecting", "redireccionando", "anime online",
            "manga online", "ver anime", "watch anime", "leer manga", "read manga",
            "default", "untitled", "sin titulo", "sin título", "just a moment",
            "attention required", "access denied", "cloudflare", "ddos-guard",
            "ver series", "watch series", "hentai", "raw", "scan", "scans",
        ];
        if generics.contains(&lower.as_str()) {
            return true;
        }

        if lower.chars().count() <= 2 {
            return true;
        }

        let prefixes = [
            "bienvenid", "welcome to", "inicio |", "inicio -", "home |", "home -",
            "cargando", "loading", "redirect", "redirig", "404", "not found",
            "pagina no", "página no", "access denied", "just a moment",
            "attention required", "cloudflare", "ddos-guard",
        ];
        prefixes.iter().any(|p| lower.starts_with(p))
    }

    pub(crate) fn title_case_name(s: &str) -> String {
        let words: Vec<String> = s
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    Some(first) => {
                        let mut out = String::new();
                        out.push(first.to_uppercase().collect::<String>().chars().next().unwrap_or(first));
                        out.push_str(&chars.as_str().to_lowercase());
                        out
                    }
                    None => String::new(),
                }
            })
            .filter(|w| !w.is_empty())
            .collect();

        if words.is_empty() {
            s.trim().to_string()
        } else {
            words.join(" ")
        }
    }

    pub(crate) fn extract_domain_label(url: &str) -> String {
        let without_proto = url.trim_start_matches("https://").trim_start_matches("http://");
        let host = without_proto.split('/').next().unwrap_or(without_proto);
        let host = host.split(':').next().unwrap_or(host);
        let labels: Vec<&str> = host.split('.').filter(|s| !s.is_empty()).collect();
        if labels.is_empty() {
            return String::new();
        }

        // Direcciones IP y hosts sin TLD: no sirven como nombre bonito.
        if labels.iter().all(|l| l.chars().all(|c| c.is_ascii_digit())) {
            return String::new();
        }

        let two_level = ["com", "co", "net", "org", "gob", "edu", "ac", "gov", "nic", "mil"];
        let mut idx = labels.len();
        if idx >= 1 {
            idx -= 1; // quitar TLD
        }
        if idx >= 2 {
            let mid = labels[idx - 1].to_ascii_lowercase();
            if two_level.contains(&mid.as_str()) && labels[idx].chars().count() <= 2 {
                idx -= 1; // quitar segundo nivel (co.uk, com.ar, ...)
            }
        }

        if idx == 0 {
            return String::new();
        }

        let skipped = [
            "www", "www1", "www2", "www3", "www4", "m", "mobile", "app", "api",
            "cdn", "beta", "new", "web", "ver", "watch", "online", "site",
        ];
        let mut pos = idx - 1;
        while pos > 0 && skipped.contains(&labels[pos].to_ascii_lowercase().as_str()) {
            pos -= 1;
        }

        let label = labels[pos];
        if label.is_empty()
            || label.chars().count() <= 1
            || skipped.contains(&label.to_ascii_lowercase().as_str())
            || two_level.contains(&label.to_ascii_lowercase().as_str())
        {
            return String::new();
        }

        let mut chars = label.chars();
        match chars.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    pub(crate) fn detect_favicon(&self, doc: &Html, base_url: &str) -> Option<String> {
        let selectors = [
            "link[rel*='icon'][sizes='192x192']",
            "link[rel*='apple-touch-icon']",
            "link[rel*='icon']",
            "link[rel='shortcut icon']"
        ];
        for sel_str in selectors {
            if let Ok(sel) = Selector::parse(sel_str) {
                if let Some(el) = doc.select(&sel).next() {
                    if let Some(href) = el.value().attr("href") {
                        if href.starts_with("http") {
                            return Some(href.to_string());
                        } else if href.starts_with("//") {
                            return Some(format!("https:{}", href));
                        } else {
                            return Some(format!("{}/{}", base_url, href.trim_start_matches('/')));
                        }
                    }
                }
            }
        }
        Some(format!("{}/favicon.ico", base_url))
    }

    pub(crate) fn detect_cms_and_type(&self, body: &str) -> (String, String) {
        let body_lower = body.to_lowercase();
        let is_manga = body_lower.contains("wp-manga")
            || body_lower.contains("madara")
            || body_lower.contains("mangathemesia")
            || body_lower.contains("theme-mangathemesia")
            || body_lower.contains("manhwa")
            || body_lower.contains("scanlation")
            || (body_lower.contains("manga") && !body_lower.contains("anime") && !body_lower.contains("video"));

        let cms = if body.contains("__sveltekit") || body.contains("data-sveltekit") || body.contains("/_app/immutable") {
            "sveltekit"
        } else if body.contains("dooplay") || body.contains("wp-content/themes/dooplay") {
            "dooplay"
        } else if body.contains("wp-manga") || body.contains("madara") {
            "madara"
        } else if body.contains("mangathemesia") || body.contains("theme-mangathemesia") {
            "mangathemesia"
        } else {
            "generic"
        };

        let ext_type = if is_manga { "manga" } else { "anime" };
        (cms.to_string(), ext_type.to_string())
    }

    pub(crate) fn detect_servers_and_dub(&self, body: &str, _base_url: &str) -> (Vec<String>, bool) {
        let mut hosts = HashSet::new();
        let known_hosts = [
            ("MP4Upload", "mp4upload.com"),
            ("Voe", "voe.sx"),
            ("StreamWish", "streamwish"),
            ("Byse", "byse"),
            ("Zilla", "player.zilla-networks.com"),
            ("Filemoon", "filemoon"),
            ("DoodStream", "dood"),
            ("UPNShare", "uns.bio"),
            ("Mega", "mega.nz"),
        ];

        for (label, pattern) in known_hosts {
            if body.contains(pattern) {
                hosts.insert(label.to_string());
            }
        }

        let has_sub_dub = body.contains("SUB:[") || body.contains("DUB:[") || body.contains("LAT:[")
            || body.contains("audio=dub") || body.contains("latino");

        (hosts.into_iter().collect(), has_sub_dub)
    }

    pub(crate) fn detect_selectors(&self, cms: &str) -> (String, String, String) {
        match cms {
            "dooplay" => (
                "article.item-movies, article.item, div.poster".to_string(),
                "div.data a, h3 a".to_string(),
                "div.pagination a.current + a".to_string(),
            ),
            "madara" => (
                "div.page-item-detail, div.c-tabs-item__content".to_string(),
                "div.post-title a, h3 a".to_string(),
                "div.nav-previous a, a.next".to_string(),
            ),
            "sveltekit" => (
                "article".to_string(),
                "h3, div[class*=text-2xs]".to_string(),
                "a:contains(»)".to_string(),
            ),
            _ => (
                "article, div.item, div.card".to_string(),
                "h3 a, h2 a, .title a".to_string(),
                "a.next, a[rel=next]".to_string(),
            ),
        }
    }
}
