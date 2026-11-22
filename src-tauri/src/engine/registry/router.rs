//! Selección de motores por capacidad.
//!
//! La decisión se toma comparando etiquetas declaradas con lo que el análisis
//! observó realmente. No hay tablas de dominios ni nombres de proveedores.

use super::model::{EngineDescriptor, EngineKind, EngineSource};
use crate::engine::analyzer::SiteCapabilities;

const MAX_CANDIDATES: usize = 6;

fn needed_tags(caps: &SiteCapabilities, catalog_missing: bool) -> Vec<String> {
    let mut tags = Vec::new();
    if catalog_missing {
        tags.push("catalog-missing".to_string());
    }
    for case in &caps.player_cases {
        if !tags.contains(case) {
            tags.push(case.clone());
        }
    }
    for format in &caps.stream_formats {
        if !tags.contains(format) {
            tags.push(format.clone());
        }
    }
    tags
}

/// Devuelve los motores más probables, ordenados por coincidencia de
/// capacidades. Sin coincidencias no se fuerza ningún motor: AEXT prefiere
/// decir "sin motor compatible" antes que inventar uno.
pub fn select(
    engines: &[EngineDescriptor],
    caps: &SiteCapabilities,
    catalog_missing: bool,
) -> Vec<EngineDescriptor> {
    let needed = needed_tags(caps, catalog_missing);
    let mut scored: Vec<(i32, usize, EngineDescriptor)> = Vec::new();

    for (index, engine) in engines.iter().enumerate() {
        if !engine.enabled {
            continue;
        }
        let mut score = 0i32;
        for tag in &needed {
            if engine.capabilities.iter().any(|c| c == tag) {
                score += 3;
            }
        }
        if catalog_missing && engine.kind == EngineKind::Catalog {
            score += 2;
        }
        if !catalog_missing && engine.kind == EngineKind::Playback {
            score += 1;
        }
        // A igualdad de capacidades, primero lo que el usuario instaló: el
        // motor común es un punto de partida, no una respuesta definitiva.
        if engine.source == EngineSource::Repo {
            score += 1;
        }
        if score > 0 {
            scored.push((score, index, engine.clone()));
        }
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .take(MAX_CANDIDATES)
        .map(|(_, _, engine)| engine)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(id: &str, capabilities: &[&str], kind: EngineKind) -> EngineDescriptor {
        EngineDescriptor {
            id: id.to_string(),
            kind,
            capabilities: capabilities.iter().map(|s| s.to_string()).collect(),
            enabled: true,
            ..Default::default()
        }
    }

    #[test]
    fn routes_by_capability_without_domains() {
        let engines = vec![
            engine("api", &["catalog-missing", "json-catalog"], EngineKind::Catalog),
            engine("hls", &["hls"], EngineKind::Playback),
        ];
        let caps = SiteCapabilities {
            player_cases: vec!["hls-master".to_string()],
            stream_formats: vec!["hls".to_string()],
            ..Default::default()
        };
        let picked = select(&engines, &caps, false);
        assert_eq!(picked.first().map(|e| e.id.as_str()), Some("hls"));
        let picked = select(&engines, &SiteCapabilities::default(), true);
        assert_eq!(picked.first().map(|e| e.id.as_str()), Some("api"));
    }

    #[test]
    fn unknown_capability_selects_nothing() {
        let engines = vec![engine("api", &["json-catalog"], EngineKind::Catalog)];
        assert!(select(&engines, &SiteCapabilities::default(), false).is_empty());
    }
}
