use crate::engine::analyzer::{AnalysisResult, ApiListResult, SiteCapabilities};
use std::collections::HashMap;

use super::filters::humanize_filter_value;

/// Devuelve (mostrar_selector, sub_verificado, dub_verificado). Sin un máster
/// HLS abierto por AEXT no se genera ningún control SUB/DUB: el DOM o un texto
/// "DUB" en el HTML no es evidencia de una pista real.
pub(crate) fn verified_audio_selection(caps: &SiteCapabilities) -> (bool, bool, bool) {
    let sub = caps.master_verified && caps.sub_verified;
    let dub = caps.master_verified && caps.dub_verified;
    (sub || dub, sub, dub)
}

/// Normaliza `genres`/`genreIds`/`category_id` a `genre`/`category` para poder
/// emparejar parámetros del sitio con recursos de su propia API REST.
pub(crate) fn api_stem(value: &str) -> String {
    let v = value.trim().to_lowercase();
    if v.ends_with("ies") && v.len() > 4 {
        format!("{}y", &v[..v.len() - 3])
    } else if v.ends_with('s') && !v.ends_with("ss") && v.len() > 3 {
        v[..v.len() - 1].to_string()
    } else {
        v
    }
}

pub(crate) fn api_field_stem(field: &str) -> String {
    let trimmed = field
        .trim_end_matches("_ids")
        .trim_end_matches("_id")
        .trim_end_matches("Ids")
        .trim_end_matches("Id");
    api_stem(trimmed)
}

/// Listas `{id,name}` verificadas que corresponden a un parámetro del sitio.
pub(crate) fn api_option_lists<'a>(analysis: &'a AnalysisResult, param: &str) -> Vec<&'a ApiListResult> {
    let key = api_field_stem(param);
    if key.is_empty() {
        return Vec::new();
    }
    analysis
        .api_lists
        .iter()
        .filter(|api| api.options.len() >= 2 && api_stem(&api.resource) == key)
        .collect()
}

/// Metadatos que un sitio publica como lista y que se pueden excluir con
/// evidencia real. No hay géneros anime prefijados: si el sitio no los expone,
/// no hay filtro.
pub(crate) fn api_exclusion_lists(analysis: &AnalysisResult) -> Vec<&ApiListResult> {
    const EXCLUDABLE: &[&str] = &["genre", "category", "country", "tag", "language"];
    analysis
        .api_lists
        .iter()
        .filter(|api| {
            !api.options.is_empty() && EXCLUDABLE.contains(&api_stem(&api.resource).as_str())
        })
        .collect()
}

/// Pares `(etiqueta, id)` reales que pueden excluirse, con la etiqueta
/// calificada por su recurso (`Genre: Comedy`) para que ids repetidos en
/// distintas listas no se mezclen.
pub(crate) fn api_exclusion_pairs(analysis: &AnalysisResult) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    for api in api_exclusion_lists(analysis) {
        let resource = humanize_filter_value(&api_stem(&api.resource));
        for option in &api.options {
            let label = format!("{}: {}", resource, option.label);
            if !pairs.iter().any(|(l, v)| l == &label && v == &option.value) {
                pairs.push((label, option.value.clone()));
            }
        }
    }
    pairs.truncate(80);
    pairs
}

/// Segmento usado por el sitio para la ficha (`/cinema/<slug>`, `/anime/<slug>`).
/// Se deduce de las URLs reales ya detectadas, nunca se inventa.
pub(crate) fn detail_segment(analysis: &AnalysisResult) -> Option<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for record in &analysis.catalog_records {
        if let Ok(url) = url::Url::parse(&record.url) {
            let segments: Vec<&str> = url
                .path_segments()
                .map(|s| s.filter(|p| !p.is_empty()).collect())
                .unwrap_or_default();
            if segments.len() >= 2 {
                *counts.entry(segments[segments.len() - 2].to_string()).or_insert(0) += 1;
            }
        }
    }
    counts.into_iter().max_by_key(|(_, count)| *count).map(|(segment, _)| segment)
}

pub(crate) fn api_string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    let obj = value.as_object()?;
    obj.get(key).and_then(|v| match v {
        serde_json::Value::String(s) => Some(s.trim().to_string()).filter(|s| !s.is_empty()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

/// Traduce los ids de metadatos presentes en cada registro a los nombres reales
/// devueltos por la API del propio sitio (`category_id: 3` -> `Series`). Sólo
/// empareja campos con el mismo recurso: `genre_ids` con `/genres`, nunca con
/// `/categories`, aunque ambos usen ids numéricos.
pub(crate) fn api_record_genres(
    value: &serde_json::Value,
    analysis: &AnalysisResult,
) -> Option<String> {
    let obj = value.as_object()?;
    let mut labels: Vec<String> = Vec::new();
    for (field, raw) in obj {
        let stem = api_field_stem(field);
        if stem.is_empty() {
            continue;
        }
        let scalars: Vec<String> = match raw {
            serde_json::Value::String(s) => vec![s.trim().to_string()],
            serde_json::Value::Number(n) => vec![n.to_string()],
            serde_json::Value::Array(items) => items
                .iter()
                .filter_map(|v| match v {
                    serde_json::Value::String(s) => Some(s.trim().to_string()),
                    serde_json::Value::Number(n) => Some(n.to_string()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        for api in api_exclusion_lists(analysis) {
            if api_stem(&api.resource) != stem {
                continue;
            }
            for scalar in &scalars {
                if let Some(option) = api.options.iter().find(|o| &o.value == scalar) {
                    if !labels.iter().any(|l| l == &option.label) {
                        labels.push(option.label.clone());
                    }
                }
            }
        }
    }
    if labels.is_empty() {
        None
    } else {
        Some(labels.join(", "))
    }
}

pub(crate) struct ApiCatalogRecord {
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) image: Option<String>,
    pub(crate) genre: Option<String>,
}

/// Registros reales devueltos por un endpoint de catálogo verificado. Sólo se
/// usan listas que parecen catálogo por su recurso y que traen título; un array
/// de opciones (`/genres`) nunca se convierte en tarjetas.
pub(crate) fn api_catalog_records(analysis: &AnalysisResult) -> Vec<ApiCatalogRecord> {
    const CATALOG: &[&str] = &["content", "movie", "film", "serie", "show", "catalog", "title"];
    let segment = detail_segment(analysis);
    // La ficha se construye sobre el origen, no sobre la URL analizada: si el
    // usuario abrió `/en`, esa ruta de idioma no debe colarse en el enlace.
    let origin = url::Url::parse(&analysis.url)
        .map(|u| u.origin().ascii_serialization())
        .unwrap_or_else(|_| analysis.url.trim_end_matches('/').to_string());
    let mut out: Vec<ApiCatalogRecord> = Vec::new();
    for api in &analysis.api_lists {
        if api.title_field.is_empty() || !CATALOG.contains(&api_stem(&api.resource).as_str()) {
            continue;
        }
        let Ok(serde_json::Value::Array(records)) = serde_json::from_str(&api.sample_json) else {
            continue;
        };
        for record in records {
            let Some(title) = api_string_field(&record, &api.title_field) else {
                continue;
            };
            let slug = api_string_field(&record, &api.url_field).unwrap_or_default();
            let url = if slug.starts_with("http") {
                slug
            } else if let Some(segment) = &segment {
                if slug.is_empty() {
                    continue;
                }
                format!("{}/{}/{}", origin.trim_end_matches('/'), segment, slug)
            } else {
                continue;
            };
            let image = api_string_field(&record, &api.image_field);
            let genre = api_record_genres(&record, analysis);
            if !out.iter().any(|old| old.title.eq_ignore_ascii_case(&title)) {
                out.push(ApiCatalogRecord { title, url, image, genre });
            }
        }
    }
    out
}
