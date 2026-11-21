use regex::Regex;

use super::models::{AnalysisResult, DetailProbe, MasterTrackProbe, SiteAnalyzer};

impl SiteAnalyzer {

    /// Extrae un contentId de muestra de una ruta o de un estado hidratado.
    pub(crate) fn content_id_from_hint(hint: &str) -> Option<(String, String)> {
        let flat = hint.replace("\\\"", "\"");
        if let Ok(re) = Regex::new(r"(?i)(episodes?|movies?)/([0-9a-f]{24})") {
            if let Some(cap) = re.captures(&flat) {
                let raw = cap[1].to_lowercase();
                let kind = if raw.starts_with("movie") { "movie" } else { "episode" };
                return Some((kind.to_string(), cap[2].to_string()));
            }
        }
        let re = Regex::new(r#"(?i)contentId\s*[:=]\s*["']([0-9a-f]{24})["']"#).ok()?;
        let id = re.captures(&flat)?[1].to_string();
        let kind = if flat.to_lowercase().contains("/movie") { "movie" } else { "episode" };
        Some((kind.to_string(), id))
    }

    /// Identificador con forma de UUID para el sondeo. Pluto no valida que
    /// corresponda a una sesión de navegador real, sólo que tenga ese formato.
    pub(crate) fn probe_client_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let pid = std::process::id() as u64;
        let tail = (nanos ^ pid.wrapping_mul(0x9e37_79b9_7f4a_7c15)) & 0x0fff_ffff_ffff;
        format!(
            "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
            (nanos ^ (pid << 32)) as u32,
            ((nanos >> 32) & 0xffff) as u16,
            ((nanos >> 12) & 0x0fff) as u16,
            ((nanos >> 40) & 0x0fff) as u16,
            tail
        )
    }

    /// Comprueba de verdad un máster HLS de muestra. Es la única fuente de las
    /// banderas `master_verified`/`sub_verified`/`dub_verified`: un texto
    /// "SUB"/"DUB" en el DOM no cuenta como pista de audio o subtítulo.
    pub(crate) async fn probe_master_tracks(
        &self,
        result: &AnalysisResult,
        hints: &[String],
    ) -> MasterTrackProbe {
        let mut out = MasterTrackProbe::default();
        let uses_session = result
            .capabilities
            .player_cases
            .iter()
            .any(|c| c == "session-bootstrap")
            || result.capabilities.api_endpoints.iter().any(|e| {
                e.contains("/v4/start") || e.contains("boot.") || e.contains("/stitch/")
            });
        if !uses_session {
            return out;
        }
        let Some((kind, id)) = hints.iter().find_map(|h| Self::content_id_from_hint(h)) else {
            out.detail = "no se encontró un contenido de muestra".to_string();
            return out;
        };
        out.attempted = true;

        let app_version = result
            .capabilities
            .app_version
            .as_deref()
            .filter(|v| !v.is_empty())
            .unwrap_or("10.12.1096");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let boot_url = format!(
            "https://boot.pluto.tv/v4/start?appName=web&appVersion={}&deviceVersion=124.0.0.0&deviceModel=web&deviceMake=chrome&deviceType=web&clientID={}&clientModelNumber=1.0.0&serverSideAds=false&clientTime={}",
            app_version,
            Self::probe_client_id(),
            now
        );
        let Ok(boot) = self
            .client
            .get(&boot_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .header("Referer", "https://pluto.tv/")
            .header("Accept", "application/json")
            .send()
            .await
        else {
            out.detail = "boot/start no respondió".to_string();
            return out;
        };
        if !boot.status().is_success() {
            out.detail = format!("boot/start HTTP {}", boot.status());
            return out;
        }
        let Ok(boot_body) = boot.text().await else {
            out.detail = "boot/start sin cuerpo".to_string();
            return out;
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&boot_body) else {
            out.detail = "boot/start no devolvió JSON".to_string();
            return out;
        };
        let token = json.get("sessionToken").and_then(|v| v.as_str()).unwrap_or("");
        let params = json.get("stitcherParams").and_then(|v| v.as_str()).unwrap_or("");
        let stitcher = json
            .get("stitcher")
            .and_then(|v| v.as_str())
            .unwrap_or("https://service-stitcher.clusters.pluto.tv");
        if token.is_empty() || params.is_empty() {
            out.detail = "boot/start sin sesión utilizable".to_string();
            return out;
        }

        let segment = if kind == "movie" { "movie" } else { "episode" };
        let master_url = format!(
            "{}/v2/stitch/hls/{}/{}/master.m3u8?{}&jwt={}&includeExtendedEvents=true&masterJWTPassthrough=true",
            stitcher.trim_end_matches('/'), segment, id, params, token
        );
        let Ok(master) = self
            .client
            .get(&master_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .header("Referer", "https://pluto.tv/")
            .header("Accept", "*/*")
            .send()
            .await
        else {
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
