use crate::engine::analyzer::AnalysisResult;

use super::api_helpers::{api_exclusion_pairs, api_option_lists, verified_audio_selection};
use super::i18n::get_i18n;
use super::templates::filters::{audio_filter_class_kt_fmt, kotlin_filters_kt_fmt};

/// Etiqueta legible derivada de un valor real del sitio. No traduce: sólo
/// separa guiones bajos y camelCase para no mostrar `kinopoisk_rating` crudo.
pub(crate) fn humanize_filter_value(value: &str) -> String {
    let mut spaced = String::new();
    let mut previous = ' ';
    for c in value.chars() {
        if c.is_uppercase() && previous.is_lowercase() {
            spaced.push(' ');
        }
        spaced.push(c);
        previous = c;
    }
    spaced
        .replace(['_', '-'], " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn generate_kotlin_filters(
    class_name: &str,
    pkg_slug: &str,
    lang: &str,
    pkg_lang: &str,
    pkg_prefix: &str,
    analysis: &AnalysisResult,
) -> String {
    let i18n = get_i18n(lang);
    let filter_header = i18n.filter_header;
    let caps = &analysis.capabilities;
    let kt = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let (show_audio, sub_ok, dub_ok) = verified_audio_selection(caps);
    let mut audio_labels: Vec<&str> = vec![i18n.filter_audio_entries[0]];
    let mut audio_vals: Vec<(&str, &str)> = vec![("all", "")];
    if sub_ok {
        audio_labels.push(i18n.filter_audio_entries[1]);
        audio_vals.push(("sub", "sub"));
    }
    if dub_ok {
        audio_labels.push(i18n.filter_audio_entries[2]);
        audio_vals.push(("dub", "dub"));
    }
    let audio_entries = audio_labels.iter().map(|s| format!("\"{}\"", kt(s))).collect::<Vec<_>>().join(", ");
    let audio_vals_kt = audio_vals
        .iter()
        .map(|(k, v)| format!("Pair(\"{}\", \"{}\")", kt(k), kt(v)))
        .collect::<Vec<_>>()
        .join(",\n            ");
    let audio_filter_class = if show_audio {
        let filter_audio_title = i18n.filter_audio_title;
        format!(
            audio_filter_class_kt_fmt(),
            audio_entries = audio_entries,
            audio_vals_kt = audio_vals_kt,
            filter_audio_title = kt(filter_audio_title),
        )
    } else {
        String::new()
    };
    let audio_filter_item = if show_audio {
        "        AudioFilter(),\n".to_string()
    } else {
        String::new()
    };

    // Sólo se generan filtros que el sitio expone de verdad. Los géneros de
    // la plantilla se eliminaron: nunca se podían traducir en una consulta.
    #[derive(Clone)]
    struct ParamGroup {
        name: String,
        /// Pares `(etiqueta, valor)`; la etiqueta puede venir de la API real.
        values: Vec<(String, String)>,
    }
    let mut grouped: Vec<(String, ParamGroup)> = Vec::new();
    for f in &analysis.filter_params {
        if f.param.is_empty() {
            continue;
        }
        let index = match grouped.iter().position(|(param, _)| param == &f.param) {
            Some(index) => index,
            None => {
                grouped.push((
                    f.param.clone(),
                    ParamGroup {
                        name: f.name.clone(),
                        values: Vec::new(),
                    },
                ));
                grouped.len() - 1
            }
        };
        let entry = &mut grouped[index].1;
        // Un filtro `text` no aporta valores: al quedarse `values` vacío se
        // genera un TextFilter en lugar de un selector con opciones.
        if f.kind != "text"
            && !f.value.is_empty()
            && !entry.values.iter().any(|(_, value)| value == &f.value)
        {
            entry.values.push((humanize_filter_value(&f.value), f.value.clone()));
        }
    }
    // Un parámetro del sitio sin valores enumerados se completa con las listas
    // `{id,name}` que el propio sitio publica en su API REST y que AEXT verificó.
    for (param, group) in grouped.iter_mut() {
        if !group.values.is_empty() {
            continue;
        }
        for api in api_option_lists(analysis, param.as_str()) {
            for option in &api.options {
                if !group.values.iter().any(|(_, value)| value == &option.value) {
                    group.values.push((option.label.clone(), option.value.clone()));
                }
            }
        }
    }
    // Exclusión con evidencia: los valores son los ids/nombres reales de las
    // listas verificadas. Si el sitio no publica esas listas, no hay filtro.
    let exclusion_pairs = api_exclusion_pairs(analysis);
    let (exclude_class, exclude_items) = if exclusion_pairs.is_empty() {
        (String::new(), String::new())
    } else {
        let items = exclusion_pairs
            .iter()
            .map(|(label, value)| {
                format!("        ExcludeFilter(\"{}\", \"{}\"),\n", kt(label), kt(value))
            })
            .collect::<String>();
        (
            "    class ExcludeFilter(name: String, val value: String) : AnimeFilter.CheckBox(name, false)\n\n"
                .to_string(),
            format!("        Header(\"{}\"),\n{}", kt(i18n.filter_genre_blacklist_title), items),
        )
    };
    // Se usan las mismas clases que las extensiones de la comunidad
    // (QueryPartFilter, CheckBoxFilterList/CheckBoxVal, TextFilter, SortFilter):
    // nombres y formas estándar en lugar de ParamSelectFilter/ParamCheckFilter.
    // El bloque `when (f)` de búsqueda referencia todas juntas, así que se
    // definen en bloque aunque un sitio concreto no use alguna de ellas.
    let mut param_classes = String::new();
    if !grouped.is_empty() {
        param_classes.push_str(
            r#"    open class QueryPartFilter(displayName: String, val param: String, val vals: Array<Pair<String, String>>) :
        AnimeFilter.Select<String>(displayName, vals.map { it.first }.toTypedArray()) {
        fun toQueryPart(): String = vals[state].second
    }

"#,
        );
        param_classes.push_str(
            r#"    open class CheckBoxFilterList(name: String, val param: String, val pairs: Array<Pair<String, String>>) :
        AnimeFilter.Group<AnimeFilter.CheckBox>(name, pairs.map { CheckBoxVal(it.first, false) })

"#,
        );
        param_classes.push_str(
            r#"    class CheckBoxVal(name: String, state: Boolean = false) : AnimeFilter.CheckBox(name, state)

"#,
        );
        param_classes.push_str(
            r#"    class TextFilter(name: String, val param: String) : AnimeFilter.Text(name)

"#,
        );
    }
    param_classes.push_str(&exclude_class);
    // Género, país, categoría, formato, idioma y subtítulos son listas multiselección
    // en los repos de referencia; año, estado, orden, calidad y similares son select.
    const MULTISELECT_LABELS: &[&str] = &[
        "Genre", "Country", "Category", "Format", "Audio Language", "Subtitles", "Copyright Holder",
    ];
    let param_items = grouped
        .iter()
        .map(|(param, group)| {
            let pairs = group
                .values
                .iter()
                .map(|(label, value)| format!("Pair(\"{}\", \"{}\")", kt(label), kt(value)))
                .collect::<Vec<_>>();
            if !group.values.is_empty() && MULTISELECT_LABELS.contains(&group.name.as_str()) {
                format!(
                    "        CheckBoxFilterList(\"{}\", \"{}\", arrayOf({})),\n",
                    kt(&group.name),
                    kt(param),
                    pairs.join(", ")
                )
            } else if !pairs.is_empty() {
                let opts = std::iter::once("Pair(\"All\", \"\")".to_string())
                    .chain(pairs)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "        QueryPartFilter(\"{}\", \"{}\", arrayOf({})),\n",
                    kt(&group.name),
                    kt(param),
                    opts
                )
            } else {
                format!(
                    "        TextFilter(\"{}\", \"{}\"),\n",
                    kt(&group.name),
                    kt(param)
                )
            }
        })
        .collect::<String>();

    format!(
        kotlin_filters_kt_fmt(),
        filter_header = kt(filter_header),
        param_classes = param_classes,
        audio_filter_class = audio_filter_class,
        param_items = param_items,
        exclude_items = exclude_items,
        audio_filter_item = audio_filter_item,
        class_name = class_name,
        pkg_prefix = pkg_prefix,
        pkg_lang = pkg_lang,
        pkg_slug = pkg_slug,
    )
}
