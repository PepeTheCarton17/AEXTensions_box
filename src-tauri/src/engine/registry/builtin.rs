//! Motores comunes incluidos en AEXT.
//!
//! Son descriptores genéricos: declaran capacidades y sondas sobre datos ya
//! detectados en el sitio. No contienen dominios, rutas de proveedor ni APIs
//! concretas. Los motores especializados se instalan desde un repositorio.

use super::model::{EngineDescriptor, EngineKind, EngineProbe, EngineSource};

fn probe(id: &str, template: &str, expect: &str) -> EngineProbe {
    EngineProbe {
        id: id.to_string(),
        method: "GET".to_string(),
        url_template: template.to_string(),
        expect: expect.to_string(),
        min_items: 1,
        ..Default::default()
    }
}

pub fn builtin_engines() -> Vec<EngineDescriptor> {
    vec![
        EngineDescriptor {
            id: "builtin-public-api".to_string(),
            name: "API pública detectada".to_string(),
            kind: EngineKind::Catalog,
            capabilities: vec![
                "public-api".to_string(),
                "json-catalog".to_string(),
                "catalog-missing".to_string(),
                "session-bootstrap".to_string(),
            ],
            source: EngineSource::Builtin,
            license: "AEXT built-in".to_string(),
            provenance: "builtin:aext".to_string(),
            probes: vec![probe("public-json", "{endpoint}", "json_list")],
            ..Default::default()
        },
        EngineDescriptor {
            id: "builtin-stream-probe".to_string(),
            name: "Sonda de stream público".to_string(),
            kind: EngineKind::Playback,
            capabilities: vec![
                "hls".to_string(),
                "hls-master".to_string(),
                "stream-probe".to_string(),
            ],
            source: EngineSource::Builtin,
            license: "AEXT built-in".to_string(),
            provenance: "builtin:aext".to_string(),
            probes: vec![probe("public-hls", "{stream}", "hls")],
            ..Default::default()
        },
    ]
}
