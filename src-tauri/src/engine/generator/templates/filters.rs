/// Plantilla principal del archivo de filtros Kotlin.

macro_rules! kotlin_filters_kt_fmt {
    () => { r#"package {pkg_prefix}.{pkg_lang}.{pkg_slug}

import eu.kanade.tachiyomi.animesource.model.AnimeFilter
import eu.kanade.tachiyomi.animesource.model.AnimeFilterList

object {class_name}Filters {{
    class Header(name: String) : AnimeFilter.Header(name)

{param_classes}{audio_filter_class}    fun getFilterList(): AnimeFilterList = AnimeFilterList(
        Header("{filter_header}"),
{param_items}{exclude_items}{audio_filter_item}    )
}}
"# };
}
pub(crate) use kotlin_filters_kt_fmt;
pub const KOTLIN_FILTERS_KT: &str = kotlin_filters_kt_fmt!();

/// Plantilla de la clase de filtro de audio.
macro_rules! audio_filter_class_kt_fmt {
    () => { r#"    class AudioFilter : AnimeFilter.Select<String>(
        "{filter_audio_title}",
        arrayOf({audio_entries})
    ) {{
        val vals = arrayOf(
            {audio_vals_kt}
        )
    }}

"# };
}
pub(crate) use audio_filter_class_kt_fmt;
pub const AUDIO_FILTER_CLASS_KT: &str = audio_filter_class_kt_fmt!();

