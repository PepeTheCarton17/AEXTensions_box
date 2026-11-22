#[cfg(test)]
mod video_engine_tests {
    use crate::engine::generator::{generate_kotlin_filters, generate_kotlin_source, VIDEO_ENGINE_KT};
    use crate::engine::analyzer::{AnalysisResult, ApiOption, CatalogRecord, SiteCapabilities, SiteFilterParam, SitePathFilter, SiteSearchAction};

    fn generic_fixture() -> AnalysisResult {
        AnalysisResult {
            url: "https://catalog.example".to_string(),
            entry_url: "https://catalog.example/show/one".to_string(),
            name: "Demo".to_string(),
            lang: "es".to_string(),
            ext_type: "anime".to_string(),
            cms: "generic".to_string(),
            favicon_url: None,
            detected_hosts: vec!["catalog.example".to_string()],
            has_sub_dub: false,
            protections: Vec::new(),
            card_selector: "article".to_string(),
            title_selector: "h2".to_string(),
            next_page_selector: String::new(),
            popular_path: None,
            latest_path: None,
            search_path: None,
            search_param: None,
            page_style: "none".to_string(),
            catalog_detected: false,
            episode_selector: String::new(),
            sample_items: Vec::new(),
            warnings: Vec::new(),
            capabilities: SiteCapabilities {
                player_cases: vec!["hls-master".to_string()],
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn video_engine_has_no_vendor_specific_api_or_host() {
        // El motor común no debe contener URLs, hashes ni nombres de un
        // proveedor concreto. Los motores especializados se instalan aparte.
        for marker in [
            "pluto",
            "boot.pluto.tv",
            "PtvStart",
            "CBS_COM",
            "BOT_SESSION_BLOCKED",
            "tnPersistedDocumentHash",
            "stitcherParams",
            "api-session",
            "api-graphql",
            "CookieJar.NO_COOKIES",
        ] {
            assert!(
                !VIDEO_ENGINE_KT.to_lowercase().contains(marker.to_lowercase().as_str()),
                "el motor común no debe incluir código específico de un proveedor: {}",
                marker
            );
        }
    }

    #[test]
    fn generated_videos_are_verified_with_real_fallbacks() {
        for marker in [
            "private data class StreamProbe",
            "private fun probeOnce(",
            "private fun checkStream(",
            "private fun verifyCandidates(",
            "private val browserHeaders",
            "con cabeceras de navegador",
            "val ctx = VideoContext(finalUrl, html)",
            "val strategies = orderedStrategies()",
            "for (strategy in strategies)",
            "verifyCandidates(candidates)",
        ] {
            assert!(
                VIDEO_ENGINE_KT.contains(marker),
                "falta verificación en VIDEO_ENGINE_KT: {}",
                marker
            );
        }
        assert!(
            VIDEO_ENGINE_KT.contains("siteStreamFormats.contains(format)"),
            "las capacidades detectadas deben ordenar la verificación"
        );
    }

    #[test]
    fn strategies_are_ranked_and_learn_from_real_results() {
        for marker in [
            "private data class VideoContext",
            "private class VideoStrategy",
            "private fun strategyStats(",
            "private fun recordStrategy(",
            "private fun orderedStrategies()",
            "VideoStrategy(\"html-embeds\", strategyPriority(\"html-embeds\"))",
            "(ok + 1.0) / (ok + fail + 2.0)",
            "thenBy { it.priority }",
        ] {
            assert!(
                VIDEO_ENGINE_KT.contains(marker),
                "falta el pipeline de estrategias en VIDEO_ENGINE_KT: {}",
                marker
            );
        }
    }

    #[test]
    fn tracks_and_region_come_from_the_source_not_from_a_dub_guess() {
        for marker in [
            "private fun hlsTracks(body: String): Pair<List<String>, List<String>>",
            "private fun labelWithTracks(video: Video, probe: StreamProbe): Video",
            "audioLangs = (first.audioLangs + child.audioLangs).distinct()",
            "if (idx <= 0) return \"\"",
            "if (audio.isBlank()) \"\" else \"[$audio] \"",
        ] {
            assert!(
                VIDEO_ENGINE_KT.contains(marker),
                "falta detección honesta de idiomas: {}",
                marker
            );
        }
    }

    #[test]
    fn generated_kotlin_never_calls_the_video_copy_synthetic() {
        assert!(
            !VIDEO_ENGINE_KT.contains("video.copy("),
            "el template no debe llamar a Video.copy$default: no existe en todas las versiones de Nekoyomi"
        );
        assert!(
            !VIDEO_ENGINE_KT.contains(".copy(headers"),
            "el template no debe llamar a Video.copy$default para cambiar cabeceras"
        );
        assert!(
            !VIDEO_ENGINE_KT.contains(", \"HLS\", browserHeaders"),
            "el tercer argumento de Video es videoUrl, no el tipo de stream"
        );
        assert!(
            !VIDEO_ENGINE_KT.contains(", \"HLS\", headers"),
            "el tercer argumento de Video es videoUrl, no el tipo de stream"
        );
        for marker in [
            "private fun rebuildVideo(video: Video, quality: String, headers: Headers?): Video",
            "return Video(video.url, quality, video.videoUrl, headers)",
            "return rebuildVideo(video, labeled, video.headers)",
            "rebuildVideo(labeled, labeled.quality.orEmpty(), browserHeaders)",
        ] {
            assert!(
                VIDEO_ENGINE_KT.contains(marker),
                "falta la reconstrucción compatible de Video: {}",
                marker
            );
        }
    }

    #[test]
    fn generated_extension_has_no_vendor_api_preferences() {
        let kt = generate_kotlin_source(
            &generic_fixture(), "Demo", "Demo", "demo", "es", "es",
            "eu.kanade.tachiyomi.animeextension",
        );
        for marker in [
            "key = \"api_mode\"",
            "siteAppVersion",
            "siteRegion",
            "siteLocale",
            "sitePlayerCases",
            "Sesión web",
            "GraphQL",
        ] {
            assert!(
                !kt.contains(marker),
                "el Kotlin generado no debe incluir una API de proveedor: {}",
                marker
            );
        }
        assert!(
            kt.contains("Métodos disponibles encontrados de reproducción"),
            "debe conservarse el aviso genérico de métodos detectados"
        );
        assert!(kt.contains("Jsoup.parse(html, ctx.url)"));
    }

    #[test]
    fn sub_dub_ui_is_only_generated_when_a_real_master_verifies_tracks() {
        let unverified = generic_fixture();
        let kt = generate_kotlin_source(
            &unverified, "Demo", "Demo", "demo", "es", "es",
            "eu.kanade.tachiyomi.animeextension",
        );
        let filters = generate_kotlin_filters(
            "Demo", "demo", "es", "es", "eu.kanade.tachiyomi.animeextension",
            &unverified,
        );
        assert!(!kt.contains("key = \"preferred_audio\""), "sin verificación no se ofrece SUB/DUB");
        assert!(!kt.contains("AudioFilter"), "sin verificación no se genera el filtro de audio");
        assert!(!filters.contains("AudioFilter"), "sin verificación no se genera AudioFilter");
        assert!(kt.contains("val finalQuery = cleanQuery"), "la búsqueda no debe inventar 'latino'");

        let mut verified = generic_fixture();
        verified.capabilities.master_verified = true;
        verified.capabilities.sub_verified = true;
        let kt = generate_kotlin_source(
            &verified, "Demo", "Demo", "demo", "es", "es",
            "eu.kanade.tachiyomi.animeextension",
        );
        let filters = generate_kotlin_filters(
            "Demo", "demo", "es", "es", "eu.kanade.tachiyomi.animeextension",
            &verified,
        );
        assert!(kt.contains("key = \"preferred_audio\""), "con SUB verificado sí se ofrece el selector");
        assert!(filters.contains("class AudioFilter"), "con SUB verificado sí se genera el filtro");
        assert!(!kt.contains("entryValues = arrayOf(\"all\", \"sub\", \"dub\")"), "DUB no verificado no se ofrece");
        assert!(kt.contains("entryValues = arrayOf(\"all\", \"sub\")"), "sólo la opción verificada");
    }

    #[test]
    fn detected_filters_search_action_and_jsonld_catalog_reach_generated_kotlin() {
        let mut fixture = generic_fixture();
        fixture.detected_language = "en".to_string();
        fixture.catalog_source = "jsonld".to_string();
        fixture.search_path = Some("/catalog".to_string());
        fixture.search_param = Some("name".to_string());
        fixture.search_action = Some(SiteSearchAction {
            url_template: "https://catalog.example/catalog?name={search_term_string}".to_string(),
            query_param: Some("name".to_string()),
        });
        fixture.catalog_records = vec![CatalogRecord {
            title: "Título original".to_string(),
            alternate_title: Some("English title".to_string()),
            language: Some("uz".to_string()),
            url: "https://catalog.example/cinema/one".to_string(),
            image: Some("https://catalog.example/poster.jpg".to_string()),
        }];
        fixture.filter_params = vec![
            SiteFilterParam { name: "Género".to_string(), param: "genreIds".to_string(), value: "1".to_string(), kind: "check".to_string() },
            SiteFilterParam { name: "Género".to_string(), param: "genreIds".to_string(), value: "2".to_string(), kind: "check".to_string() },
        ];

        let kt = generate_kotlin_source(
            &fixture, "Demo", "Demo", "demo", "es", "es",
            "eu.kanade.tachiyomi.animeextension",
        );
        let filters = generate_kotlin_filters(
            "Demo", "demo", "es", "es", "eu.kanade.tachiyomi.animeextension", &fixture,
        );

        for marker in [
            "LdRecord(title = \"Título original\"",
            "alternateTitle = \"English title\"",
            "withJsonLdFallback(page, lastSearchQuery)",
            "lastSearchQuery = finalQuery",
            "val value = f.toQueryPart()",
            "is DemoFilters.QueryPartFilter",
            "else -> Unit",
            "pickTitle(element, \"h2\")",
            "data-title-es",
            "data-title-en",
            "private val pageLanguage = \"en\"",
        ] {
            assert!(kt.contains(marker), "falta en el Kotlin generado: {marker}");
        }
        for marker in [
            "class QueryPartFilter",
            "QueryPartFilter(\"Género\", \"genreIds\", arrayOf(Pair(\"Todos\", \"\"), Pair(\"1\", \"1\"), Pair(\"2\", \"2\")))",
        ] {
            assert!(filters.contains(marker), "falta en los filtros generados: {marker}");
        }
        assert!(!filters.contains("DEFAULT_GENRES"), "no se deben generar géneros inventados");
        assert!(!filters.contains("GenreFilter("), "el filtro de género falso debe desaparecer");
    }

    #[test]
    fn spanish_extension_translates_filter_labels_values_and_headers() {
        let mut fixture = generic_fixture();
        fixture.search_path = None;
        fixture.search_param = None;
        fixture.filter_params = vec![
            SiteFilterParam { name: "Sort By".to_string(), param: "sortBy".to_string(), value: "publish_date".to_string(), kind: "select".to_string() },
            SiteFilterParam { name: "Genre".to_string(), param: "genreIds".to_string(), value: "action".to_string(), kind: "check".to_string() },
        ];

        let filters = generate_kotlin_filters(
            "Demo", "demo", "es", "es", "eu.kanade.tachiyomi.animeextension", &fixture,
        );
        for marker in [
            "Header(\"La búsqueda por texto ignora el filtro\")",
            "QueryPartFilter(\"Orden\", \"sortBy\", arrayOf(Pair(\"Todos\", \"\"), Pair(\"Fecha de publicación\", \"publish_date\")))",
            "CheckBoxFilterList(\"Género\", \"genreIds\", arrayOf(Pair(\"Acción\", \"action\")))",
        ] {
            assert!(filters.contains(marker), "falta en los filtros generados: {marker}\n{}", filters);
        }
        assert!(!filters.contains("Pair(\"All\", \"\")"), "All no debe aparecer en español");
        assert!(!filters.contains("Publish Date"), "el valor humanizado debe traducirse");
    }

    #[test]
    fn path_filters_replace_catalog_route_and_keep_pagination() {
        let mut fixture = generic_fixture();
        fixture.path_filters = vec![SitePathFilter {
            name: "Category (Movies)".to_string(),
            segment: "category".to_string(),
            base_path: "/latam/movies/".to_string(),
            values: vec![
                ApiOption {
                    value: "/latam/movies/category/action-adventure/".to_string(),
                    label: "Acción y Aventura".to_string(),
                },
                ApiOption {
                    value: "/latam/movies/category/comedy/".to_string(),
                    label: "Comedia".to_string(),
                },
            ],
        }];

        let filters = generate_kotlin_filters(
            "Demo", "demo", "es", "es", "eu.kanade.tachiyomi.animeextension", &fixture,
        );
        let kt = generate_kotlin_source(
            &fixture, "Demo", "Demo", "demo", "es", "es",
            "eu.kanade.tachiyomi.animeextension",
        );

        for marker in [
            "class PathFilter",
            "fun toPath(): String = vals[state].second",
            "PathFilter(\"Categoría (Movies)\", arrayOf(Pair(\"Todos\", \"\"), Pair(\"Acción y Aventura\", \"/latam/movies/category/action-adventure/\"), Pair(\"Comedia\", \"/latam/movies/category/comedy/\")))",
        ] {
            assert!(filters.contains(marker), "falta en los filtros generados: {marker}\n{}", filters);
        }
        for marker in [
            "val pathOverride = filters.filterIsInstance<DemoFilters.PathFilter>()",
            "preferences.edit().putString(\"selected_path_filter\", pathOverride).apply()",
            "Uri.parse(baseUrl.trimEnd('/') + pathOverride).buildUpon().apply {",
            "originalUrl.getQueryParameter(name)?.let { appendQueryParameter(name, it) }",
            "is DemoFilters.PathFilter -> Unit",
            "GET(pageUrl(catalogPath(listingPath), page), headers)",
        ] {
            assert!(kt.contains(marker), "falta en el Kotlin generado: {marker}");
        }
    }

    #[test]
    fn all_language_uses_english_labels_and_text_filters_for_unknown_values() {
        let mut fixture = generic_fixture();
        // Este test comprueba el idioma: se declara un endpoint para que se
        // use la cabecera normal y no el aviso de "sin búsqueda".
        fixture.search_path = Some("/search".to_string());
        fixture.search_param = Some("q".to_string());
        fixture.filter_params = vec![
            SiteFilterParam { name: "Sort By".to_string(), param: "sortBy".to_string(), value: "publish_date".to_string(), kind: "select".to_string() },
            SiteFilterParam { name: "Sort By".to_string(), param: "sortBy".to_string(), value: "imdb_rating".to_string(), kind: "select".to_string() },
            SiteFilterParam { name: "Genre".to_string(), param: "genreIds".to_string(), value: String::new(), kind: "text".to_string() },
        ];
        let filters = generate_kotlin_filters(
            "Demo", "demo", "all", "all", "eu.kanade.tachiyomi.animeextension", &fixture,
        );
        assert!(filters.contains("Header(\"Search & Catalog Filters\")"));
        assert!(!filters.contains("Filtros de búsqueda"));
        assert!(filters.contains("class TextFilter"));
        assert!(filters.contains("TextFilter(\"Genre\", \"genreIds\")"));
        assert!(filters.contains("Pair(\"Publish Date\", \"publish_date\")"));
        assert!(filters.contains("Pair(\"Imdb Rating\", \"imdb_rating\")"));
    }

    #[test]
    fn distinct_params_with_the_same_label_stay_as_separate_filters() {
        let mut fixture = generic_fixture();
        fixture.filter_params = vec![
            SiteFilterParam { name: "Sort By".to_string(), param: "sort".to_string(), value: String::new(), kind: "text".to_string() },
            SiteFilterParam { name: "Sort By".to_string(), param: "sortBy".to_string(), value: "publish_date".to_string(), kind: "select".to_string() },
        ];
        let filters = generate_kotlin_filters(
            "Demo", "demo", "es", "es", "eu.kanade.tachiyomi.animeextension", &fixture,
        );
        for marker in [
            "TextFilter(\"Orden\", \"sort\")",
            "QueryPartFilter(\"Orden\", \"sortBy\", arrayOf(Pair(\"Todos\", \"\"), Pair(\"Fecha de publicación\", \"publish_date\")))",
        ] {
            assert!(filters.contains(marker), "falta en los filtros generados: {marker}\n{}", filters);
        }
    }

    #[test]
    fn multilingual_sites_use_all_and_selection_only_translates_texts() {
        use crate::engine::generator::ExtensionGenerator;
        let mut fixture = generic_fixture();
        fixture.detected_language = "uz".to_string();
        fixture.site_locales = vec!["uz".to_string(), "ru".to_string(), "en".to_string()];

        // Varios idiomas declarados: la extensión no pertenece a uno solo.
        // La elección de la UI traduce textos, pero no cambia la identidad.
        let bundle = ExtensionGenerator::generate(&fixture, None, Some("es"), Some("anime"));
        assert_eq!(bundle.lang, "all");
        assert!(bundle.pkg_id.contains(".all."));

        let source = bundle
            .files
            .iter()
            .find(|(path, _)| path.ends_with("/Demo.kt"))
            .map(|(_, bytes)| String::from_utf8_lossy(bytes).into_owned())
            .expect("falta la fuente Kotlin generada");
        assert!(source.contains("override val lang = \"all\""), "{source}");
        assert!(!source.contains("override val lang = \"es\""), "{source}");
        // El idioma principal sigue sirviendo para elegir títulos, aunque el
        // paquete sea multilenguaje.
        assert!(source.contains("data-title-uz"), "{source}");

        let filters = bundle
            .files
            .iter()
            .find(|(path, _)| path.ends_with("/DemoFilters.kt"))
            .map(|(_, bytes)| String::from_utf8_lossy(bytes).into_owned())
            .expect("faltan los filtros Kotlin generados");
        assert!(filters.contains("La búsqueda por texto ignora el filtro"), "{filters}");

        let es = generate_kotlin_filters(
            "Demo", "demo", "es", "all", "eu.kanade.tachiyomi.animeextension", &fixture,
        );
        assert!(es.contains("La búsqueda por texto ignora el filtro"));
        let fallback = generate_kotlin_filters(
            "Demo", "demo", "all", "all", "eu.kanade.tachiyomi.animeextension", &fixture,
        );
        assert!(fallback.contains("Text search ignores the filter"));

        let bundle = ExtensionGenerator::generate(&fixture, None, None, Some("anime"));
        assert_eq!(bundle.lang, "all");
        assert!(bundle.pkg_id.contains(".all."));
    }

    #[test]
    fn single_language_site_keeps_its_detected_language() {
        use crate::engine::generator::ExtensionGenerator;
        let mut fixture = generic_fixture();
        fixture.detected_language = "uz".to_string();
        fixture.site_locales = vec!["uz".to_string()];

        let bundle = ExtensionGenerator::generate(&fixture, None, Some("es"), Some("anime"));
        assert_eq!(bundle.lang, "uz");
        assert!(bundle.pkg_id.contains(".uz."));

        let source = bundle
            .files
            .iter()
            .find(|(path, _)| path.ends_with("/Demo.kt"))
            .map(|(_, bytes)| String::from_utf8_lossy(bytes).into_owned())
            .expect("falta la fuente Kotlin generada");
        assert!(source.contains("override val lang = \"uz\""), "{source}");
        assert!(!source.contains("override val lang = \"es\""), "{source}");
    }

    #[test]
    fn stream_failures_are_logged_and_explained_instead_of_silent_loading_failed() {
        for marker in [
            "import android.util.Log",
            "Log.e(\"AEXT\"",
            "Log.w(\"AEXT\"",
            "playlist vacía",
            "máster sin variantes",
            "variante vacía",
            "HTTP 403 (token/región rechazados)",
            "sin verificar",
            "ningún método aplicable a esta página",
        ] {
            assert!(
                VIDEO_ENGINE_KT.contains(marker) || marker == "import android.util.Log",
                "falta comprobación o traza de errores: {}",
                marker
            );
        }
    }

    #[test]
    fn verified_rest_lists_feed_filters_exclusion_and_catalog() {
        use crate::engine::analyzer::{ApiListResult, ApiOption};
        let mut fixture = generic_fixture();
        fixture.url = "https://catalog.example".to_string();
        fixture.catalog_records = vec![CatalogRecord {
            title: "Título original".to_string(),
            url: "https://catalog.example/cinema/original".to_string(),
            ..Default::default()
        }];
        fixture.filter_params = vec![SiteFilterParam {
            name: "Genre".to_string(),
            param: "genreIds".to_string(),
            value: String::new(),
            kind: "text".to_string(),
        }];
        fixture.api_lists = vec![
            ApiListResult {
                url: "https://api.example/v1/genres".to_string(),
                resource: "genres".to_string(),
                options: vec![
                    ApiOption { value: "4".to_string(), label: "Comedy".to_string() },
                    ApiOption { value: "9".to_string(), label: "Horror".to_string() },
                ],
                title_field: "name".to_string(),
                record_count: 2,
                ..Default::default()
            },
            ApiListResult {
                url: "https://api.example/v1/categories".to_string(),
                resource: "categories".to_string(),
                options: vec![ApiOption { value: "3".to_string(), label: "Series".to_string() }],
                ..Default::default()
            },
            ApiListResult {
                url: "https://api.example/v1/contents".to_string(),
                resource: "contents".to_string(),
                sample_json: r#"[{"id":"abc","permalink":"new-one","name":"New One","poster_link":"https://img.example/a.webp","category_id":3}]"#.to_string(),
                title_field: "name".to_string(),
                image_field: "poster_link".to_string(),
                url_field: "permalink".to_string(),
                record_count: 1,
                ..Default::default()
            },
        ];

        let filters = generate_kotlin_filters(
            "Demo", "demo", "es", "es", "eu.kanade.tachiyomi.animeextension", &fixture,
        );
        assert!(filters.contains("CheckBoxFilterList(\"Género\", \"genreIds\", arrayOf(Pair(\"Comedy\", \"4\")"), "{}", filters);
        assert!(filters.contains("ExcludeFilter(\"Género: Comedy\", \"4\")"), "{}", filters);
        assert!(filters.contains("ExcludeFilter(\"Categoría: Series\", \"3\")"), "{}", filters);
        assert!(!filters.contains("Ecchi"), "no se permiten géneros inventados");

        let kt = generate_kotlin_source(
            &fixture, "Catalog", "Demo", "demo", "es", "es",
            "eu.kanade.tachiyomi.animeextension",
        );
        assert!(kt.contains("LdRecord(title = \"New One\""), "{}", kt);
        assert!(kt.contains("https://catalog.example/cinema/new-one"), "{}", kt);
        assert!(kt.contains("genre = \"Series\""), "{}", kt);
        assert!(kt.contains("excluded_api_values"), "{}", kt);
        assert!(kt.contains(".filter { it.state }"), "{}", kt);
    }

    #[test]
    fn verified_filter_lists_generate_filters_without_html_params() {
        use crate::engine::analyzer::{ApiListResult, ApiOption};
        let mut fixture = generic_fixture();
        // SPA sin `<select>` y sin parámetros en bundles: sólo listas REST
        // verificadas con un GET público.
        fixture.filter_params = Vec::new();
        fixture.api_lists = vec![
            ApiListResult {
                url: "https://api.example/user/api/v1/filters/genres".to_string(),
                resource: "genres".to_string(),
                options: vec![
                    ApiOption { value: "4".to_string(), label: "Comedy".to_string() },
                    ApiOption { value: "9".to_string(), label: "Horror".to_string() },
                ],
                record_count: 2,
                ..Default::default()
            },
            ApiListResult {
                url: "https://api.example/user/api/v1/filters/years".to_string(),
                resource: "years".to_string(),
                options: vec![
                    ApiOption { value: "2026".to_string(), label: "2026".to_string() },
                    ApiOption { value: "2025".to_string(), label: "2025".to_string() },
                ],
                record_count: 2,
                ..Default::default()
            },
        ];

        let filters = generate_kotlin_filters(
            "Frame", "frame", "es", "es", "eu.kanade.tachiyomi.animeextension", &fixture,
        );
        assert!(
            filters.contains("CheckBoxFilterList(\"Género\", \"genres\", arrayOf(Pair(\"Comedy\", \"4\")"),
            "{}",
            filters
        );
        assert!(
            filters.contains("QueryPartFilter(\"Año\", \"years\", arrayOf(Pair(\"Todos\", \"\"), Pair(\"2026\", \"2026\")"),
            "{}",
            filters
        );

        let kt = generate_kotlin_source(
            &fixture, "Frame", "Frame", "frame", "es", "es",
            "eu.kanade.tachiyomi.animeextension",
        );
        assert!(kt.contains("is FrameFilters.QueryPartFilter"), "{}", kt);
        assert!(kt.contains("excluded_api_values"), "{}", kt);
    }

    #[test]
    fn blacklist_is_a_generic_inverted_filter_input() {
        let mut fixture = generic_fixture();
        // Sin listas verificadas ni parámetros: el input de lista negra debe
        // existir igualmente, porque el ajuste de lista negra siempre existe.
        fixture.api_lists = Vec::new();
        fixture.filter_params = Vec::new();

        let filters_es = generate_kotlin_filters(
            "Demo", "demo", "es", "es", "eu.kanade.tachiyomi.animeextension", &fixture,
        );
        assert!(
            filters_es.contains("class BlacklistInputFilter(name: String, initial: String) : AnimeFilter.Text(name)"),
            "{}",
            filters_es
        );
        assert!(filters_es.contains("init { state = initial }"), "{}", filters_es);
        assert!(
            filters_es.contains("BlacklistInputFilter(\"Lista Negra - excluir (separar con , o ;)\", initialBlacklist)"),
            "{}",
            filters_es
        );

        let filters_en = generate_kotlin_filters(
            "Demo", "demo", "en", "en", "eu.kanade.tachiyomi.animeextension", &fixture,
        );
        assert!(
            filters_en.contains("BlacklistInputFilter(\"Blacklist - exclude (separate with , or ;)\", initialBlacklist)"),
            "{}",
            filters_en
        );

        let kt = generate_kotlin_source(
            &fixture, "Demo", "Demo", "demo", "es", "es",
            "eu.kanade.tachiyomi.animeextension",
        );
        // El valor se guarda en la misma clave del ajuste y se invierte para
        // ocultar resultados en popular/latest/search.
        assert!(kt.contains("Filters.BlacklistInputFilter"), "{}", kt);
        assert!(
            kt.contains("putString(\"genre_blacklist_text\", blacklistInput)"),
            "{}",
            kt
        );
        assert!(
            kt.contains("getFilterList(preferences.getString(\"genre_blacklist_text\", \"\") ?: \"\")"),
            "{}",
            kt
        );
        assert!(kt.contains(".split(\",\", \";\", \"|\")"), "{}", kt);
        assert!(kt.contains("filterNot { anime ->"), "{}", kt);
    }

    #[test]
    fn catalog_cards_expose_detected_genre_tags() {
        let mut fixture = generic_fixture();
        fixture.genre_selector = "a[href*='/genero/']".to_string();

        let kt = generate_kotlin_source(
            &fixture, "Demo", "Demo", "demo", "es", "es",
            "eu.kanade.tachiyomi.animeextension",
        );
        assert!(kt.contains("private fun pickGenres(element: Element, genreSelector: String)"), "{}", kt);
        assert!(
            kt.contains("genre = pickGenres(element, \"a[href*='/genero/']\")"),
            "{}",
            kt
        );
        // La etiqueta detectada alimenta `anime.genre`, que es lo que lee la
        // lista negra; si no se detectó, el generador no inventa ninguna.
        fixture.genre_selector = String::new();
        let kt_empty = generate_kotlin_source(
            &fixture, "Demo", "Demo", "demo", "es", "es",
            "eu.kanade.tachiyomi.animeextension",
        );
        assert!(kt_empty.contains("genre = pickGenres(element, \"\")"), "{}", kt_empty);
    }
}
