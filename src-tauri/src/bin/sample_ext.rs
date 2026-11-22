//! Genera proyectos de extensión de ejemplo para comprobar que el código
//! Kotlin generado compila de verdad.
//!
//! Uso: `cargo run --bin sample_ext -- <directorio-de-salida>`
//!
//! Escribe cinco proyectos independientes (anime con selector de episodios,
//! anime genérico, catálogo JSON-LD, SPA multilingüe y manga) con sus iconos, para que el CI pueda ejecutar
//! `gradle assembleRelease` sobre ellos. Sin este paso, un error de compilación
//! en las plantillas solo se descubría al usar la extensión en Nekoyomi.

use aext_lib::engine::analyzer::{AnalysisResult, CatalogRecord, SiteCapabilities, SiteFilterParam, SiteSearchAction};
use aext_lib::engine::generator::ExtensionGenerator;
use image::{Rgba, RgbaImage};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn anime_fixture(name: &str, host: &str, episode_selector: &str, catalog: bool) -> AnalysisResult {
    AnalysisResult {
        url: format!("https://{host}"),
        entry_url: format!("https://{host}/"),
        name: name.to_string(),
        lang: "es".to_string(),
        ext_type: "anime".to_string(),
        cms: "generic".to_string(),
        favicon_url: None,
        detected_hosts: vec![host.to_string()],
        has_sub_dub: catalog,
        protections: Vec::new(),
        card_selector: "article, div.item, div.card".to_string(),
        title_selector: "h2, h3, .title".to_string(),
        next_page_selector: "a.next, .pagination a:last-child".to_string(),
        popular_path: catalog.then(|| "/directorio".to_string()),
        latest_path: None,
        search_path: catalog.then(|| "/directorio".to_string()),
        search_param: catalog.then(|| "q".to_string()),
        page_style: if catalog { "path" } else { "none" }.to_string(),
        catalog_detected: catalog,
        episode_selector: episode_selector.to_string(),
        sample_items: Vec::new(),
        warnings: Vec::new(),
        // Un fixture con capacidades y otro sin ellas: el caso vacío detecta
        // regressions al generar `setOf()` sin tipo inferible.
        capabilities: SiteCapabilities {
            stream_formats: if catalog { vec!["hls".to_string()] } else { Vec::new() },
            qualities: if catalog { vec!["720p".to_string()] } else { Vec::new() },
            player_cases: if catalog {
                vec!["hls-master".to_string(), "ads-hls".to_string()]
            } else {
                Vec::new()
            },
            api_endpoints: if catalog {
                vec!["https://media.example.com/hls/master.m3u8".to_string()]
            } else {
                Vec::new()
            },
            // Un fixture con SUB verificado y otro sin nada verificado: el CI
            // debe compilar ambos caminos del generador (con y sin AudioFilter).
            master_verified: catalog,
            sub_verified: catalog,
            dub_verified: false,
            playback_methods: if catalog {
                vec!["HLS".to_string()]
            } else {
                Vec::new()
            },
            ..Default::default()
        },
        ..Default::default()
    }
}

/// Catálogo SPA sin tarjetas HTML: el generador debe compilar usando los
/// títulos y la búsqueda declarados en JSON-LD, y los filtros reales.
fn jsonld_fixture() -> AnalysisResult {
    AnalysisResult {
        url: "https://catalog.example".to_string(),
        entry_url: "https://catalog.example/en/catalog".to_string(),
        name: "Catalogo Struct".to_string(),
        lang: "es".to_string(),
        ext_type: "anime".to_string(),
        cms: "generic".to_string(),
        card_selector: "article".to_string(),
        title_selector: "h2".to_string(),
        catalog_source: "jsonld".to_string(),
        catalog_records: vec![
            CatalogRecord {
                title: "Titulo original".to_string(),
                alternate_title: Some("English fallback title".to_string()),
                language: Some("uz".to_string()),
                url: "https://catalog.example/en/cinema/one".to_string(),
                image: Some("https://catalog.example/poster.jpg".to_string()),
            },
            CatalogRecord {
                title: "Segundo titulo".to_string(),
                alternate_title: None,
                language: Some("uz".to_string()),
                url: "https://catalog.example/en/cinema/two".to_string(),
                image: None,
            },
        ],
        search_action: Some(SiteSearchAction {
            url_template: "https://catalog.example/en/catalog?name={search_term_string}".to_string(),
            query_param: Some("name".to_string()),
        }),
        filter_params: vec![
            SiteFilterParam { name: "Genre".to_string(), param: "genreIds".to_string(), value: "1".to_string(), kind: "check".to_string() },
            SiteFilterParam { name: "Genre".to_string(), param: "genreIds".to_string(), value: "2".to_string(), kind: "check".to_string() },
        ],
        detected_language: "en".to_string(),
        popular_path: Some("/en/catalog".to_string()),
        search_path: Some("/en/catalog".to_string()),
        search_param: Some("name".to_string()),
        warnings: vec!["Fixture JSON-LD para CI".to_string()],
        ..Default::default()
    }
}

/// SPA multilingüe: el sitio declara tres locales y expone filtros en un
/// bundle. Aunque la UI pidiera `es`, el paquete debe ser `all` y el Kotlin
/// debe compilar con selectores de valores y filtros de texto libre.
fn spa_fixture() -> AnalysisResult {
    AnalysisResult {
        url: "https://spa.example".to_string(),
        entry_url: "https://spa.example/catalog".to_string(),
        name: "SPA Multilingue".to_string(),
        lang: "es".to_string(),
        ext_type: "anime".to_string(),
        cms: "generic".to_string(),
        card_selector: "article".to_string(),
        title_selector: "h3".to_string(),
        catalog_source: "jsonld".to_string(),
        catalog_records: vec![CatalogRecord {
            title: "Titulo real".to_string(),
            alternate_title: Some("Real title".to_string()),
            language: Some("uz".to_string()),
            url: "https://spa.example/cinema/one".to_string(),
            image: None,
        }],
        search_action: Some(SiteSearchAction {
            url_template: "https://spa.example/catalog?name={search_term_string}".to_string(),
            query_param: Some("name".to_string()),
        }),
        filter_params: vec![
            SiteFilterParam { name: "Sort By".to_string(), param: "sortBy".to_string(), value: "publish_date".to_string(), kind: "select".to_string() },
            SiteFilterParam { name: "Sort By".to_string(), param: "sortBy".to_string(), value: "imdb_rating".to_string(), kind: "select".to_string() },
            SiteFilterParam { name: "Status".to_string(), param: "status".to_string(), value: "release".to_string(), kind: "select".to_string() },
            SiteFilterParam { name: "Country".to_string(), param: "countryIds".to_string(), value: String::new(), kind: "text".to_string() },
        ],
        detected_language: "uz".to_string(),
        site_locales: vec!["uz".to_string(), "ru".to_string(), "en".to_string()],
        popular_path: Some("/catalog".to_string()),
        search_path: Some("/catalog".to_string()),
        search_param: Some("name".to_string()),
        warnings: vec!["Fixture SPA multilingüe para CI".to_string()],
        ..Default::default()
    }
}

fn manga_fixture() -> AnalysisResult {
    AnalysisResult {
        url: "https://manga.example".to_string(),
        entry_url: "https://manga.example/".to_string(),
        name: "Manga Ejemplo".to_string(),
        lang: "en".to_string(),
        ext_type: "manga".to_string(),
        cms: "madara".to_string(),
        favicon_url: None,
        detected_hosts: vec!["manga.example".to_string()],
        has_sub_dub: false,
        protections: Vec::new(),
        card_selector: "div.page-item-detail, div.manga".to_string(),
        title_selector: "h3 a, .post-title h3".to_string(),
        next_page_selector: "a.next.page-numbers".to_string(),
        popular_path: Some("/manga".to_string()),
        latest_path: Some("/manga".to_string()),
        search_path: Some("/manga".to_string()),
        search_param: Some("s".to_string()),
        page_style: "path".to_string(),
        catalog_detected: true,
        episode_selector: String::new(),
        sample_items: Vec::new(),
        warnings: Vec::new(),
        capabilities: SiteCapabilities::default(),
        ..Default::default()
    }
}

fn icon(size: u32) -> Vec<u8> {
    let mut image = RgbaImage::from_pixel(size, size, Rgba([124, 58, 237, 255]));
    let center = size / 2;
    let radius = size / 4;
    for y in center.saturating_sub(radius)..(center + radius).min(size) {
        for x in center.saturating_sub(radius)..(center + radius).min(size) {
            image.put_pixel(x, y, Rgba([255, 255, 255, 255]));
        }
    }
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("PNG en memoria");
    bytes
}

fn write_project(
    root: &Path,
    analysis: &AnalysisResult,
    lang: &str,
    ext_type: &str,
) -> Result<PathBuf, String> {
    let bundle = ExtensionGenerator::generate_with_version(
        analysis,
        None,
        Some(lang),
        Some(ext_type),
        1,
        "16.1",
        Some(lang),
    );

    let mut files: HashMap<String, Vec<u8>> = bundle.files.clone();
    let png = icon(192);
    files.insert("icon/ic_launcher.png".to_string(), png.clone());
    files.insert(format!("icon/{}.png", bundle.pkg_id), png);
    for (density, size) in [
        ("mipmap-hdpi", 72u32),
        ("mipmap-xhdpi", 96),
        ("mipmap-xxhdpi", 144),
        ("mipmap-xxxhdpi", 192),
    ] {
        files.insert(
            format!(
                "{}/src/main/res/{}/ic_launcher.png",
                bundle.module_path, density
            ),
            icon(size),
        );
    }

    for (rel_path, bytes) in files {
        let target = root.join(rel_path.replace('/', std::path::MAIN_SEPARATOR_STR));
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&target, bytes).map_err(|e| e.to_string())?;
    }

    println!(
        "ok  {} -> {}",
        bundle.pkg_id,
        root.display()
    );
    Ok(root.to_path_buf())
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("uso: sample_ext <directorio-de-salida>");
        std::process::exit(2);
    });
    let out = PathBuf::from(out);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("crear directorio de salida");

    let projects = [
        (
            "anime-verificado",
            anime_fixture("Anime Verificado", "anime.example", "a[href*='/episode/']", true),
            "es",
            "anime",
        ),
        (
            "anime-generico",
            anime_fixture("Anime Generico", "generico.example", "", false),
            "en",
            "anime",
        ),
        ("catalogo-jsonld", jsonld_fixture(), "es", "anime"),
        ("spa-multilingue", spa_fixture(), "es", "anime"),
        ("manga", manga_fixture(), "en", "manga"),
    ];

    for (dir, analysis, lang, ext_type) in projects {
        let root = out.join(dir);
        std::fs::create_dir_all(&root).expect("crear proyecto");
        if let Err(error) = write_project(&root, &analysis, lang, ext_type) {
            eprintln!("error generando {dir}: {error}");
            std::process::exit(1);
        }
        println!("generado {}", root.display());
    }
}
