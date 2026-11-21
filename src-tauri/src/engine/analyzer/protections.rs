use reqwest::header::HeaderMap;

/// Protecciones anunciadas por el HTML o por las cabeceras de la respuesta.
/// Sólo se añaden avisos cuando el sitio las declara explícitamente.
pub(crate) fn detect_protections(body: &str, headers: &HeaderMap) -> Vec<String> {
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
    protections
}
