//! Carga de los archivos Kotlin que un motor instalado declaró en su manifiesto.
//!
//! Los archivos viven en `<storage>/engines/<id>/`, fuera del árbol que se
//! borra al actualizar una extensión. AEXT sólo los lee; no interpreta su
//! contenido ni introduce rutas propias del motor en el generador.

use super::model::EngineDescriptor;
use super::store;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineAssets {
    pub id: String,
    pub package: String,
    pub entry_class: String,
    /// (ruta declarada, contenido). La ruta se usa sólo para dar un nombre
    /// único al archivo dentro del módulo generado.
    pub files: Vec<(String, Vec<u8>)>,
}

fn safe_relative(file: &str) -> Option<PathBuf> {
    let path = Path::new(file);
    if file.is_empty()
        || path.is_absolute()
        || path.components().any(|c| c.as_os_str() == "..")
    {
        return None;
    }
    Some(path.to_path_buf())
}

/// Devuelve los motores activos con un contrato utilizable. Si un archivo no
/// se puede leer, se omite ese motor entero: nunca se genera un bundle que
/// referencie una clase ausente.
pub fn collect(storage_dir: &Path, engines: &[EngineDescriptor]) -> Vec<EngineAssets> {
    let mut out = Vec::new();
    for engine in engines {
        if !engine.enabled {
            continue;
        }
        let Some(runtime) = &engine.runtime else { continue };
        let dir = store::install_dir(storage_dir, &engine.id);
        let mut files = Vec::new();
        for relative in &engine.files {
            if !relative.ends_with(".kt") {
                continue;
            }
            let Some(safe) = safe_relative(relative) else { continue };
            let Ok(content) = fs::read(dir.join(safe)) else {
                files.clear();
                break;
            };
            files.push((relative.clone(), content));
        }
        if files.is_empty() {
            continue;
        }
        out.push(EngineAssets {
            id: engine.id.clone(),
            package: runtime.package.clone(),
            entry_class: runtime.entry_class.clone(),
            files,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::registry::{EngineKind, EngineRuntime};

    fn engine() -> EngineDescriptor {
        EngineDescriptor {
            id: "demo-engine".to_string(),
            name: "Demo".to_string(),
            kind: EngineKind::Playback,
            files: vec!["src/DemoResolver.kt".to_string()],
            runtime: Some(EngineRuntime {
                api: crate::engine::registry::STREAM_RESOLVER_API.to_string(),
                package: "com.example.engine".to_string(),
                entry_class: "DemoResolver".to_string(),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn ignores_engines_without_a_runtime() {
        let mut descriptor = engine();
        descriptor.runtime = None;
        let dir = std::env::temp_dir().join("aext-assets-no-runtime");
        assert!(collect(&dir, &[descriptor]).is_empty());
    }

    #[test]
    fn rejects_unsafe_relative_paths() {
        assert!(safe_relative("../secret.kt").is_none());
        assert!(safe_relative("/etc/passwd").is_none());
        assert!(safe_relative("src/DemoResolver.kt").is_some());
    }
}
