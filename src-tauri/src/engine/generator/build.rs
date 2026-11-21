use super::templates::build_gradle::{build_gradle_kt_fmt, settings_gradle_kt_fmt};

pub(crate) fn generate_build_gradle(
    pkg_id: &str,
    version_code: i32,
    version_name: &str,
    pkg_lang: &str,
    pkg_slug: &str,
    ext_type: &str,
) -> String {
    // extensions-lib de Aniyomi solo trae `animesource`. Los scrapers de manga
    // usan la API `eu.kanade.tachiyomi.source` de Mihon/Tachiyomi, que está en
    // la librería original; sin esto el proyecto de manga no compila.
    let extensions_lib = if ext_type == "manga" {
        "com.github.tachiyomiorg:extensions-lib:1.4.4"
    } else {
        "com.github.aniyomiorg:extensions-lib:14"
    };
    format!(
        build_gradle_kt_fmt(),
        extensions_lib = extensions_lib,
        pkg_id = pkg_id,
        version_code = version_code,
        version_name = version_name,
        pkg_lang = pkg_lang,
        pkg_slug = pkg_slug,
    )
}

pub(crate) fn generate_settings_gradle(pkg_lang: &str, pkg_slug: &str) -> String {
    format!(settings_gradle_kt_fmt(), pkg_lang = pkg_lang, pkg_slug = pkg_slug)
}
