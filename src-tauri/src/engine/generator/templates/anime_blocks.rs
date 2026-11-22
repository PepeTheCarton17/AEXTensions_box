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
pub const EXCLUSION_CAPTURE_KT: &str = exclusion_capture_kt_fmt!();

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
pub const FILTER_BLOCK_KT: &str = filter_block_kt_fmt!();

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
pub const SEARCH_AUDIO_BLOCK_KT: &str = search_audio_block_kt_fmt!();

