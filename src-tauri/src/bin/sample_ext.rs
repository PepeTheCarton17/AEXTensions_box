//! Genera proyectos de extensión de ejemplo para comprobar que el código
//! Kotlin generado compila de verdad.
//!
//! Uso: `cargo run --bin sample_ext -- <directorio-de-salida>`
//!
//! Escribe tres proyectos independientes (anime con selector de episodios,
//! anime genérico y manga) con sus iconos, para que el CI pueda ejecutar
//! `gradle assembleRelease` sobre ellos. Sin este paso, un error de compilación
//! en las plantillas solo se descubría al usar la extensión en Nekoyomi.

use aext_lib::engine::analyzer::{AnalysisResult, SiteCapabilities};
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
        capabilities: SiteCapabilities {
            stream_formats: vec!["hls".to_string()],
            qualities: vec!["720p".to_string()],
            ..Default::default()
        },
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
