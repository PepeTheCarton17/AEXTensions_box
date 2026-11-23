//! Contrato Kotlin y registro generado para motores instalables.
//!
//! AEXT emite una interfaz pública, un registro con las clases declaradas y
//! copia los `.kt` del motor al módulo. No conoce su lógica, sus endpoints ni
//! sus clientes: sólo une las piezas con un contrato estable.

use crate::engine::registry::EngineAssets;
use std::path::Path;

pub const CONTRACT_PACKAGE: &str = "eu.kanade.tachiyomi.aext";

pub fn contract_source() -> &'static str {
    r#"package eu.kanade.tachiyomi.aext

import eu.kanade.tachiyomi.animesource.model.Video
import okhttp3.Headers
import okhttp3.OkHttpClient

/** Datos reales de la pagina. Sin cookies ni sesion del usuario. */
data class AextVideoContext(
    val url: String,
    val html: String,
    val endpoints: List<String>,
    val streams: List<String>,
    val client: OkHttpClient,
    val headers: Headers,
)

/** Contrato publico implementado por los motores instalables. */
interface AextStreamResolver {
    val id: String
    fun canResolve(ctx: AextVideoContext): Boolean
    fun resolve(ctx: AextVideoContext): List<Video>
}
"#
}

pub fn registry_source(engines: &[EngineAssets]) -> String {
    let mut out = format!("package {}\n\n", CONTRACT_PACKAGE);
    for (index, engine) in engines.iter().enumerate() {
        out.push_str(&format!(
            "import {}.{} as AextResolver{}\n",
            engine.package, engine.entry_class, index
        ));
    }
    out.push_str("\nclass AextEngineRegistry {\n    val streamResolvers: List<AextStreamResolver> = listOf(");
    for index in 0..engines.len() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push_str(&format!("AextResolver{}()", index));
    }
    out.push_str(")\n}\n");
    out
}

fn unique_file_name(id: &str, relative: &str) -> String {
    let base = Path::new(relative)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Resolver.kt");
    let prefix: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{}_{}", prefix, base)
}

/// (ruta dentro del bundle, contenido) para el contrato, el registro y los
/// archivos declarados por cada motor activo.
pub fn bundle_files(module_path: &str, engines: &[EngineAssets]) -> Vec<(String, Vec<u8>)> {
    let contract_dir = format!(
        "{}/src/main/java/{}",
        module_path,
        CONTRACT_PACKAGE.replace('.', "/")
    );
    let mut out = vec![
        (
            format!("{}/AextEngineContract.kt", contract_dir),
            contract_source().as_bytes().to_vec(),
        ),
        (
            format!("{}/AextEngineRegistry.kt", contract_dir),
            registry_source(engines).into_bytes(),
        ),
    ];
    for engine in engines {
        let target_dir = format!(
            "{}/src/main/java/{}",
            module_path,
            engine.package.replace('.', "/")
        );
        for (relative, content) in &engine.files {
            out.push((
                format!("{}/{}", target_dir, unique_file_name(&engine.id, relative)),
                content.clone(),
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(id: &str, package: &str, class: &str) -> EngineAssets {
        EngineAssets {
            id: id.to_string(),
            package: package.to_string(),
            entry_class: class.to_string(),
            files: vec![("src/Resolver.kt".to_string(), b"class Resolver".to_vec())],
        }
    }

    #[test]
    fn contract_exposes_only_generic_http_primitives() {
        let contract = contract_source();
        for marker in [
            "interface AextStreamResolver",
            "data class AextVideoContext",
            "val client: OkHttpClient",
            "val headers: Headers",
            "fun resolve(ctx: AextVideoContext): List<Video>",
        ] {
            assert!(contract.contains(marker), "falta en el contrato: {marker}");
        }
    }

    #[test]
    fn registry_uses_aliases_for_declared_entry_classes() {
        let source = registry_source(&[
            engine("a", "com.example.one", "Resolver"),
            engine("b", "com.example.two", "Resolver"),
        ]);
        assert!(source.contains("import com.example.one.Resolver as AextResolver0"));
        assert!(source.contains("import com.example.two.Resolver as AextResolver1"));
        assert!(source.contains("listOf(AextResolver0(), AextResolver1())"));
    }

    #[test]
    fn bundle_includes_contract_registry_and_engine_sources() {
        let files = bundle_files("src/all/demo", &[engine("engine-a", "com.example.one", "Resolver")]);
        let paths: Vec<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
        assert!(paths.contains(&"src/all/demo/src/main/java/eu/kanade/tachiyomi/aext/AextEngineContract.kt"));
        assert!(paths.contains(&"src/all/demo/src/main/java/eu/kanade/tachiyomi/aext/AextEngineRegistry.kt"));
        assert!(paths.contains(&"src/all/demo/src/main/java/com/example/one/engine_a_Resolver.kt"));
    }
}
