use crate::engine::analyzer::AnalysisResult;

use super::naming::kt_path_literal;
use super::templates::manga_source::{manga_filters_kt_fmt, manga_source_kt_fmt};

pub(crate) fn generate_manga_source(
    analysis: &AnalysisResult,
    display_name: &str,
    class_name: &str,
    pkg_slug: &str,
    lang: &str,
    pkg_lang: &str,
    pkg_prefix: &str,
) -> String {
    let base_url = &analysis.url;
    let name = display_name;
    let card_sel = &analysis.card_selector;
    let title_sel = if analysis.title_selector.trim().is_empty() {
        "img".to_string()
    } else {
        analysis.title_selector.clone()
    };
    let next_sel = &analysis.next_page_selector;

    let popular_path = kt_path_literal(analysis.popular_path.as_deref());
    let latest_path = kt_path_literal(analysis.latest_path.as_deref());
    let search_path = kt_path_literal(analysis.search_path.as_deref());
    let search_param = kt_path_literal(analysis.search_param.as_deref());
    let page_style = analysis.page_style.as_str();
    let supports_latest = if latest_path.is_empty() { "false" } else { "true" };

    format!(
        manga_source_kt_fmt(),
        pkg_prefix = pkg_prefix,
        pkg_lang = pkg_lang,
        pkg_slug = pkg_slug,
        class_name = class_name,
        name = name,
        base_url = base_url,
        lang = lang,
        supports_latest = supports_latest,
        popular_path = popular_path,
        latest_path = latest_path,
        search_path = search_path,
        search_param = search_param,
        page_style = page_style,
        card_sel = card_sel,
        title_sel = title_sel,
        next_sel = next_sel,
    )
}

pub(crate) fn generate_manga_filters(
    class_name: &str,
    pkg_slug: &str,
    pkg_lang: &str,
    pkg_prefix: &str,
) -> String {
    format!(
        manga_filters_kt_fmt(),
        pkg_prefix = pkg_prefix,
        pkg_lang = pkg_lang,
        pkg_slug = pkg_slug,
        class_name = class_name,
    )
}
