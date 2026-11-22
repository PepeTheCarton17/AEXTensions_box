use regex::Regex;
use std::collections::HashMap;

use super::models::{SiteAnalyzer, SiteCapabilities};

impl SiteAnalyzer {

    pub(crate) fn label_mentions(labels: &[String], needles: &[&str]) -> bool {
        labels.iter().any(|label| {
            let lower = label.to_lowercase();
            needles.iter().any(|needle| lower.contains(needle))
        })
    }

    /// Nombres legibles de los métodos encontrados. Se muestran tal cual: si
    /// no hay evidencia, se dice expresamente que no hay método confirmado.
    pub fn playback_methods(caps: &SiteCapabilities) -> Vec<String> {
        let mut out = Vec::new();
        let cases = &caps.player_cases;
        if cases.iter().any(|c| c == "session-bootstrap") {
            out.push("Sesión web".to_string());
        }
        if cases.iter().any(|c| c == "graphql-persisted") {
            out.push("GraphQL interno".to_string());
        }
        if cases.iter().any(|c| c == "hls-master") {
            out.push("HLS".to_string());
        }
        if cases.iter().any(|c| c == "dash-mpd") {
            out.push("DASH".to_string());
        }
        if cases.iter().any(|c| c == "iframe-player")
            || caps.stream_formats.iter().any(|f| f == "iframe")
        {
            out.push("Reproductor incrustado".to_string());
        }
        if caps.stream_formats.iter().any(|f| f == "mp4") {
            out.push("MP4 directo".to_string());
        }
        if out.is_empty() {
            out.push("Sin método confirmado".to_string());
        }
        out
    }

    /// Detecta capacidades reales a partir del HTML disponible. Es
    /// determinista y conservador: sólo afirma lo que aparece en el cuerpo.
    pub(crate) fn detect_capabilities(body: &str) -> SiteCapabilities {
        let mut caps = SiteCapabilities::default();
        let lower = body.to_lowercase();

        fn push_unique(list: &mut Vec<String>, value: String) {
            if !value.is_empty() && !list.contains(&value) && list.len() < 12 {
                list.push(value);
            }
        }

        if lower.contains(".m3u8") || lower.contains("application/vnd.apple.mpegurl") || lower.contains("#extm3u") {
            caps.stream_formats.push("hls".to_string());
        }
        if lower.contains(".mpd") || lower.contains("application/dash+xml") || lower.contains("dash.js") {
            caps.stream_formats.push("dash".to_string());
        }
        if lower.contains(".mp4") {
            caps.stream_formats.push("mp4".to_string());
        }
        if lower.contains("<iframe") {
            caps.stream_formats.push("iframe".to_string());
        }

        // Caso del reproductor. Clasifica el sitio para que el generador pueda
        // ordenar las estrategias antes de tener aciertos/fallos reales, y para
        // avisar cuando el stream exige algo que Nekoyomi no puede hacer.
        let mut cases: Vec<String> = Vec::new();
        if lower.contains("stitcherparams") || lower.contains("sessiontoken")
            || (lower.contains("boot.") && lower.contains("/v4/start"))
        {
            push_unique(&mut cases, "session-bootstrap".to_string());
        }
        if (lower.contains("graphql") || lower.contains("apollo"))
            && (lower.contains("persisteddocumenthash") || lower.contains("tnpersisteddocumenthash"))
        {
            push_unique(&mut cases, "graphql-persisted".to_string());
        }
        if caps.stream_formats.iter().any(|f| f == "hls") {
            push_unique(&mut cases, "hls-master".to_string());
        }
        if caps.stream_formats.iter().any(|f| f == "dash") {
            push_unique(&mut cases, "dash-mpd".to_string());
        }
        if caps.stream_formats.iter().any(|f| f == "iframe") {
            push_unique(&mut cases, "iframe-player".to_string());
        }
        if lower.contains("widevine") || lower.contains("playready") || lower.contains("fairplay") {
            push_unique(&mut cases, "drm-widevine".to_string());
        }
        // Detección honesta de esquemas que AEXT NO resuelve. No generamos
        // código para ellos: solo los etiquetamos para avisar al usuario.
        if lower.contains("signaturecipher") || lower.contains("innertube")
            || lower.contains("player_response") || lower.contains("playerresponse")
            || lower.contains("descramble") || lower.contains("descipher")
            || (lower.contains("nsig") && lower.contains("player"))
        {
            push_unique(&mut cases, "cipher-signature".to_string());
        }
        if lower.contains("extension-proxy") || lower.contains("corsproxy")
            || lower.contains("cors-proxy") || lower.contains("proxy.php")
            || lower.contains("/proxy?")
        {
            push_unique(&mut cases, "extension-proxy".to_string());
        }
        if lower.contains("_ad/") || lower.contains("_ad%2f")
            || lower.contains("dai.google.com")
        {
            push_unique(&mut cases, "ads-hls".to_string());
        }
        caps.player_cases = cases;

        // Calidades e idiomas de un máster HLS embebido en el propio HTML.
        if let Ok(re) = Regex::new(r#"RESOLUTION\s*=\s*\d+x(\d+)"#) {
            for cap in re.captures_iter(body) {
                if let Some(height) = cap.get(1) {
                    push_unique(&mut caps.qualities, format!("{}p", height.as_str()));
                }
            }
        }
        if let Ok(re) = Regex::new(r#"#EXT-X-MEDIA:([^\r\n]+)"#) {
            for cap in re.captures_iter(body) {
                let mut attrs = HashMap::new();
                // Los atributos HLS pueden ir con comillas (NAME="English") o
                // sin ellas (TYPE=AUDIO). Aceptar ambos es necesario para no
                // perder las pistas reales de un máster embebido en el HTML.
                if let Ok(pairs) = Regex::new(r#"([A-Za-z0-9-]+)\s*=\s*(?:"([^"]*)"|([^,\r\n]*))"#) {
                    for pair in pairs.captures_iter(cap.get(1).map(|m| m.as_str()).unwrap_or("")) {
                        let value = pair
                            .get(2)
                            .or_else(|| pair.get(3))
                            .map(|m| m.as_str().trim().to_string())
                            .unwrap_or_default();
                        attrs.insert(pair[1].to_uppercase(), value);
                    }
                }
                let kind = attrs.get("TYPE").map(|s| s.as_str()).unwrap_or("");
                let lang = attrs.get("LANGUAGE").cloned().unwrap_or_default();
                let name = attrs.get("NAME").cloned().unwrap_or_else(|| lang.clone());
                let label = if !lang.is_empty() && lang != "und" {
                    format!("{} ({})", name, lang)
                } else {
                    name
                };
                if kind.eq_ignore_ascii_case("AUDIO") {
                    push_unique(&mut caps.audio_languages, label);
                } else if kind.eq_ignore_ascii_case("SUBTITLES") {
                    push_unique(&mut caps.subtitle_languages, label);
                }
            }
        }

        // Endpoints API/GraphQL citados en HTML o JS embebido.
        if let Ok(re) = Regex::new(r#"https?://[^"'\s<>\\]+"#) {
            for cap in re.captures_iter(body) {
                let url = cap[0].trim_end_matches(|c: char| matches!(c, ',' | ';' | ')' | '}' | ']' | '`'));
                let lower_url = url.to_lowercase();
                // Los arranques de sesión de reproductores web suelen vivir en
                // `/v4/start` o en un host `boot.`, sin `/api/` en la ruta.
                if (lower_url.contains("/api/")
                    || lower_url.contains("/graphql")
                    || lower_url.contains("api.")
                    || lower_url.contains("/v4/start")
                    || lower_url.contains("/stitch/")
                    || lower_url.contains("boot."))
                    && url.len() < 200
                {
                    push_unique(&mut caps.api_endpoints, url.to_string());
                }
            }
        }

        // Versión declarada por el propio sitio, en meta tag o en JSON embebido.
        if caps.app_version.is_none() {
            for pattern in [
                r#"(?i)(?:globalAppVersion|appVersion|app_version)\s*[:=]\s*["']([0-9][0-9A-Za-z.\-]+)["']"#,
                r#"(?i)name=["'](?:appVersion|app_version|globalAppVersion)["'][^>]*content=["']([0-9][0-9A-Za-z.\-]+)["']"#,
            ] {
                if let Ok(re) = Regex::new(pattern) {
                    if let Some(cap) = re.captures(body) {
                        caps.app_version = Some(cap[1].to_string());
                        break;
                    }
                }
            }
        }

        // Los estados hidratados (Next.js/React Query) guardan la región real
        // resuelta por IP, a veces con comillas escapadas dentro de un bloque
        // RSC. Normalizamos esas comillas antes de leerla para no depender del
        // formato de serialización.
        let flat = body.replace("\\\"", "\"");
        if let Ok(re) = Regex::new(
            r#""region"\s*:\s*\{[^}]*?"locale"\s*:\s*"([^"]+)"[^}]*?"region"\s*:\s*"([^"]+)""#,
        ) {
            if let Some(cap) = re.captures(&flat) {
                caps.locale = Some(cap[1].to_string());
                caps.region = Some(cap[2].to_string());
            }
        }
        if caps.region.is_none() {
            if let Ok(re) = Regex::new(r#""region"\s*:\s*"([A-Za-z]{2})""#) {
                if let Some(cap) = re.captures(&flat) {
                    caps.region = Some(cap[1].to_uppercase());
                }
            }
        }
        if caps.locale.is_none() {
            if let Ok(re) = Regex::new(r#"translations-([a-z]{2}(?:-[A-Za-z0-9]+)?)"#) {
                if let Some(cap) = re.captures(&flat) {
                    caps.locale = Some(cap[1].to_string());
                }
            }
        }

        caps
    }

    /// Combina lo visto en la portada con lo visto en una ficha de ejemplo.
    pub(crate) fn merge_capabilities(base: &mut SiteCapabilities, extra: &SiteCapabilities) {
        fn merge(base: &mut Vec<String>, extra: &[String]) {
            for value in extra {
                if !base.contains(value) {
                    base.push(value.clone());
                }
            }
        }
        merge(&mut base.stream_formats, &extra.stream_formats);
        merge(&mut base.qualities, &extra.qualities);
        merge(&mut base.audio_languages, &extra.audio_languages);
        merge(&mut base.subtitle_languages, &extra.subtitle_languages);
        merge(&mut base.api_endpoints, &extra.api_endpoints);
        merge(&mut base.player_cases, &extra.player_cases);
        merge(&mut base.playback_methods, &extra.playback_methods);
        base.master_verified |= extra.master_verified;
        base.sub_verified |= extra.sub_verified;
        base.dub_verified |= extra.dub_verified;
        if base.app_version.is_none() {
            base.app_version = extra.app_version.clone();
        }
        if base.region.is_none() {
            base.region = extra.region.clone();
        }
        if base.locale.is_none() {
            base.locale = extra.locale.clone();
        }
    }
}
