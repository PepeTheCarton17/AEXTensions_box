use serde::{Deserialize, Serialize};
use regex::Regex;
use scraper::{Html, Selector};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub url: String,
    pub name: String,
    pub lang: String,
    pub ext_type: String, // "anime" | "manga"
    pub cms: String,      // "sveltekit" | "dooplay" | "madara" | "mangathemesia" | "generic"
    pub favicon_url: Option<String>,
    pub detected_hosts: Vec<String>,
    pub has_sub_dub: bool,
    pub protections: Vec<String>,
    pub card_selector: String,
    pub title_selector: String,
    pub next_page_selector: String,
}

pub struct SiteAnalyzer {
    client: reqwest::Client,
}

impl SiteAnalyzer {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_default();
        Self { client }
    }

    pub async fn analyze(&self, url: &str) -> Result<AnalysisResult, String> {
        let clean_url = url.trim_end_matches('/');
        let resp = self.client.get(clean_url)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .send()
            .await
            .map_err(|e| format!("Error conectando con {}: {}", url, e))?;

        let status = resp.status();
        let headers = resp.headers().clone();
        let body = resp.text().await.map_err(|e| format!("Error leyendo HTML: {}", e))?;

        let doc = Html::parse_document(&body);

        // 1. Detección de Nombre y Título
        let name = self.detect_site_name(&doc, clean_url);

        // 2. Detección de Favicon
        let favicon_url = self.detect_favicon(&doc, clean_url);

        // 3. Detección de Protecciones
        let mut protections = Vec::new();
        if body.contains("challenges.cloudflare.com/turnstile") || body.contains("cf-turnstile") {
            protections.push("Cloudflare Turnstile".to_string());
        }
        if body.contains("anubis") || headers.get("X-Anubis-Challenge").is_some() {
            protections.push("Anubis PoW Challenge".to_string());
        }
        if headers.get("server").and_then(|s| s.to_str().ok()).map(|s| s.contains("cloudflare")).unwrap_or(false) {
            protections.push("Cloudflare WAF".to_string());
        }

        // 4. Detección de CMS y Tipo
        let (cms, ext_type) = self.detect_cms_and_type(&body);

        // 5. Detección de Servidores de Video (para anime)
        let (detected_hosts, has_sub_dub) = self.detect_servers_and_dub(&body, clean_url).await;

        // 6. Selectores CSS
        let (card_sel, title_sel, next_sel) = self.detect_selectors(&doc, &cms);

        Ok(AnalysisResult {
            url: clean_url.to_string(),
            name,
            lang: "es".to_string(),
            ext_type,
            cms,
            favicon_url,
            detected_hosts,
            has_sub_dub,
            protections,
            card_selector: card_sel,
            title_selector: title_sel,
            next_page_selector: next_sel,
        })
    }

    fn detect_site_name(&self, doc: &Html, url: &str) -> String {
        let meta_sel = Selector::parse("meta[property='og:site_name']").unwrap();
        if let Some(el) = doc.select(&meta_sel).next() {
            if let Some(val) = el.value().attr("content") {
                if !val.trim().is_empty() {
                    return val.trim().to_string();
                }
            }
        }
        let title_sel = Selector::parse("title").unwrap();
        if let Some(el) = doc.select(&title_sel).next() {
            let t = el.text().collect::<String>();
            let clean = t.split(&['|', '-', '–', '—', ':'][..]).next().unwrap_or(&t).trim();
            if !clean.is_empty() {
                return clean.to_string();
            }
        }
        url.trim_start_matches("https://").trim_start_matches("http://")
            .split('.').next().unwrap_or("Extension").to_string()
    }

    fn detect_favicon(&self, doc: &Html, base_url: &str) -> Option<String> {
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

    fn detect_cms_and_type(&self, body: &str) -> (String, String) {
        if body.contains("__sveltekit") || body.contains("data-sveltekit") || body.contains("/_app/immutable") {
            ("sveltekit".to_string(), "anime".to_string())
        } else if body.contains("dooplay") || body.contains("wp-content/themes/dooplay") {
            ("dooplay".to_string(), "anime".to_string())
        } else if body.contains("wp-manga") || body.contains("madara") {
            ("madara".to_string(), "manga".to_string())
        } else if body.contains("mangathemesia") || body.contains("theme-mangathemesia") {
            ("mangathemesia".to_string(), "manga".to_string())
        } else if body.contains("capitulo") || body.contains("episodio") || body.contains("anime") {
            ("generic".to_string(), "anime".to_string())
        } else {
            ("generic".to_string(), "manga".to_string())
        }
    }

    async fn detect_servers_and_dub(&self, body: &str, _base_url: &str) -> (Vec<String>, bool) {
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

    fn detect_selectors(&self, _doc: &Html, cms: &str) -> (String, String, String) {
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
