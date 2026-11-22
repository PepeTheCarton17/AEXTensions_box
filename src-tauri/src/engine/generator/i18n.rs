pub(crate) struct I18nData {
    pub(crate) pref_audio_title: &'static str,
    pub(crate) pref_audio_entries: &'static [&'static str],
    pub(crate) pref_audio_summary: &'static str,
    pub(crate) pref_api_title: &'static str,
    pub(crate) pref_api_entries: &'static [&'static str],
    /// Etiqueta del aviso que enumera los métodos encontrados de reproducción.
    pub(crate) methods_title: &'static str,
    pub(crate) pref_server_title: &'static str,
    pub(crate) pref_server_summary: &'static str,
    pub(crate) pref_quality_title: &'static str,
    pub(crate) pref_server_blacklist_title: &'static str,
    pub(crate) pref_server_blacklist_summary: &'static str,
    pub(crate) pref_genre_blacklist_title: &'static str,
    pub(crate) pref_genre_blacklist_summary: &'static str,
    pub(crate) filter_header: &'static str,
    /// Aviso mostrado cuando el sitio no declaró endpoint de búsqueda: el
    /// cuadro de texto sólo filtra el catálogo ya cargado.
    pub(crate) filter_header_no_search: &'static str,
    pub(crate) filter_all: &'static str,
    pub(crate) filter_audio_title: &'static str,
    pub(crate) filter_audio_entries: &'static [&'static str],
    pub(crate) filter_genre_blacklist_title: &'static str,
    /// Etiqueta del input del panel de filtros que excluye (invierte) términos
    /// separados por comas/punto y coma en vez de buscarlos en el sitio.
    pub(crate) filter_blacklist_input_title: &'static str,
}

pub(crate) fn get_i18n(lang: &str) -> I18nData {
    match lang.to_lowercase().as_str() {
        // Un sitio que declara español de forma explícita sí recibe textos en
        // español. `all` y los locales sin plantilla propia caen al inglés para
        // no inventar una traducción en un paquete multilingüe.
        "es" | "es-es" | "es-419" => I18nData {
            pref_audio_title: "Audio Preferido (SUB / DUB)",
            pref_audio_entries: &["Mostrar Todos", "Priorizar Subtitulado", "Priorizar Doblaje (DUB)"],
            pref_audio_summary: "Aparece solo porque AEXT verificó estas pistas en un stream real; nada se inventa.",
            pref_api_title: "API preferida",
            pref_api_entries: &["Automático", "Sesión web", "GraphQL", "HTML"],
            methods_title: "Métodos disponibles encontrados de reproducción",
            pref_server_title: "Servidor Preferido (Stream por Defecto)",
            pref_server_summary: "Solo reordena servidores ya detectados; no activa un servidor que no está disponible.",
            pref_quality_title: "Calidad Preferida (Stream por Defecto)",
            pref_server_blacklist_title: "Lista Negra de Servidores (Ocultar)",
            pref_server_blacklist_summary: "Desactiva servidores en el reproductor (ej. Zilla)",
            pref_genre_blacklist_title: "Lista Negra de Géneros (# - Ocultar)",
            pref_genre_blacklist_summary: "Oculta permanentemente animes con estos géneros (#) en el catálogo",
            filter_header: "Filtros de búsqueda y catálogo",
            filter_header_no_search: "La búsqueda por texto ignora el filtro",
            filter_all: "Todos",
            filter_audio_title: "Audio / Doblaje",
            filter_audio_entries: &["Todos", "Subtitulado (SUB)", "Doblado (DUB)"],
            filter_genre_blacklist_title: "Lista Negra de Géneros (# - Excluir)",
            filter_blacklist_input_title: "Lista Negra - excluir (separar con , o ;)",
        },
        "en" => I18nData {
            pref_audio_title: "Preferred Audio (SUB / DUB)",
            pref_audio_entries: &["Show All", "Prioritize Subbed", "Prioritize Dubbed (DUB)"],
            pref_audio_summary: "Shown only because AEXT verified these tracks in a real stream; nothing is invented.",
            pref_api_title: "Preferred API",
            pref_api_entries: &["Automatic", "Web session", "GraphQL", "HTML"],
            methods_title: "Playback methods found",
            pref_server_title: "Preferred Server (Default stream)",
            pref_server_summary: "Only reorders servers already found; it does not enable an unavailable server.",
            pref_quality_title: "Preferred Quality (Default stream)",
            pref_server_blacklist_title: "Server Blacklist (Hide)",
            pref_server_blacklist_summary: "Disable servers in player (e.g. Zilla)",
            pref_genre_blacklist_title: "Genre Blacklist (# - Hide)",
            pref_genre_blacklist_summary: "Permanently hide anime with these genres or tags (#) in catalog",
            filter_header: "Search & Catalog Filters",
            filter_header_no_search: "Text search ignores the filter",
            filter_all: "All",
            filter_audio_title: "Audio / Dubbing",
            filter_audio_entries: &["All", "Subtitled (SUB)", "Dubbed (DUB)"],
            filter_genre_blacklist_title: "Genre Blacklist (# - Exclude)",
            filter_blacklist_input_title: "Blacklist - exclude (separate with , or ;)",
        },
        "pt-br" | "pt" => I18nData {
            pref_audio_title: "Áudio Preferido (LEG / DUB)",
            pref_audio_entries: &["Mostrar Todos", "Priorizar Legendado", "Priorizar Dublado (DUB)"],
            pref_audio_summary: "Exibido apenas porque o AEXT verificou estas faixas num stream real; nada é inventado.",
            pref_api_title: "API preferida",
            pref_api_entries: &["Automático", "Sessão web", "GraphQL", "HTML"],
            methods_title: "Métodos de reprodução encontrados",
            pref_server_title: "Servidor Preferido (Stream Padrão)",
            pref_server_summary: "Apenas reordena servidores já encontrados; não ativa um servidor indisponível.",
            pref_quality_title: "Qualidade Preferida (Stream Padrão)",
            pref_server_blacklist_title: "Lista Negra de Servidores (Ocultar)",
            pref_server_blacklist_summary: "Desative servidores no reprodutor (ex. Zilla)",
            pref_genre_blacklist_title: "Lista Negra de Gêneros (# - Ocultar)",
            pref_genre_blacklist_summary: "Oculta permanentemente animes com estes gêneros (#) no catálogo",
            filter_header: "Filtros de busca e catálogo",
            filter_header_no_search: "A busca por texto ignora o filtro",
            filter_all: "Todos",
            filter_audio_title: "Áudio / Dublagem",
            filter_audio_entries: &["Todos", "Legendado (LEG)", "Dublado (DUB)"],
            filter_genre_blacklist_title: "Lista Negra de Gêneros (# - Excluir)",
            filter_blacklist_input_title: "Lista Negra - excluir (separar com , ou ;)",
        },
        // `all` e idiomas sin plantilla propia caen aquí. Un paquete multilingüe
        // no puede mostrar textos en español: sería una traducción inventada.
        _ => I18nData {
            pref_audio_title: "Preferred Audio (SUB / DUB)",
            pref_audio_entries: &["Show All", "Prioritize Subbed", "Prioritize Dubbed (DUB)"],
            pref_audio_summary: "Shown only because AEXT verified these tracks in a real stream; nothing is invented.",
            pref_api_title: "Preferred API",
            pref_api_entries: &["Automatic", "Web session", "GraphQL", "HTML"],
            methods_title: "Playback methods found",
            pref_server_title: "Preferred Server (Default stream)",
            pref_server_summary: "Only reorders servers already found; it does not enable an unavailable server.",
            pref_quality_title: "Preferred Quality (Default stream)",
            pref_server_blacklist_title: "Server Blacklist (Hide)",
            pref_server_blacklist_summary: "Disable servers in player (e.g. Zilla)",
            pref_genre_blacklist_title: "Genre Blacklist (# - Hide)",
            pref_genre_blacklist_summary: "Permanently hide anime with these genres or tags (#) in catalog",
            filter_header: "Search & Catalog Filters",
            filter_header_no_search: "Text search ignores the filter",
            filter_all: "All",
            filter_audio_title: "Audio / Dubbing",
            filter_audio_entries: &["All", "Subtitled (SUB)", "Dubbed (DUB)"],
            filter_genre_blacklist_title: "Genre Blacklist (# - Exclude)",
            filter_blacklist_input_title: "Blacklist - exclude (separate with , or ;)",
        },
    }
}

/// Etiqueta canónica de filtro en el idioma elegido para la extensión. Las
/// etiquetas del analizador siguen siendo canónicas en inglés para poder
/// emparejarlas con los repos de referencia; aquí se traducen para la UI.
pub(crate) fn filter_label(lang: &str, canonical: &str) -> String {
    // Varias secciones pueden compartir segmento (`Category (Movies)`): se
    // traduce sólo la etiqueta canónica y se conserva el calificador de la ruta.
    let (base, qualifier) = match canonical.split_once(" (") {
        Some((base, qualifier)) => (base, qualifier.strip_suffix(')').unwrap_or(qualifier)),
        None => (canonical, ""),
    };
    let key = base.trim().to_lowercase().replace([' ', '-'], "_");
    let (es, pt) = match key.as_str() {
        "year" | "years" => ("Año", "Ano"),
        "country" => ("País", "País"),
        "genre" => ("Género", "Gênero"),
        "category" => ("Categoría", "Categoria"),
        "audio_language" => ("Idioma de audio", "Idioma de áudio"),
        "language" => ("Idioma", "Idioma"),
        "quality" => ("Calidad", "Qualidade"),
        "format" => ("Formato", "Formato"),
        "season" => ("Temporada", "Temporada"),
        "copyright_holder" => ("Titular de derechos", "Titular de direitos"),
        "profession" => ("Profesión", "Profissão"),
        "tv_category" => ("Categoría de TV", "Categoria de TV"),
        "tag" => ("Etiqueta", "Etiqueta"),
        "sort_by" | "sort_order" | "sort" | "order" => ("Orden", "Ordem"),
        "status" => ("Estado", "Status"),
        "subtitles" => ("Subtítulos", "Legendas"),
        "display" => ("Visualización", "Exibição"),
        "type" => ("Tipo", "Tipo"),
        "rating" => ("Calificación", "Avaliação"),
        "search" => ("Búsqueda", "Busca"),
        "dubbed" => ("Doblado", "Dublado"),
        _ => return canonical.to_string(),
    };
    let translated = match lang.to_lowercase().as_str() {
        "es" | "es-es" | "es-419" => es.to_string(),
        "pt" | "pt-br" => pt.to_string(),
        _ => canonical.to_string(),
    };
    if qualifier.is_empty() {
        translated
    } else {
        format!("{translated} ({qualifier})")
    }
}

/// Traduce los valores humanizados que salen de parámetros crudos del sitio.
/// Las etiquetas reales devueltas por la API del sitio se dejan intactas: son
/// datos verificados, no texto generado por AEXT.
pub(crate) fn filter_value(lang: &str, value: &str) -> String {
    let key = value.trim().to_lowercase().replace([' ', '-'], "_");
    let (es, pt) = match key.as_str() {
        "none" => ("Ninguno", "Nenhum"),
        "any" => ("Cualquiera", "Qualquer"),
        "all" => ("Todos", "Todos"),
        "default" => ("Predeterminado", "Padrão"),
        "latest_updated" => ("Actualizados recientemente", "Atualizados recentemente"),
        "latest_added" => ("Agregados recientemente", "Adicionados recentemente"),
        "publish_date" | "release_date" => ("Fecha de publicación", "Data de publicação"),
        "imdb_rating" | "kinopoisk_rating" => ("Calificación IMDb", "Avaliação IMDb"),
        "rating" | "score" => ("Calificación", "Avaliação"),
        "name_a_z" | "name_asc" => ("Nombre A-Z", "Nome A-Z"),
        "most_viewed" => ("Más vistos", "Mais vistos"),
        "release_year" => ("Año de estreno", "Ano de lançamento"),
        "trending" => ("Tendencia", "Tendência"),
        "asc" => ("Ascendente", "Crescente"),
        "desc" => ("Descendente", "Decrescente"),
        "yes" => ("Sí", "Sim"),
        "no" => ("No", "Não"),
        "sub" => ("Subtitulado", "Legendado"),
        "dub" => ("Doblado", "Dublado"),
        "action" => ("Acción", "Ação"),
        "adventure" => ("Aventura", "Aventura"),
        "comedy" => ("Comedia", "Comédia"),
        "horror" => ("Terror", "Terror"),
        "drama" => ("Drama", "Drama"),
        _ => return value.to_string(),
    };
    match lang.to_lowercase().as_str() {
        "es" | "es-es" | "es-419" => es.to_string(),
        "pt" | "pt-br" => pt.to_string(),
        _ => value.to_string(),
    }
}
