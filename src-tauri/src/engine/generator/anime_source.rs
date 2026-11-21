use crate::engine::analyzer::{AnalysisResult, SiteAnalyzer};
use std::collections::HashSet;

use super::api_helpers::{api_catalog_records, api_exclusion_pairs, verified_audio_selection};
use super::i18n::get_i18n;
use super::naming::kt_path_literal;
use super::templates::anime_blocks::{
    exclusion_capture_kt_fmt, filter_block_kt_fmt, search_audio_block_kt_fmt,
};
use super::templates::anime_source::anime_source_kt_fmt;
use super::templates::video_engine::VIDEO_ENGINE_KT;

pub(crate) fn generate_kotlin_source(analysis: &AnalysisResult, display_name: &str, class_name: &str, pkg_slug: &str, lang: &str, pkg_lang: &str, pkg_prefix: &str) -> String {
    let base_url = &analysis.url;
    let name = display_name;
    let card_sel = &analysis.card_selector;
    let title_sel = if analysis.title_selector.trim().is_empty() {
        "img".to_string()
    } else {
        analysis.title_selector.clone()
    };
    let next_sel = &analysis.next_page_selector;
    // Selector de episodios comprobado en una ficha real; si el sondeo no pudo
    // abrir una, se mantiene el genérico.
    let episode_sel = if analysis.episode_selector.trim().is_empty() {
        "li.episode, a[href*=/ver/], div.episodios a".to_string()
    } else {
        analysis.episode_selector.clone()
    };
    let i18n = get_i18n(lang);

    // Rutas reales detectadas. Si no se detectaron, se dejan vacías y el
    // código generado usa la URL de entrada en vez de inventar `/animes`.
    let popular_path = kt_path_literal(analysis.popular_path.as_deref());
    let latest_path = kt_path_literal(analysis.latest_path.as_deref());
    let search_path = kt_path_literal(analysis.search_path.as_deref());
    let search_param = kt_path_literal(analysis.search_param.as_deref());
    let page_style = analysis.page_style.as_str();
    let supports_latest = if latest_path.is_empty() { "false" } else { "true" };
    let site_cap_formats = analysis.capabilities.stream_formats.iter()
        .map(|f| format!("\"{}\"", f))
        .collect::<Vec<_>>()
        .join(", ");
    // boot/start admite cualquier versión, pero usar la que el sitio declara
    // hace que la sesión generada sea indistinguible de la del reproductor web.
    let site_app_version = analysis.capabilities.app_version
        .as_deref()
        .filter(|v| !v.is_empty())
        .unwrap_or("10.12.1096");
    let site_player_cases = analysis.capabilities.player_cases.iter()
        .map(|c| format!("\"{}\"", c))
        .collect::<Vec<_>>()
        .join(", ");
    let site_region = analysis.capabilities.region.as_deref().unwrap_or("");
    let site_locale = analysis.capabilities.locale.as_deref().unwrap_or("");
    let kt_str = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"").replace(['\n', '\r', '\t'], " ");
    let detected_language = if analysis.detected_language.trim().is_empty() {
        lang.to_string()
    } else {
        analysis.detected_language.clone()
    };
    let mut titles_seen: HashSet<String> = HashSet::new();
    let api_catalog = api_catalog_records(analysis);
    let mut ld_entries: Vec<String> = Vec::new();
    // Las tarjetas de la API verificada van primero: son datos reales obtenidos
    // del endpoint público, no una plantilla ni un título inventado.
    for record in &api_catalog {
        if !titles_seen.insert(record.title.to_lowercase()) {
            continue;
        }
        let image = record.image.as_deref().map(|s| format!("\"{}\"", kt_str(s))).unwrap_or_else(|| "null".to_string());
        let genre = record.genre.as_deref().map(|s| format!("\"{}\"", kt_str(s))).unwrap_or_else(|| "null".to_string());
        ld_entries.push(format!(
            "        LdRecord(title = \"{}\", alternateTitle = null, language = null, url = \"{}\", image = {}, genre = {})",
            kt_str(&record.title), kt_str(&record.url), image, genre
        ));
    }
    for r in &analysis.catalog_records {
        if !titles_seen.insert(r.title.to_lowercase()) {
            continue;
        }
        let alt = r.alternate_title.as_deref().map(|s| format!("\"{}\"", kt_str(s))).unwrap_or_else(|| "null".to_string());
        let lang = r.language.as_deref().map(|s| format!("\"{}\"", kt_str(s))).unwrap_or_else(|| "null".to_string());
        let image = r.image.as_deref().map(|s| format!("\"{}\"", kt_str(s))).unwrap_or_else(|| "null".to_string());
        ld_entries.push(format!(
            "        LdRecord(title = \"{}\", alternateTitle = {}, language = {}, url = \"{}\", image = {})",
            kt_str(&r.title), alt, lang, kt_str(&r.url), image
        ));
    }
    let ld_entries = ld_entries.join(",\n");
    let ld_records_block = if ld_entries.is_empty() {
        "    private val jsonLdRecords = emptyList<LdRecord>()".to_string()
    } else {
        format!("    private val jsonLdRecords = listOf(\n{ld_entries}\n    )")
    };
    let has_site_filters = analysis.filter_params.iter().any(|f| {
        !f.param.is_empty() && (f.kind == "text" || !f.value.is_empty())
    });
    // La exclusión es un filtro real: sus valores vienen de las listas `{id,name}`
    // que el sitio publica en su API. Se guarda en preferencias para que also
    // afecte a popular/latest, que no reciben el AnimeFilterList.
    let exclusion_pairs = api_exclusion_pairs(analysis, lang);
    let exclusion_capture = if exclusion_pairs.is_empty() {
        String::new()
    } else {
        format!(concat!(exclusion_capture_kt_fmt!()), class_name = class_name)
    };
    let filter_block = if has_site_filters {
        format!(concat!(filter_block_kt_fmt!()), class_name = class_name)
    } else {
        "        val finalUrl = url\n".to_string()
    };

    // SUB/DUB sólo se ofrece si AEXT abrió un máster real y encontró pistas.
    let (show_audio, sub_ok, dub_ok) = verified_audio_selection(&analysis.capabilities);
    let mut audio_labels: Vec<&str> = vec![i18n.pref_audio_entries[0]];
    let mut audio_values: Vec<&str> = vec!["all"];
    if sub_ok {
        audio_labels.push(i18n.pref_audio_entries[1]);
        audio_values.push("sub");
    }
    if dub_ok {
        audio_labels.push(i18n.pref_audio_entries[2]);
        audio_values.push("dub");
    }
    let pref_audio_title = i18n.pref_audio_title;
    let pref_audio_entries = audio_labels.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ");
    let pref_audio_values = audio_values.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ");
    let pref_audio_summary = i18n.pref_audio_summary;
    let audio_pref_block = if show_audio {
        format!(
            "        val audioPref = ListPreference(screen.context).apply {{\n            key = \"preferred_audio\"\n            title = \"{pref_audio_title}\"\n            entries = arrayOf({pref_audio_entries})\n            entryValues = arrayOf({pref_audio_values})\n            setDefaultValue(\"all\")\n            summary = \"{pref_audio_summary} — %s\"\n        }}\n        screen.addPreference(audioPref)\n\n"
        )
    } else {
        String::new()
    };
    let search_audio_block = if show_audio {
        format!(concat!(search_audio_block_kt_fmt!()), class_name = class_name)
    } else {
        "        val finalQuery = cleanQuery\n".to_string()
    };

    let pref_api_title = i18n.pref_api_title;
    let pref_api_entries = i18n.pref_api_entries.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ");
    let methods = if analysis.capabilities.playback_methods.is_empty() {
        SiteAnalyzer::playback_methods(&analysis.capabilities)
    } else {
        analysis.capabilities.playback_methods.clone()
    };
    let methods_title = i18n.methods_title;
    let methods_summary = methods.join(", ");
    let escape_kt = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let methods_entries = methods
        .iter()
        .map(|m| format!("\"{}\"", escape_kt(m)))
        .collect::<Vec<_>>()
        .join(", ");
    let pref_server_title = i18n.pref_server_title;
    let pref_server_summary = i18n.pref_server_summary;
    let pref_quality_title = i18n.pref_quality_title;
    let pref_server_blacklist_title = i18n.pref_server_blacklist_title;
    let pref_server_blacklist_summary = i18n.pref_server_blacklist_summary;
    let pref_genre_blacklist_title = i18n.pref_genre_blacklist_title;
    let pref_genre_blacklist_summary = i18n.pref_genre_blacklist_summary;

    format!(
        concat!(anime_source_kt_fmt!()),
        video_engine = VIDEO_ENGINE_KT,
        site_cap_formats = site_cap_formats,
        site_app_version = site_app_version,
        site_player_cases = site_player_cases,
        site_region = site_region,
        site_locale = site_locale,
        detected_language = kt_str(&detected_language),
        ld_records_block = ld_records_block,
        filter_block = filter_block,
        exclusion_capture = exclusion_capture,
        search_audio_block = search_audio_block,
        audio_pref_block = audio_pref_block,
        pref_api_title = pref_api_title,
        pref_api_entries = pref_api_entries,
        methods_title = methods_title,
        methods_summary = methods_summary,
        methods_entries = methods_entries,
        pref_server_summary = pref_server_summary,
        base_url = base_url,
        card_sel = card_sel,
        class_name = class_name,
        episode_sel = episode_sel,
        lang = lang,
        latest_path = latest_path,
        name = name,
        next_sel = next_sel,
        page_style = page_style,
        pkg_lang = pkg_lang,
        pkg_prefix = pkg_prefix,
        pkg_slug = pkg_slug,
        popular_path = popular_path,
        pref_genre_blacklist_summary = pref_genre_blacklist_summary,
        pref_genre_blacklist_title = pref_genre_blacklist_title,
        pref_quality_title = pref_quality_title,
        pref_server_blacklist_summary = pref_server_blacklist_summary,
        pref_server_blacklist_title = pref_server_blacklist_title,
        pref_server_title = pref_server_title,
        search_param = search_param,
        search_path = search_path,
        supports_latest = supports_latest,
        title_sel = title_sel,
    )
}
