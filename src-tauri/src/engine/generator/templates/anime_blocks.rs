/// Bloque Kotlin que guarda las exclusiones verificadas de la API.
macro_rules! exclusion_capture_kt_fmt {
    () => { r#"        val excludedApiValues = filters.filterIsInstance<{class_name}Filters.ExcludeFilter>()
            .filter {{ it.state }}
            .map {{ it.value.lowercase() }}
            .toHashSet()
        preferences.edit().putStringSet("excluded_api_values", excludedApiValues).apply()
"# };
}
pub(crate) use exclusion_capture_kt_fmt;

/// Bloque Kotlin que guarda el término escrito en el input de lista negra del
/// panel de filtros en la MISMA preferencia que el ajuste de lista negra. Así
/// la exclusión afecta también a popular/latest, que no reciben el
/// AnimeFilterList. El input se inicializa con el ajuste, así que escribirlo de
/// nuevo lo mantiene sincronizado y permite borrarlo vaciando el input.
macro_rules! blacklist_capture_kt_fmt {
    () => { r#"        val blacklistInput = filters.filterIsInstance<{class_name}Filters.BlacklistInputFilter>()
            .firstOrNull()?.state.orEmpty()
        preferences.edit().putString("genre_blacklist_text", blacklistInput).apply()
"# };
}
pub(crate) use blacklist_capture_kt_fmt;

/// Bloque Kotlin que aplica los filtros detectados a la URL.
macro_rules! filter_block_kt_fmt {
    () => { r#"        // Un filtro de ruta sustituye el path del catálogo; los query params
        // originales (p. ej. la paginación) se conservan sobre la nueva ruta.
        val pathOverride = filters.filterIsInstance<{class_name}Filters.PathFilter>()
            .firstOrNull()?.toPath().orEmpty()
        preferences.edit().putString("selected_path_filter", pathOverride).apply()
        val originalUrl = Uri.parse(url)
        val urlBuilder = if (pathOverride.isNotBlank()) {{
            Uri.parse(baseUrl.trimEnd('/') + pathOverride).buildUpon().apply {{
                originalUrl.queryParameterNames.forEach {{ name ->
                    originalUrl.getQueryParameter(name)?.let {{ appendQueryParameter(name, it) }}
                }}
            }}
        }} else {{
            originalUrl.buildUpon()
        }}
        for (f in filters) {{
            when (f) {{
                is {class_name}Filters.PathFilter -> Unit
                is {class_name}Filters.QueryPartFilter -> {{
                    val value = f.toQueryPart()
                    if (value.isNotBlank()) urlBuilder.appendQueryParameter(f.param, value)
                }}
                is {class_name}Filters.CheckBoxFilterList -> {{
                    f.state.filter {{ it.state }}.forEach {{ box ->
                        f.pairs.firstOrNull {{ it.first == box.name }}?.second?.let {{ value ->
                            urlBuilder.appendQueryParameter(f.param, value)
                        }}
                    }}
                }}
                is {class_name}Filters.TextFilter -> if (f.state.isNotBlank()) {{
                    urlBuilder.appendQueryParameter(f.param, f.state)
                }}
                else -> Unit
            }}
        }}
        val finalUrl = urlBuilder.build().toString()
"# };
}
pub(crate) use filter_block_kt_fmt;

/// Bloque Kotlin que aplica el filtro SUB/DUB a la búsqueda.
macro_rules! search_audio_block_kt_fmt {
    () => { r#"        val audioFilter = filters.filterIsInstance<{class_name}Filters.AudioFilter>().firstOrNull()
        val isDubSelected = audioFilter?.let {{ it.vals[it.state].second == "dub" }} ?: false

        val finalQuery = when {{
            isDubSelected && cleanQuery.isNotBlank() -> "$cleanQuery latino"
            isDubSelected -> "latino"
            else -> cleanQuery
        }}
"# };
}
pub(crate) use search_audio_block_kt_fmt;

