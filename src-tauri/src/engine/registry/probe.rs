//! Ejecución de las sondas declaradas por los motores.
//!
//! Todas las peticiones son anónimas: se usa un `reqwest::Client` sin jar de
//! cookies y sólo se envían las cabeceras declaradas en el manifiesto (que la
//! validación prohíbe que sean de sesión). AEXT no elude login ni DRM.

use super::model::{EngineDescriptor, EngineProbe, EngineTrial};
use scraper::{Html, Selector};

const MAX_TARGETS: usize = 5;
const MAX_SAMPLE: usize = 4000;

/// Datos reales observados en el sitio, usados para rellenar los marcadores.
pub struct ProbeContext {
    pub base: String,
    pub endpoints: Vec<String>,
    pub streams: Vec<String>,
}

fn fill(template: &str, value: &str) -> String {
    template
        .replace("{base}", value)
        .replace("{endpoint}", value)
        .replace("{stream}", value)
}

fn driver(template: &str) -> Option<&'static str> {
    if template.contains("{endpoint}") {
        Some("{endpoint}")
    } else if template.contains("{stream}") {
        Some("{stream}")
    } else if template.contains("{base}") {
        Some("{base}")
    } else {
        None
    }
}

fn targets(probe: &EngineProbe, ctx: &ProbeContext) -> Vec<String> {
    match driver(&probe.url_template) {
        Some("{endpoint}") => ctx.endpoints.iter().take(MAX_TARGETS).cloned().collect(),
        Some("{stream}") => ctx.streams.iter().take(MAX_TARGETS).cloned().collect(),
        Some("{base}") => vec![ctx.base.clone()],
        _ => vec![probe.url_template.clone()],
    }
}

fn count_json(body: &str, field: Option<&str>) -> (usize, Option<String>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return (0, None);
    };
    let list = match field {
        Some(key) => value
            .get(key)
            .or_else(|| value.get("data").and_then(|data| data.get(key))),
        None => {
            if value.is_array() {
                Some(&value)
            } else {
                value
                    .as_object()
                    .and_then(|map| map.values().find(|item| item.is_array()))
            }
        }
    };
    let array = list.and_then(|item| item.as_array());
    let count = array.map(|a| a.len()).unwrap_or(0);
    // Guardamos la lista, no el envelope: el generador consume arrays reales.
    let sample = array
        .and_then(|a| serde_json::to_string(a).ok())
        .map(|json| json.chars().take(MAX_SAMPLE).collect())
        .unwrap_or_default();
    (count, Some(sample))
}

fn count_html(body: &str, selector: &str) -> (usize, Option<String>, String) {
    let parsed = match Selector::parse(selector) {
        Ok(selector) => selector,
        Err(error) => return (0, None, format!("selector inválido: {:?}", error)),
    };
    let document = Html::parse_document(body);
    (document.select(&parsed).count(), None, String::new())
}

fn count_hls(body: &str) -> (usize, Option<String>, String) {
    let valid = body.contains("#EXTM3U")
        && (body.contains("#EXT-X-STREAM-INF") || body.contains("#EXTINF"));
    if valid {
        (1, None, "playlist HLS válida".to_string())
    } else {
        (0, None, "respuesta sin playlist HLS".to_string())
    }
}

fn empty_trial(engine: &EngineDescriptor, probe: &EngineProbe, url: &str) -> EngineTrial {
    EngineTrial {
        engine_id: engine.id.clone(),
        engine_name: engine.name.clone(),
        license: engine.license.clone(),
        provenance: engine.provenance.clone(),
        probe_id: probe.id.clone(),
        url: url.to_string(),
        ..Default::default()
    }
}

/// Prueba todas las sondas de un motor. Devuelve un resultado por intento; el
/// generador sólo consume los que tienen `success = true`.
pub async fn probe_engine(
    client: &reqwest::Client,
    engine: &EngineDescriptor,
    ctx: &ProbeContext,
) -> Vec<EngineTrial> {
    let mut trials = Vec::new();
    for probe in &engine.probes {
        for value in targets(probe, ctx) {
            let url = fill(&probe.url_template, &value);
            let mut trial = empty_trial(engine, probe, &url);
            let method = if probe.method.eq_ignore_ascii_case("POST") {
                reqwest::Method::POST
            } else {
                reqwest::Method::GET
            };
            let mut request = client.request(method, &url);
            for (name, header_value) in &probe.headers {
                request = request.header(name.as_str(), header_value.as_str());
            }
            if let Some(body_template) = &probe.body_template {
                request = request
                    .header("content-type", "application/json")
                    .body(fill(body_template, &value));
            }

            let response = match request.send().await {
                Ok(response) => response,
                Err(error) => {
                    trial.detail = format!("sin respuesta: {}", error);
                    trials.push(trial);
                    continue;
                }
            };
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if !status.is_success() {
                trial.detail = format!("HTTP {}", status.as_str());
                trials.push(trial);
                continue;
            }

            let (count, sample, note) = match probe.expect.as_str() {
                "json_list" => {
                    let (count, sample) = count_json(&body, probe.list_field.as_deref());
                    (count, sample, String::new())
                }
                "html_items" => count_html(&body, probe.item_selector.as_deref().unwrap_or("")),
                "hls" => count_hls(&body),
                _ => (0, None, "expect desconocido".to_string()),
            };
            trial.success = count >= probe.min_items.max(1);
            trial.content_count = count;
            trial.sample_json = sample;
            trial.detail = if trial.success {
                format!("{} registros", count)
            } else if note.is_empty() {
                format!("sin contenido ({} registros)", count)
            } else {
                note
            };
            let success = trial.success;
            trials.push(trial);
            if success {
                break;
            }
        }
    }
    trials
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_only_declared_markers() {
        assert_eq!(fill("{base}/list", "https://site.test"), "https://site.test/list");
        assert_eq!(fill("{endpoint}", "https://site.test/api"), "https://site.test/api");
    }

    #[test]
    fn counts_a_real_json_list() {
        let body = r#"{"data":{"items":[{"id":1},{"id":2}]}}"#;
        let (count, sample) = count_json(body, Some("items"));
        assert_eq!(count, 2);
        // El generador recibe el array, no el objeto que lo contenía.
        assert!(sample.unwrap().starts_with('['));
    }

    #[test]
    fn rejects_empty_playlists() {
        assert_eq!(count_hls("<html>error</html>").0, 0);
        assert_eq!(count_hls("#EXTM3U\n#EXTINF:4,a.ts\n").0, 1);
    }
}
