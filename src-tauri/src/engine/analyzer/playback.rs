use regex::Regex;

use super::models::{AnalysisResult, DetailProbe, MasterTrackProbe, SiteAnalyzer};

impl SiteAnalyzer {

    /// Extrae del texto una playlist HLS citada por el sitio. Es una señal
    /// genérica: ni el dominio ni el proveedor intervienen en la decisión.
    pub(crate) fn direct_master_url(hint: &str) -> Option<String> {
        let flat = hint.replace("\\/", "/").replace("\\u0026", "&");
        let re = Regex::new(r#"https?://[^"'\s<>\\]+?\.m3u8(?:\?[^"'\s<>\\]*)?"#).ok()?;
        re.find(&flat).map(|m| m.as_str().trim_end_matches(|c: char| matches!(c, ')' | '}' | ']' | '`')).to_string())
    }

    /// Comprueba de verdad un máster HLS citado por el sitio. Es la única
    /// fuente de las banderas `master_verified`/`sub_verified`/`dub_verified`:
    /// un texto "SUB"/"DUB" en el DOM no cuenta como pista real.
    pub(crate) async fn probe_master_tracks(
        &self,
        _result: &AnalysisResult,
        hints: &[String],
    ) -> MasterTrackProbe {
        let mut out = MasterTrackProbe::default();
        let Some(url) = hints.iter().find_map(|h| Self::direct_master_url(h)) else {
            out.detail = "el sitio no citó ningún máster HLS directo".to_string();
            return out;
        };
        out.attempted = true;

        let Ok(master) = self.client.get(&url).send().await else {
            out.detail = "máster HLS no respondió".to_string();
            return out;
        };
        let status = master.status();
        let Ok(body) = master.text().await else {
            out.detail = "máster HLS sin cuerpo".to_string();
            return out;
        };
        if !status.is_success() {
            out.detail = format!("máster HLS HTTP {}", status);
            if status.as_u16() == 403 {
                out.detail.push_str(" (token o región rechazados)");
            }
            return out;
        }
        if !body.contains("#EXTM3U")
            || !(body.contains("#EXT-X-STREAM-INF") || body.contains("#EXTINF"))
        {
            out.detail = format!("respuesta sin playlist ({} bytes)", body.len());
            return out;
        }

        out.verified = true;
        out.detail = format!("máster HLS ({} bytes)", body.len());
        if let Ok(re) = Regex::new(r#"#EXT-X-MEDIA:([^\r\n]+)"#) {
            for cap in re.captures_iter(&body) {
                let line = cap.get(1).map(|m| m.as_str()).unwrap_or("");
                let attr = |name: &str| -> String {
                    Regex::new(&format!(r#"{}="([^"]*)""#, name))
                        .ok()
                        .and_then(|r| r.captures(line))
                        .map(|c| c[1].trim().to_string())
                        .unwrap_or_default()
                };
                let kind = attr("TYPE").to_uppercase();
                let lang = attr("LANGUAGE");
                let name = attr("NAME");
                let label = match (name.is_empty(), lang.is_empty() || lang == "und") {
                    (true, true) => String::new(),
                    (true, false) => lang,
                    (false, true) => name,
                    (false, false) => format!("{} ({})", name, lang),
                };
                if label.is_empty() {
                    continue;
                }
                let list = if kind == "AUDIO" {
                    &mut out.audio_languages
                } else if kind == "SUBTITLES" {
                    &mut out.subtitle_languages
                } else {
                    continue;
                };
                if !list.contains(&label) {
                    list.push(label);
                }
            }
        }
        out
    }

    /// Abre una ficha de ejemplo, cuenta enlaces de episodios y deduce un
    /// selector que sí los encuentre. No modifica el análisis: solo aporta
    /// evidencia para los avisos y para la lista de episodios generada.
    pub(crate) async fn probe_detail(&self, detail_url: &str) -> DetailProbe {
        let mut probe = DetailProbe {
            url: detail_url.to_string(),
            fetched: false,
            episode_links: 0,
            has_embed: false,
            episode_selector: None,
            html: String::new(),
        };

        let Ok(resp) = self.client.get(detail_url).send().await else {
            return probe;
        };
        if !resp.status().is_success() {
            return probe;
        }
        let Ok(body) = resp.text().await else {
            return probe;
        };
        probe.fetched = true;
        probe.has_embed = body.contains(".m3u8") || body.contains("<iframe");
        probe.html = body.clone();
        if let Some((selector, count)) = Self::detect_episode_selector(&body) {
            probe.episode_selector = Some(selector);
            probe.episode_links = count;
        }

        probe
    }
}
