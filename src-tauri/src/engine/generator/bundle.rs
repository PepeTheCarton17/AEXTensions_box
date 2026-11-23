use crate::engine::analyzer::AnalysisResult;
use crate::engine::registry::EngineAssets;
use std::collections::HashMap;

use super::anime_source::generate_kotlin_source;
use super::build::{generate_build_gradle, generate_settings_gradle};
use super::filters::generate_kotlin_filters;
use super::manga::{generate_manga_filters, generate_manga_source};
use super::manifest::generate_android_manifest;
use super::naming::{sanitize_class_name, sanitize_display_name, sanitize_pkg_slug};
use super::repo::{generate_github_workflow, generate_repo_index_script};
use super::templates::build_gradle::{GITIGNORE, GRADLE_PROPERTIES, ROOT_BUILD_GRADLE};
use super::templates::gradle_wrapper::{
    GRADLEW_BAT, GRADLEW_SH, GRADLE_WRAPPER_JAR_B64, GRADLE_WRAPPER_PROPERTIES, KEYSTORE_B64,
};
use super::util::decode_base64;

pub struct ExtensionBundle {
    pub pkg_id: String,
    pub class_name: String,
    pub name: String,
    pub lang: String,
    pub ext_type: String,
    pub version: String,
    pub version_code: i32,
    /// Ruta del módulo Gradle dentro del repo, p. ej. `src/es/animeav1`.
    pub module_path: String,
    pub files: HashMap<String, Vec<u8>>,
}

pub struct ExtensionGenerator;

impl ExtensionGenerator {
    pub fn generate(
        analysis: &AnalysisResult,
        custom_name: Option<&str>,
        labels_lang: Option<&str>,
        custom_ext_type: Option<&str>,
    ) -> ExtensionBundle {
        Self::generate_with_version(
            analysis,
            custom_name,
            labels_lang,
            custom_ext_type,
            1,
            "16.1",
            None,
        )
    }

    /// Igual que `generate`, pero incluye los archivos Kotlin de los motores
    /// instalados que el usuario tiene activos en este momento.
    pub fn generate_with_engines(
        analysis: &AnalysisResult,
        custom_name: Option<&str>,
        labels_lang: Option<&str>,
        custom_ext_type: Option<&str>,
        engines: &[EngineAssets],
    ) -> ExtensionBundle {
        Self::generate_with_version_and_engines(
            analysis, custom_name, labels_lang, custom_ext_type, 1, "16.1", None, engines,
        )
    }

    /// `labels_lang` es la elección del usuario y sólo afecta a los textos
    /// visibles (filtros y preferencias). `package_lang` fija la identidad del
    /// paquete al actualizar; si es `None`, la decide el análisis del sitio.
    pub fn generate_with_version(
        analysis: &AnalysisResult,
        custom_name: Option<&str>,
        labels_lang: Option<&str>,
        custom_ext_type: Option<&str>,
        version_code: i32,
        version_name: &str,
        package_lang: Option<&str>,
    ) -> ExtensionBundle {
        Self::generate_with_version_and_engines(
            analysis,
            custom_name,
            labels_lang,
            custom_ext_type,
            version_code,
            version_name,
            package_lang,
            &[],
        )
    }

    pub fn generate_with_version_and_engines(
        analysis: &AnalysisResult,
        custom_name: Option<&str>,
        labels_lang: Option<&str>,
        custom_ext_type: Option<&str>,
        version_code: i32,
        version_name: &str,
        package_lang: Option<&str>,
        engines: &[EngineAssets],
    ) -> ExtensionBundle {
        let raw_name = custom_name
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or(&analysis.name);
        let class_name = sanitize_class_name(raw_name);
        let pkg_slug = sanitize_pkg_slug(raw_name);
        // La identidad del paquete (`src/<lang>/<slug>`, package id) la decide
        // el análisis del sitio. La elección del usuario NO la cambia: sólo se
        // usa para los textos visibles de filtros y preferencias.
        let lang = package_lang
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_lowercase)
            .unwrap_or_else(|| {
                // Un sitio que declara varios idiomas no pertenece a una sola
                // extensión: se publica como "all". Sólo se fija un idioma
                // cuando el propio sitio declara uno inequívocamente.
                if analysis.site_locales.len() > 1 {
                    return "all".to_string();
                }
                let detected = analysis.detected_language.trim().to_lowercase();
                if !detected.is_empty() {
                    detected
                } else {
                    let fallback = analysis.lang.trim().to_lowercase();
                    if fallback.is_empty() { "all".to_string() } else { fallback }
                }
            });
        let labels_lang = labels_lang
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_lowercase)
            .unwrap_or_else(|| lang.clone());
        let display_name = sanitize_display_name(raw_name);

        // El código de idioma visible puede ser "pt-br", pero un paquete Kotlin
        // no admite guiones: usamos una variante segura para package/paths.
        let pkg_lang_raw: String = lang
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_lowercase();
        let pkg_lang = if pkg_lang_raw.is_empty() {
            "all".to_string()
        } else {
            pkg_lang_raw
        };

        let ext_type = custom_ext_type
            .unwrap_or(&analysis.ext_type)
            .to_lowercase();
        let ext_type = if ext_type == "manga" { "manga".to_string() } else { "anime".to_string() };

        let pkg_prefix = if ext_type == "manga" {
            "eu.kanade.tachiyomi.extension"
        } else {
            "eu.kanade.tachiyomi.animeextension"
        };
        let pkg_id = format!("{}.{}.{}", pkg_prefix, pkg_lang, pkg_slug);
        let source_dir = pkg_prefix.replace('.', "/");
        // Estructura multi-módulo habitual en los repos de extensiones de
        // Aniyomi/Nekoyomi: cada extensión es un módulo Gradle propio.
        let module_path = format!("src/{}/{}", pkg_lang, pkg_slug);

        let mut files = HashMap::new();

        // 1. Archivo Kotlin Principal
        let kt_path = format!(
            "{}/src/main/java/{}/{}/{}.kt",
            module_path, source_dir, pkg_lang, class_name
        );
        let (kt_content, filters_content) = if ext_type == "manga" {
            (
                generate_manga_source(
                    analysis,
                    &display_name,
                    &class_name,
                    pkg_slug.as_str(),
                    &labels_lang,
                    &pkg_lang,
                    pkg_prefix,
                ),
                generate_manga_filters(&class_name, &pkg_slug, &pkg_lang, pkg_prefix),
            )
        } else {
            (
                generate_kotlin_source(
                    analysis,
                    &display_name,
                    &class_name,
                    &pkg_slug,
                    &labels_lang,
                    &pkg_lang,
                    pkg_prefix,
                ),
                generate_kotlin_filters(
                    &class_name,
                    &pkg_slug,
                    &labels_lang,
                    &pkg_lang,
                    pkg_prefix,
                    analysis,
                ),
            )
        };
        files.insert(kt_path, kt_content.into_bytes());

        // 1b. Contrato, registro y código de los motores activos. Son datos
        // declarados por repositorios externos; AEXT no decide su contenido.
        // El contrato de streams sólo existe en módulos de anime: un módulo
        // manga no tiene `Video` ni las dependencias del reproductor.
        if ext_type == "anime" {
            for (path, content) in super::engine_runtime::bundle_files(&module_path, engines) {
                files.insert(path, content);
            }
        }

        // 2. Archivo Kotlin de Filtros
        let filters_path = format!(
            "{}/src/main/java/{}/{}/{}Filters.kt",
            module_path, source_dir, pkg_lang, class_name
        );
        files.insert(filters_path, filters_content.into_bytes());

        // 3. AndroidManifest.xml (dentro del módulo)
        let manifest_content = generate_android_manifest(&pkg_id, &class_name, &display_name, &ext_type);
        files.insert(
            format!("{}/src/main/AndroidManifest.xml", module_path),
            manifest_content.into_bytes(),
        );

        // 4. build.gradle.kts del módulo
        let gradle_content = generate_build_gradle(
            &pkg_id,
            version_code,
            version_name,
            &pkg_lang,
            &pkg_slug,
            &ext_type,
        );
        files.insert(
            format!("{}/build.gradle.kts", module_path),
            gradle_content.into_bytes(),
        );

        // 5. Archivos raíz del proyecto Gradle (sin esto, Actions no compila).
        files.insert(
            "build.gradle.kts".to_string(),
            ROOT_BUILD_GRADLE.as_bytes().to_vec(),
        );
        files.insert(
            "settings.gradle.kts".to_string(),
            generate_settings_gradle(&pkg_lang, &pkg_slug).into_bytes(),
        );
        files.insert(
            "gradle.properties".to_string(),
            GRADLE_PROPERTIES.as_bytes().to_vec(),
        );
        files.insert(
            ".gitignore".to_string(),
            GITIGNORE.as_bytes().to_vec(),
        );

        // 5b. Wrapper fijado a Gradle 9.5.1. Sin esto, la compilación local usa
        // cualquier Gradle que haya en el PATH y la versión puede no coincidir
        // con la que soporta el AGP/Kotlin generados.
        files.insert("gradlew".to_string(), GRADLEW_SH.as_bytes().to_vec());
        files.insert("gradlew.bat".to_string(), GRADLEW_BAT.as_bytes().to_vec());
        files.insert(
            "gradle/wrapper/gradle-wrapper.properties".to_string(),
            GRADLE_WRAPPER_PROPERTIES.as_bytes().to_vec(),
        );
        files.insert(
            "gradle/wrapper/gradle-wrapper.jar".to_string(),
            decode_base64(GRADLE_WRAPPER_JAR_B64),
        );

        // 6. GitHub Actions workflow
        let workflow_content = generate_github_workflow();
        files.insert(".github/workflows/build_push.yml".to_string(), workflow_content.into_bytes());

        // 7. Generador de Index del Repo (generate-repo-index.py)
        let index_script = generate_repo_index_script();
        files.insert(".github/scripts/generate-repo-index.py".to_string(), index_script.into_bytes());

        // 8. Keystore permanente determinista (signingkey.jks) para evitar "Firma diferente"
        //    Es una clave propia de AEXT, autofirmada y pública por diseño: vive en el
        //    repositorio generado para que las actualizaciones conserven la firma.
        let keystore_bytes = decode_base64(KEYSTORE_B64);
        files.insert("signingkey.jks".to_string(), keystore_bytes);

        // AEXT NO genera README.md: el README pertenece al repositorio y puede
        // contener la URL, el disclaimer y las notas del usuario. Sobrescribirlo
        // en cada extensión (y poner el nombre de la generación en el título)
        // destruía su contenido. El índice gh-pages/index.min.json ya aporta los
        // enlaces de instalación; no hace falta un README por extensión.

        ExtensionBundle {
            pkg_id,
            class_name,
            name: display_name,
            lang: lang.to_string(),
            ext_type: ext_type.clone(),
            version: version_name.to_string(),
            version_code,
            module_path,
            files,
        }
    }
}
