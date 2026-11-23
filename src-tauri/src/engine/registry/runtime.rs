//! Contrato de ejecución declarado por un motor instalable.
//!
//! AEXT no conoce el código del motor: sólo valida que el manifiesto nombre
//! una API pública, un paquete Kotlin, una clase de entrada y al menos un
//! archivo `.kt`. La lógica concreta (clientes, endpoints, tokens de visitante)
//! vive en el repo del motor y nunca se copia al generador.

use super::model::EngineKind;
use serde::{Deserialize, Serialize};

/// Única versión del contrato que el bundle generado sabe alojar.
pub const STREAM_RESOLVER_API: &str = "aext-stream-resolver/1";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineRuntime {
    #[serde(default)]
    pub api: String,
    #[serde(default)]
    pub package: String,
    #[serde(default)]
    pub entry_class: String,
}

impl EngineRuntime {
    pub fn validate(&self, files: &[String], kind: &EngineKind) -> Result<(), String> {
        if self.api != STREAM_RESOLVER_API {
            return Err(format!(
                "runtime.api '{}' no soportada (se espera {})",
                self.api, STREAM_RESOLVER_API
            ));
        }
        if !valid_package(&self.package) {
            return Err("runtime.package debe ser un paquete Kotlin válido".to_string());
        }
        if !valid_class(&self.entry_class) {
            return Err("runtime.entry_class debe ser un identificador Kotlin válido".to_string());
        }
        if *kind == EngineKind::Catalog {
            return Err("un motor de catálogo no puede declarar resolutor de stream".to_string());
        }
        if !files.iter().any(|file| file.ends_with(".kt")) {
            return Err("runtime necesita al menos un archivo .kt en files".to_string());
        }
        Ok(())
    }
}

fn valid_package(value: &str) -> bool {
    let segments: Vec<&str> = value.split('.').collect();
    segments.len() >= 2
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && segment.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        })
}

fn valid_class(value: &str) -> bool {
    !value.is_empty()
        && value.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> EngineRuntime {
        EngineRuntime {
            api: STREAM_RESOLVER_API.to_string(),
            package: "com.example.engine".to_string(),
            entry_class: "DemoResolver".to_string(),
        }
    }

    fn files() -> Vec<String> {
        vec!["src/DemoResolver.kt".to_string()]
    }

    #[test]
    fn accepts_a_declared_stream_resolver() {
        assert!(runtime().validate(&files(), &EngineKind::Playback).is_ok());
        assert!(runtime().validate(&files(), &EngineKind::Extractor).is_ok());
    }

    #[test]
    fn rejects_unknown_api_version() {
        let mut value = runtime();
        value.api = "aext-stream-resolver/2".to_string();
        assert!(value.validate(&files(), &EngineKind::Playback).is_err());
    }

    #[test]
    fn rejects_catalog_engines_without_a_stream_contract() {
        assert!(runtime().validate(&files(), &EngineKind::Catalog).is_err());
    }

    #[test]
    fn rejects_missing_kotlin_sources() {
        assert!(runtime().validate(&[], &EngineKind::Playback).is_err());
    }

    #[test]
    fn rejects_unsafe_package_and_class_names() {
        let mut value = runtime();
        value.package = "com..engine".to_string();
        assert!(value.validate(&files(), &EngineKind::Playback).is_err());
        let mut value = runtime();
        value.entry_class = "Demo.Resolver".to_string();
        assert!(value.validate(&files(), &EngineKind::Playback).is_err());
    }
}
