//! Generador de extensiones: ensambla los archivos de un módulo Gradle.
//!
//! Cada submódulo tiene una sola responsabilidad. Las plantillas de código
//! generado viven en `templates/` y se exponen como `&str` y como macro para
//! que `format!` reciba un literal (requisito del compilador Rust).

mod anime_source;
mod api_helpers;
mod build;
mod bundle;
mod filters;
mod i18n;
mod manga;
mod manifest;
mod naming;
mod repo;
mod templates;
mod util;

#[cfg(test)]
mod tests;

pub use bundle::{ExtensionBundle, ExtensionGenerator};
pub use repo::generate_repo_readme;
pub use util::decode_base64;
pub use templates::build_gradle::ROOT_BUILD_GRADLE;
pub use templates::gradle_wrapper::{
    GRADLEW_BAT, GRADLEW_SH, GRADLE_WRAPPER_JAR_B64, GRADLE_WRAPPER_PROPERTIES,
};

// Re-exports internos: los tests viven en `generator::tests` y las funciones
// se reparten entre submódulos.
pub(crate) use anime_source::generate_kotlin_source;
pub(crate) use filters::generate_kotlin_filters;
pub(crate) use templates::video_engine::VIDEO_ENGINE_KT;
