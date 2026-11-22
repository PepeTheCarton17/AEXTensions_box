    use super::*;

    const CATALOG_HTML: &str = r#"<!doctype html>
<html><body>
<nav>
  <a href="/explorar">Explorar</a>
  <a href="/recientes">Recientes</a>
  <a href="/populares">Populares</a>
</nav>
<form action="/buscar"><input type="search" name="q"></form>
<main>
  <article><a href="/anime/1"><img alt="Uno"><h3>Uno</h3></a></article>
  <article><a href="/anime/2"><img alt="Dos"><h3>Dos</h3></a></article>
  <article><a href="/anime/3"><img alt="Tres"><h3>Tres</h3></a></article>
  <article><a href="/anime/4"><img alt="Cuatro"><h3>Cuatro</h3></a></article>
  <article><a href="/anime/5"><img alt="Cinco"><h3>Cinco</h3></a></article>
  <article><a href="/anime/6"><img alt="Seis"><h3>Seis</h3></a></article>
  <a rel="next" href="/explorar?page=2">Siguiente</a>
</main>
</body></html>"#;

    #[test]
    fn same_origin_path_only_accepts_internal_urls() {
        let origin = "https://example.com";
        assert_eq!(
            SiteAnalyzer::same_origin_path("/animes?page=2", origin, "example.com").as_deref(),
            Some("/animes?page=2")
        );
        assert_eq!(
            SiteAnalyzer::same_origin_path("animes", origin, "example.com").as_deref(),
            Some("/animes")
        );
        assert_eq!(
            SiteAnalyzer::same_origin_path("https://otro.com/x", origin, "example.com"),
            None
        );
        assert_eq!(
            SiteAnalyzer::same_origin_path("javascript:void(0)", origin, "example.com"),
            None
        );
    }

    #[test]
    fn detects_real_catalog_paths_and_search() {
        let analyzer = SiteAnalyzer::new();
        let doc = Html::parse_document(CATALOG_HTML);
        let origin = "https://example.com";
        let host = "example.com";

        let catalog = analyzer.detect_catalog(&doc, origin, host);
        assert_eq!(catalog.count, 6);
        assert_eq!(catalog.card_selector, "article");
        assert_eq!(catalog.page_style, "query");
        assert!(!catalog.next_page_selector.is_empty());
        assert!(catalog.samples.contains(&"Uno".to_string()));

        let (popular, latest) = analyzer.detect_section_paths(&doc, origin, host, &catalog);
        assert_eq!(popular.as_deref(), Some("/populares"));
        assert_eq!(latest.as_deref(), Some("/recientes"));

        let (search_path, search_param) = analyzer.detect_search(&doc, origin, host, "generic");
        assert_eq!(search_path.as_deref(), Some("/buscar"));
        assert_eq!(search_param.as_deref(), Some("q"));
    }

    #[test]
    fn detects_cards_from_repeated_link_classes() {
        let analyzer = SiteAnalyzer::new();
        let doc = Html::parse_document(
            r#"<!doctype html><html><body>
<nav>
  <a class="vds-nav-item" href="/latam/live-tv/">TV</a>
  <a class="vds-nav-item" href="/latam/movies/">Películas</a>
  <a class="vds-nav-item" href="/latam/series/">Series</a>
  <a class="vds-nav-item" href="/latam/kids/">Kids</a>
  <a class="vds-nav-item" href="/latam/sports/">Deportes</a>
  <a class="vds-nav-item" href="/latam/news/">Noticias</a>
</nav>
<main>
  <a class="TileLink_tileLink__pIMWG" href="/latam/shows/the-nanny-ptv3/"><img alt="The Nanny" src="/a.jpg"></a>
  <a class="TileLink_tileLink__pIMWG" href="/latam/shows/csi-miami-ptv1/"><img alt="CSI: Miami" src="/b.jpg"></a>
  <a class="TileLink_tileLink__pIMWG" href="/latam/shows/friends-ptv2/"><img alt="Friends" src="/c.jpg"></a>
  <a class="TileLink_tileLink__pIMWG" href="/latam/shows/seinfeld-ptv4/"><img alt="Seinfeld" src="/d.jpg"></a>
  <a class="TileLink_tileLink__pIMWG" href="/latam/shows/frasier-ptv5/"><img alt="Frasier" src="/e.jpg"></a>
  <a class="TileLink_tileLink__pIMWG" href="/latam/shows/cheers-ptv6/"><img alt="Cheers" src="/f.jpg"></a>
</main>
</body></html>"#,
        );

        let catalog = analyzer.detect_catalog(&doc, "https://catalog.example", "catalog.example");
        assert_eq!(catalog.count, 6);
        assert_eq!(catalog.card_selector, "a.TileLink_tileLink__pIMWG");
        assert_eq!(catalog.title_selector, "img");
        assert!(catalog.samples.contains(&"Friends".to_string()));
        // La barra de navegación no tiene imágenes: no debe ganar el grupo.
        assert!(!catalog.card_selector.contains("vds-nav-item"));
    }

    #[test]
    fn carousel_nav_ready_container_does_not_hide_cards() {
        let analyzer = SiteAnalyzer::new();
        let doc = Html::parse_document(
            r#"<!doctype html><html><body>
<div class="carousel__container carousel__container--nav-ready">
  <div class="swiper-slide"><a class="CardLink_cardLink__xYz" href="/shows/one/"><img alt="One" src="/a.jpg"></a></div>
  <div class="swiper-slide"><a class="CardLink_cardLink__xYz" href="/shows/two/"><img alt="Two" src="/b.jpg"></a></div>
  <div class="swiper-slide"><a class="CardLink_cardLink__xYz" href="/shows/three/"><img alt="Three" src="/c.jpg"></a></div>
  <div class="swiper-slide"><a class="CardLink_cardLink__xYz" href="/shows/four/"><img alt="Four" src="/d.jpg"></a></div>
  <div class="swiper-slide"><a class="CardLink_cardLink__xYz" href="/shows/five/"><img alt="Five" src="/e.jpg"></a></div>
  <div class="swiper-slide"><a class="CardLink_cardLink__xYz" href="/shows/six/"><img alt="Six" src="/f.jpg"></a></div>
</div>
</body></html>"#,
        );

        let catalog = analyzer.detect_catalog(&doc, "https://example.com", "example.com");
        assert!(catalog.count >= 4, "las tarjetas del carrusel deben conservarse");
        assert_eq!(catalog.card_selector, "a.CardLink_cardLink__xYz");
    }

    #[test]
    fn sticky_footer_layout_does_not_hide_cards() {
        // Un contenedor principal puede llamarse
        // `stickyFooterLayout`. Buscar `footer` como substring descartaba todo.
        let analyzer = SiteAnalyzer::new();
        let doc = Html::parse_document(
            r#"<!doctype html><html><body>
<main class="PageLayout_stickyFooterLayout__Ki1Wi">
  <div class="PageLayout_stickyFooterContentWrapper__ZMFxL">
    <div class="vds-carousel__container vds-carousel__container--nav-ready">
      <a class="TileLink_tileLink__pIMWG" href="/latam/shows/one/"><img alt="One" src="/a.jpg"></a>
      <a class="TileLink_tileLink__pIMWG" href="/latam/shows/two/"><img alt="Two" src="/b.jpg"></a>
      <a class="TileLink_tileLink__pIMWG" href="/latam/shows/three/"><img alt="Three" src="/c.jpg"></a>
      <a class="TileLink_tileLink__pIMWG" href="/latam/shows/four/"><img alt="Four" src="/d.jpg"></a>
      <a class="TileLink_tileLink__pIMWG" href="/latam/shows/five/"><img alt="Five" src="/e.jpg"></a>
      <a class="TileLink_tileLink__pIMWG" href="/latam/shows/six/"><img alt="Six" src="/f.jpg"></a>
    </div>
  </div>
</main>
</body></html>"#,
        );

        let catalog = analyzer.detect_catalog(&doc, "https://catalog.example", "catalog.example");
        assert_eq!(catalog.count, 6, "las tarjetas bajo stickyFooter deben contarse");
        assert_eq!(catalog.card_selector, "a.TileLink_tileLink__pIMWG");
        assert!(catalog.samples.contains(&"One".to_string()));
    }

    #[test]
    fn detects_episode_selector_ignoring_live_tv_menu() {
        let html = r#"<!doctype html><html><body>
<nav><a href="/latam/watch/live-tv/">TV en directo</a></nav>
<a class="EpisodeLink" href="/latam/shows/970009058/episode/6adb6f6e73415cc4ae5a4168/">T1 E1</a>
<a class="EpisodeLink" href="/latam/shows/970009058/episode/6adb6f7073415cc4ae5ab46a/">T1 E2</a>
<a class="EpisodeLink" href="/latam/shows/970009058/episode/6ada68747102b85b829f2ecb/">T1 E3</a>
<a href="/latam/shows/970009058/season/1/">Temporada 1</a>
</body></html>"#;

        let (selector, count) = SiteAnalyzer::detect_episode_selector(html).unwrap();
        assert_eq!(selector, "a[href*='/episode/']");
        assert_eq!(count, 3);
    }

    #[test]
    fn no_episode_selector_when_only_menu_links() {
        let html = r#"<!doctype html><html><body>
<a href="/ver/">Ver</a>
<a href="/latam/watch/live-tv/">TV en directo</a>
<a href="/series/">Series</a>
</body></html>"#;

        assert!(SiteAnalyzer::detect_episode_selector(html).is_none());
    }

    #[test]
    fn does_not_fabricate_paths_or_search() {
        let analyzer = SiteAnalyzer::new();
        let doc = Html::parse_document(
            r#"<!doctype html><html><body><div id="app"></div></body></html>"#,
        );
        let origin = "https://spa.example.com";
        let host = "spa.example.com";

        let catalog = analyzer.detect_catalog(&doc, origin, host);
        assert_eq!(catalog.count, 0);
        assert!(catalog.next_page_selector.is_empty());

        let (popular, latest) = analyzer.detect_section_paths(&doc, origin, host, &catalog);
        assert_eq!(popular, None);
        assert_eq!(latest, None);

        let (search_path, search_param) = analyzer.detect_search(&doc, origin, host, "generic");
        assert_eq!(search_path, None);
        assert_eq!(search_param, None);
    }

    #[test]
    fn wordpress_search_fallback_is_cms_specific() {
        let analyzer = SiteAnalyzer::new();
        let doc = Html::parse_document("<html><body></body></html>");
        let (path, param) =
            analyzer.detect_search(&doc, "https://wp.example.com", "wp.example.com", "dooplay");
        assert_eq!(path.as_deref(), Some("/"));
        assert_eq!(param.as_deref(), Some("s"));
    }

    #[test]
    fn script_urls_are_limited_and_same_origin_only() {
        let html = r#"<html><head>
          <script src="/_next/static/chunks/main.js"></script>
          <script src="https://cdn.therd.site/player.js"></script>
          <script src="/_next/static/chunks/player.js"></script>
          <script src="/_next/static/chunks/heavy.js"></script>
        </head></html>"#;

        let urls = SiteAnalyzer::script_urls(html, "https://site.example", 2);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0], "https://site.example/_next/static/chunks/main.js");
        assert!(urls.iter().all(|u| u.starts_with("https://site.example/")));
    }

    #[test]
    fn app_bundle_is_scanned_before_analytics_chunks() {
        let html = r#"<html><head>
          <script src="/scripts/tn/dit.svc.datahub.js"></script>
          <script src="/scripts/tn/adobe.min.js"></script>
          <script src="/_next/static/chunks/webpack.js"></script>
          <script src="/_next/static/chunks/pages/_app-33c.js"></script>
        </head></html>"#;
        let urls = SiteAnalyzer::script_urls(html, "https://site.example", 2);
        assert_eq!(urls.len(), 2);
        assert!(urls[0].contains("pages/_app"), "el bundle del reproductor debe ir primero: {:?}", urls);
    }

    #[test]
    fn direct_master_urls_and_playback_methods_are_concrete() {
        let url = SiteAnalyzer::direct_master_url(
            r#"{"stream":"https://cdn.example.com/hls/master.m3u8?token=abc"}"#,
        ).expect("máster HLS citado por el sitio");
        assert_eq!(url, "https://cdn.example.com/hls/master.m3u8?token=abc");
        assert!(SiteAnalyzer::direct_master_url("/shows/693ace640c94b06078d2a33f/").is_none());
        let caps = SiteCapabilities {
            player_cases: vec!["hls-master".to_string()],
            ..Default::default()
        };
        assert_eq!(SiteAnalyzer::playback_methods(&caps), vec!["HLS".to_string()]);
        assert_eq!(SiteAnalyzer::playback_methods(&SiteCapabilities::default()), vec!["Sin método confirmado".to_string()]);
    }

    #[test]
    fn detects_real_stream_capabilities_from_a_master_hls() {
        let html = r#"<html><body><script>
          const master = "https://cdn.example.com/hls/master.m3u8";
          const gql = "https://example.com/api/tn/app-shell/graphql/";
          //#EXT-X-MEDIA:TYPE=AUDIO,LANGUAGE="en",NAME="English [Original]"
          //#EXT-X-MEDIA:TYPE=SUBTITLES,LANGUAGE="es",NAME="Español"
          //#EXT-X-STREAM-INF:BANDWIDTH=1042180,RESOLUTION=1280x720
        </script></body></html>"#;

        let caps = SiteAnalyzer::detect_capabilities(html);
        assert!(caps.stream_formats.contains(&"hls".to_string()));
        assert!(caps.qualities.contains(&"720p".to_string()));
        assert!(caps.audio_languages.iter().any(|v| v.contains("English")));
        assert!(caps.subtitle_languages.iter().any(|v| v.contains("es")));
        assert!(caps.api_endpoints.iter().any(|v| v.contains("/graphql")));
    }

    #[test]
    fn detects_session_boot_capability_without_an_api_path() {
        let html = r#"<html><head>
          <meta name="globalAppVersion" content="10.12.1096">
        </head><body><script>
          const boot = "https://boot.example.com/v4/start";
          const stitcher = "https://cdn.example.com/v2/stitch/hls/x/master.m3u8";
        </script></body></html>"#;

        let caps = SiteAnalyzer::detect_capabilities(html);
        assert_eq!(caps.app_version.as_deref(), Some("10.12.1096"));
        assert!(caps.api_endpoints.iter().any(|v| v.contains("/v4/start")));
        assert!(caps.stream_formats.contains(&"hls".to_string()));
        assert!(caps.player_cases.iter().any(|c| c == "session-bootstrap"));
        assert!(caps.player_cases.iter().any(|c| c == "hls-master"));
    }

    #[test]
    fn classifies_graphql_drm_and_ad_cases_from_js_payloads() {
        let html = r#"<html><body><script>
          const ext = {"tnPersistedDocumentHash":"abc"};
          fetch("https://site.example/api/tn/app-shell/graphql/");
          const license = "com.widevine.alpha";
          const filler = "https://cdn.example.com/_ad/segment.ts";
          const mpd = "https://cdn.example.com/stream.mpd";
        </script></body></html>"#;

        let caps = SiteAnalyzer::detect_capabilities(html);
        assert!(caps.player_cases.iter().any(|c| c == "graphql-persisted"));
        assert!(caps.player_cases.iter().any(|c| c == "drm-widevine"));
        assert!(caps.player_cases.iter().any(|c| c == "ads-hls"));
        assert!(caps.player_cases.iter().any(|c| c == "dash-mpd"));
    }

    #[test]
    fn detects_region_and_locale_from_hydrated_state() {
        let html = r#"<html><body><script id="__NEXT_DATA__" type="application/json">
          {"props":{"pageProps":{"dehydratedState":{"queries":[{"queryKey":["region"],
          "state":{"data":{"region":{"locale":"es-419","region":"MX"}}}}]}}}}}
        </script></body></html>"#;

        let caps = SiteAnalyzer::detect_capabilities(html);
        assert_eq!(caps.region.as_deref(), Some("MX"));
        assert_eq!(caps.locale.as_deref(), Some("es-419"));
    }

    #[test]
    fn detects_region_from_escaped_rsc_payload() {
        let html = r#"<script>self.__next_f.push([1,"{\"queryKey\":[\"region\"],\"state\":{\"data\":{\"region\":{\"locale\":\"es-419\",\"region\":\"MX\"}}}}"])</script>"#;

        let caps = SiteAnalyzer::detect_capabilities(html);
        assert_eq!(caps.region.as_deref(), Some("MX"));
        assert_eq!(caps.locale.as_deref(), Some("es-419"));
    }

    #[test]
    fn capabilities_do_not_claim_formats_that_are_not_in_the_html() {
        let caps = SiteAnalyzer::detect_capabilities("<html><body>hola</body></html>");
        assert!(caps.stream_formats.is_empty());
        assert!(caps.qualities.is_empty());
        assert!(caps.api_endpoints.is_empty());
        assert!(caps.region.is_none());
        assert!(caps.locale.is_none());
    }

    #[test]
    fn structured_data_gives_real_catalog_search_and_language() {
        let html = r#"<html lang="en"><head>
<script type="application/ld+json">{"@context":"https://schema.org","@type":"WebSite","inLanguage":"en","potentialAction":{"@type":"SearchAction","target":{"@type":"EntryPoint","urlTemplate":"https://frame.example/catalog?name={search_term_string}"},"query-input":"required name=search_term_string"}}</script>
<script type="application/ld+json">{"@context":"https://schema.org","@type":"ItemList","name":"Hafta Topi","itemListElement":[{"@type":"ListItem","position":1,"url":"https://frame.example/cinema/one","name":"Titulo uno","image":"https://st.frame.example/one.webp"},{"@type":"ListItem","position":2,"url":"https://frame.example/cinema/two","name":"Titulo dos","alternateName":"English two","inLanguage":"uz"}]}</script>
</head><body>
<div id="app"></div>
<a href="/catalog?genreIds=1">Accion</a>
<a href="/catalog?genreIds=2">Drama</a>
<a href="/catalog?name=xxx">Buscar</a>
<a href="/catalog?page=2">2</a>
</body></html>"#;
        let doc = Html::parse_document(html);
        let origin = "https://frame.example";
        let host = "frame.example";

        assert_eq!(SiteAnalyzer::detect_language(&doc), "en");
        let (records, action) = SiteAnalyzer::detect_structured_data(&doc, origin, host);
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].title, "Titulo uno");
        assert_eq!(records[1].alternate_title.as_deref(), Some("English two"));
        assert!(records[0].image.as_deref().unwrap_or("").ends_with("one.webp"));
        let action = action.expect("SearchAction declarado");
        assert_eq!(action.query_param.as_deref(), Some("name"));
        assert_eq!(
            SiteAnalyzer::search_template_to_path(&action.url_template, origin, host),
            Some(("/catalog".to_string(), "name".to_string()))
        );

        let filters = SiteAnalyzer::detect_filter_params(&doc, origin, host, Some("name"));
        assert_eq!(filters.len(), 2, "sólo los valores reales de genreIds: {:?}", filters);
        assert!(filters.iter().all(|f| f.param == "genreIds"));
        assert!(filters.iter().all(|f| f.param != "page"));
    }

    #[test]
    fn search_template_from_another_host_is_not_used() {
        assert_eq!(
            SiteAnalyzer::search_template_to_path(
                "https://other.example/search?q={search_term_string}",
                "https://frame.example",
                "frame.example",
            ),
            None
        );
    }

    #[test]
    fn navigation_items_are_not_catalog_cards() {
        let html = r#"<!doctype html><html><body>
<header><nav class="header-nav">
  <div class="header-nav__item"><a href="/">Home</a></div>
  <div class="header-nav__item"><a href="/catalog">Catalog</a></div>
  <div class="header-nav__item"><a href="/mini-series">Mini series</a></div>
  <div class="header-nav__item"><a href="/tv">TV</a></div>
  <div class="header-nav__item"><a href="/tv-order">Buy a TV</a></div>
</nav></header>
<main><div class="swiper__movie"></div><div class="main__cards"></div></main>
<footer><div class="footer__item"><a href="/help">Help</a></div></footer>
</body></html>"#;
        let doc = Html::parse_document(html);
        let catalog = SiteAnalyzer::new().detect_catalog(&doc, "https://site.example", "site.example");
        assert_eq!(catalog.count, 0, "el menú no es un catálogo: {:?}", catalog.samples);
        assert!(catalog.card_selector.is_empty());
    }

    #[test]
    fn category_links_in_footer_are_real_path_filters() {
        let html = r#"<!doctype html><html><body>
<footer>
  <a href="/latam/movies/">Películas</a>
  <a href="/latam/movies/category/action-adventure/">Acción y Aventura</a>
  <a href="/latam/movies/category/comedy/">Comedia</a>
  <a href="/latam/shows/category/comedy/">Comedia</a>
  <a href="/latam/shows/category/drama/">Drama</a>
  <a href="/latam/shows/category/crime/">Crimen</a>
  <a href="/search?q=action">Buscar</a>
  <a href="/latam/kids/category/only-one/">Única</a>
</footer>
</body></html>"#;
        let doc = Html::parse_document(html);
        let filters = SiteAnalyzer::detect_path_filters(&doc, "https://site.example", "site.example");

        assert_eq!(filters.len(), 2, "sólo secciones con varias categorías: {:?}", filters);
        let movies = filters.iter().find(|f| f.base_path == "/latam/movies/").expect("películas");
        assert_eq!(movies.name, "Category (Movies)");
        assert_eq!(movies.segment, "category");
        assert_eq!(movies.values.len(), 2);
        assert_eq!(movies.values[0].label, "Acción y Aventura");
        assert_eq!(movies.values[0].value, "/latam/movies/category/action-adventure/");
        let shows = filters.iter().find(|f| f.base_path == "/latam/shows/").expect("series");
        assert_eq!(shows.name, "Category (Shows)");
        assert!(filters.iter().all(|f| f.values.iter().all(|v| !v.value.contains('?'))));
    }

    #[test]
    fn spa_payload_gives_locales_and_explicit_filters() {
        let html = r#"<!doctype html><html lang="uz"><head>
<script id="__NEXT_DATA__" type="application/json">{"locale":"uz","locales":["uz","ru","en"],"defaultLocale":"uz","props":{"filters":{"filterParams":[{"param":"sortBy","values":["publish_date","imdb_rating"]},{"param":"status","values":["announcement","release"]}]}},"messages":{"filter":{"publish_date":"Saralash","sort_by":"Saralash turi"}}}</script>
</head><body><div id="__next"></div></body></html>"#;
        let doc = Html::parse_document(html);
        assert_eq!(
            SiteAnalyzer::detect_site_locales(&doc),
            vec!["uz".to_string(), "ru".to_string(), "en".to_string()]
        );
        let filters = SiteAnalyzer::detect_payload_filters(&doc, None);
        assert_eq!(filters.len(), 4, "sólo los pares explícitos: {:?}", filters);
        assert!(filters.iter().all(|f| f.kind == "select"));
        assert!(filters.iter().any(|f| f.param == "sortBy" && f.value == "publish_date"));
        assert!(filters.iter().any(|f| f.param == "status" && f.value == "release"));
        assert!(filters.iter().all(|f| f.name.chars().all(|c| c.is_ascii())));
    }

    #[test]
    fn language_switcher_links_reveal_multilingual_sites() {
        let html = r#"<!doctype html><html lang="ru"><head>
<link rel="alternate" hreflang="tr" href="/tr">
</head><body>
<button data-url="/dk?cmd=lang&amp;st.lang=en&amp;st.cmd=main">English</button>
<button data-url="/dk?cmd=lang&amp;st.lang=uz&amp;st.cmd=main">O'zbek tili</button>
<a href="/locale/de/catalog">Deutsch</a>
<a href="/catalog?lang=auto">auto</a>
</body></html>"#;
        let doc = Html::parse_document(html);
        assert_eq!(
            SiteAnalyzer::detect_site_locales(&doc),
            vec!["ru".to_string(), "tr".to_string(), "en".to_string(), "uz".to_string(), "de".to_string()]
        );
    }

    #[test]
    fn js_bundles_give_real_filter_params_and_values() {
        let js = r#"let nm=["year","countryIds","genreIds","audioLanguageIds","quality"],nh=[...nm,"categoryIds","CopyrightHolderIds","isFrameDub","name","sortBy","status"],nv="publish_date";
const H=(0,a.useMemo)(()=>[{value:"publish_date",label:S("filter.publish_date")},{value:"imdb_rating",label:S("filter.imdb_rating")},{value:"kinopoisk_rating",label:S("filter.kinopoisk_rating")}],[S]),$=(0,a.useMemo)(()=>[{value:"announcement",label:S("filter.announcement")},{value:"release",label:S("filter.release")}],[S]);
render(nf,{instanceId:"catalog-sort",options:H,placeholder:S("filter.sort")});
render(nf,{instanceId:"catalog-status",options:$,placeholder:S("filter.status")});"#;
        let filters = SiteAnalyzer::detect_filters_from_js(js, Some("name"));
        let params: Vec<&str> = filters.iter().map(|f| f.param.as_str()).collect();
        for expected in ["year", "countryIds", "genreIds", "audioLanguageIds", "quality", "categoryIds", "CopyrightHolderIds", "sortBy", "status"] {
            assert!(params.contains(&expected), "falta {expected} en {:?}", params);
        }
        assert!(!params.contains(&"name"), "el parámetro de búsqueda no es un filtro");
        assert!(!params.contains(&"isFrameDub"), "el doblaje no verificado no es un filtro");
        assert!(!params.contains(&"publish_date"), "los valores no son parámetros");
        assert!(filters.iter().any(|f| f.param == "sortBy" && f.value == "imdb_rating"));
        assert!(filters.iter().any(|f| f.param == "status" && f.value == "announcement"));
        assert!(filters.iter().any(|f| f.param == "genreIds" && f.kind == "text"));
        assert!(filters.iter().all(|f| f.name.chars().all(|c| c.is_ascii())));
    }

    #[test]
    fn js_bundles_never_expose_credentials_or_internal_constants_as_filters() {
        let js = r#"let fields=["year","genreIds","userRegId","password","token","bearer","jwt","cookie","sessionId","apiKey","LS_ACCESS","DEFAULT_DATE_FORMAT","TEMP_DRAW_SUB_AMOUNT"],opts=[...fields,"status","sortBy"];
render(nf,{options:opts,instanceId:"catalog"});"#;
        let filters = SiteAnalyzer::detect_filters_from_js(js, Some("name"));
        let params: Vec<&str> = filters.iter().map(|f| f.param.as_str()).collect();
        for bogus in [
            "userRegId", "password", "token", "bearer", "jwt", "cookie", "sessionId",
            "apiKey", "LS_ACCESS", "DEFAULT_DATE_FORMAT", "TEMP_DRAW_SUB_AMOUNT",
        ] {
            assert!(!params.contains(&bogus), "parámetro interno expuesto como filtro: {bogus}");
        }
        assert!(params.contains(&"year"), "los filtros reales deben conservarse: {:?}", params);
        assert!(params.contains(&"genreIds"));
        assert!(params.contains(&"status"));
        assert!(params.contains(&"sortBy"));
    }

    #[test]
    fn js_bundles_never_turn_config_keys_into_filters() {
        // Array real de un bundle: mezcla parámetros de catálogo con claves de
        // configuración de la app. Antes, un solo `hits >= 2` publicaba TODOS
        // los tokens como filtros de texto.
        let js = r#"let nm=["year","countryIds","genreIds","audioLanguageIds","quality"],nh=[...nm,"categoryIds","CopyrightHolderIds","isFrameDub","name","sortBy","status","appDeeplinkScheme","appstoreLink","googleplayLink","authenticationCodeCooldown","authenticationQrCooldown","cookiesExpireInDays","gtmEvents","modals","siteUrl","clickMerchant","clickService","parseCardType","paymeMerchant","plumMerchant","renderCardError","resolveSubFeatureIcon"];
render(nf,{instanceId:"catalog",options:nh});"#;
        let filters = SiteAnalyzer::detect_filters_from_js(js, Some("name"));
        let params: Vec<&str> = filters.iter().map(|f| f.param.as_str()).collect();
        for expected in ["year", "countryIds", "genreIds", "audioLanguageIds", "quality", "categoryIds", "CopyrightHolderIds", "sortBy", "status"] {
            assert!(params.contains(&expected), "falta el filtro real {expected} en {:?}", params);
        }
        for bogus in [
            "appDeeplinkScheme", "appstoreLink", "googleplayLink", "authenticationCodeCooldown",
            "authenticationQrCooldown", "cookiesExpireInDays", "gtmEvents", "modals", "siteUrl",
            "clickMerchant", "clickService", "parseCardType", "paymeMerchant", "plumMerchant",
            "renderCardError", "resolveSubFeatureIcon",
        ] {
            assert!(!params.contains(&bogus), "clave de configuración expuesta como filtro: {bogus}");
        }
        assert!(!filters.iter().any(|f| f.param == "isFrameDub"), "Dubbed no es una pista verificada");
        assert!(filters.iter().all(|f| f.name.chars().all(|c| c.is_ascii())));
    }

    #[test]
    fn js_bundles_keep_generic_catalog_params_and_reject_config_ids() {
        let js = r#"let base=["year","format","season","status"],all=[...base,"clickMerchantId","paymeMerchantId","appDeeplinkScheme","renderCardError"];
render(nf,{instanceId:"catalog",options:all});"#;
        let filters = SiteAnalyzer::detect_filters_from_js(js, Some("name"));
        let params: Vec<&str> = filters.iter().map(|f| f.param.as_str()).collect();
        for expected in ["year", "format", "season", "status"] {
            assert!(params.contains(&expected), "falta el parámetro {expected} en {:?}", params);
        }
        for bogus in ["clickMerchantId", "paymeMerchantId", "appDeeplinkScheme", "renderCardError"] {
            assert!(!params.contains(&bogus), "clave de configuración expuesta como filtro: {bogus}");
        }
    }

    #[test]
    fn normalize_filters_drops_search_dub_and_duplicate_tuples() {
        let mut filters = vec![
            SiteFilterParam { name: "Search".to_string(), param: "name".to_string(), value: String::new(), kind: "text".to_string() },
            SiteFilterParam { name: "Search".to_string(), param: "q".to_string(), value: String::new(), kind: "text".to_string() },
            SiteFilterParam { name: "Dubbed".to_string(), param: "isFrameDub".to_string(), value: String::new(), kind: "check".to_string() },
            SiteFilterParam { name: "Sort By".to_string(), param: "sort".to_string(), value: String::new(), kind: "text".to_string() },
            SiteFilterParam { name: "Sort By".to_string(), param: "sortBy".to_string(), value: "publish_date".to_string(), kind: "select".to_string() },
            SiteFilterParam { name: "Sort By".to_string(), param: "sortBy".to_string(), value: "imdb_rating".to_string(), kind: "select".to_string() },
            SiteFilterParam { name: "Sort By".to_string(), param: "sortBy".to_string(), value: "imdb_rating".to_string(), kind: "select".to_string() },
        ];

        SiteAnalyzer::normalize_filter_params(&mut filters);
        let params: Vec<&str> = filters.iter().map(|f| f.param.as_str()).collect();
        assert_eq!(
            params,
            vec!["sort", "sortBy", "sortBy"],
            "se conservan los parámetros reales y sólo se deduplica el duplicado exacto: {:?}",
            filters
        );
        assert!(
            filters.iter().any(|f| f.param == "sort" && f.kind == "text" && f.value.is_empty()),
            "un parámetro real sin valores debe seguir siendo TextFilter: {:?}",
            filters
        );
        assert!(!params.iter().any(|p| *p == "name" || *p == "q" || *p == "isFrameDub"));
    }

    #[test]
    fn js_bundles_reveal_rest_api_bases_and_resource_paths() {
        let js = r#"let apiVersion="v1";let f=axios.create({baseURL:"https://api.example.com/user/api/"});
genres:{getGenres:async()=>await client.get(apiVersion+"/genres")},
categories:{getCategories:async()=>await client.get("/categories")},
getHistory:async()=>await client.get(apiVersion+"/profile-histories")},
render(nf,{href:"/catalog?genreIds=1"});"#;
        let candidates = SiteAnalyzer::detect_api_candidates(js);
        assert_eq!(candidates.bases, vec!["https://api.example.com/user/api"]);
        assert_eq!(candidates.version.as_deref(), Some("v1"));
        assert!(candidates.paths.contains(&"/genres".to_string()), "{:?}", candidates.paths);
        assert!(candidates.paths.contains(&"/categories".to_string()), "{:?}", candidates.paths);
        assert!(!candidates.paths.iter().any(|p| p.contains("profile")), "las rutas privadas no son recursos: {:?}", candidates.paths);
        assert!(!candidates.paths.iter().any(|p| p == "/catalog"), "una ruta de UI fuera de .get() no es un recurso API: {:?}", candidates.paths);
    }

    #[test]
    fn api_lists_only_store_real_json_arrays_with_names() {
        let json: serde_json::Value = serde_json::from_str(
            r#"[{"id":1,"name":"Action"},{"id":4,"name":"Comedy"},{"id":9,"name":"Horror"}]"#,
        )
        .unwrap();
        let result = SiteAnalyzer::parse_api_list("https://api.example.com/user/api/v1/genres", &json).unwrap();
        assert_eq!(result.resource, "genres");
        assert_eq!(result.record_count, 3);
        assert_eq!(result.options.len(), 3);
        assert_eq!(result.options[1].value, "4");
        assert_eq!(result.options[1].label, "Comedy");
        assert_eq!(result.title_field, "name");
        assert!(result.sample_json.contains("Comedy"));

        let wrapped: serde_json::Value = serde_json::from_str(r#"{"data":[{"slug":" movies ","title":"Movie"}]}"#).unwrap();
        let wrapped = SiteAnalyzer::parse_api_list("https://api.example.com/v1/contents", &wrapped).unwrap();
        assert_eq!(wrapped.resource, "contents");
        assert_eq!(wrapped.options[0].value, "movies");
        assert_eq!(wrapped.title_field, "title");

        let not_a_list: serde_json::Value = serde_json::from_str(r#"{"error":"unauthorized"}"#).unwrap();
        assert!(SiteAnalyzer::parse_api_list("https://api.example.com/v1/genres", &not_a_list).is_none());
    }

    #[test]
    fn minified_bundle_version_is_resolved_from_its_variable() {
        // El primer `n` del bundle es otra variable (por ejemplo un tipo); el
        // detector debe seguir buscando hasta encontrar un valor de versión.
        let js = r#"let n="function";41423,e=>{"use strict";e.s(["LS_ACCESS",()=>o,"apiVersion",()=>n]);let n="v1",r="id-r",o="ac-s";}"#;
        assert_eq!(SiteAnalyzer::detect_api_version(js).as_deref(), Some("v1"));
    }

    #[test]
    fn api_path_priority_puts_filter_endpoints_first() {
        let filters = SiteAnalyzer::api_path_priority("/filters/genres");
        let genres = SiteAnalyzer::api_path_priority("/genres");
        let categories = SiteAnalyzer::api_path_priority("/categories");
        let other = SiteAnalyzer::api_path_priority("/banners");
        assert_eq!(filters, 0, "un endpoint /filters/* debe sondearse antes que nada");
        assert!(filters < genres);
        assert!(genres < other);
        assert!(categories < other);
        assert_eq!(genres, SiteAnalyzer::api_path_priority("genres"));
    }

    #[test]
    fn api_probes_interleave_all_bases_before_falling_back() {
        let candidates = ApiCandidates {
            bases: vec![
                "https://api.example.com/auth/api".to_string(),
                "https://api.example.com/user/api".to_string(),
            ],
            paths: vec![
                "/banners".to_string(),
                "/contents".to_string(),
                "/categories".to_string(),
                "/genres".to_string(),
            ],
            version: Some("v1".to_string()),
        };
        let mut ordered = candidates.paths.clone();
        ordered.sort_by_key(|path| SiteAnalyzer::api_path_priority(path));
        let urls = SiteAnalyzer::api_probe_urls(&candidates, &ordered);
        assert!(urls[0].contains("/user/api/v1/genres"), "la base pública va primero: {:?}", urls);
        assert!(urls[1].contains("/user/api/genres"), "el fallback sin versión se intercala: {:?}", urls);
        assert!(urls.iter().take(12).any(|u| u.contains("/auth/api/v1/genres")), "la base de autenticación también se prueba: {:?}", urls);
        assert!(urls.iter().take(12).any(|u| u.contains("/user/api/v1/contents")));
        assert!(urls.iter().all(|u| u.contains("/v1/") || !u.contains("/v")), "sin versión sólo como fallback: {:?}", urls);
    }

    #[test]
    fn api_probes_stay_on_the_sites_host_family() {
        assert!(SiteAnalyzer::host_related_to("https://api.frame.uz/user/api/v1/genres", "frame.uz"));
        assert!(SiteAnalyzer::host_related_to("https://api.catalog.example/list", "www.catalog.example"));
        assert!(!SiteAnalyzer::host_related_to("https://third-party.example/genres", "frame.uz"));
    }

    #[test]
    fn html_selects_and_empty_query_params_are_real_filters() {
        let html = r#"<html><body>
<a href="/catalog?genreIds=">Todos</a>
<select name="year"><option value="2024">2024</option><option value="2023">2023</option></select>
<select name="name"><option value="x">x</option></select>
<input name="isFrameDub" value="true">
</body></html>"#;
        let doc = Html::parse_document(html);
        let filters =
            SiteAnalyzer::detect_filter_params(&doc, "https://example.com", "example.com", Some("name"));
        assert!(
            filters.iter().any(|f| f.param == "genreIds" && f.kind == "text" && f.value.is_empty()),
            "un enlace `?genreIds=` declara el parámetro aunque rellene el valor en cliente: {:?}",
            filters
        );
        assert_eq!(filters.iter().filter(|f| f.param == "year").count(), 2);
        assert!(filters.iter().all(|f| f.param != "name" && f.param != "isFrameDub"));
    }

    #[test]
    fn js_query_strings_and_param_objects_reveal_catalog_filters() {
        let js = r#"let a="/catalog?genreIds=1&year=2024&isFrameDub=true&page=2&id=uz.frame.cinema";
let b = paramsToQuery({categoryIds:t("category_id"), profile_id:e, sortBy:"year"});"#;
        let filters = SiteAnalyzer::detect_filters_from_js(js, Some("name"));
        let params: Vec<&str> = filters.iter().map(|f| f.param.as_str()).collect();
        for expected in ["genreIds", "year", "categoryIds", "sortBy"] {
            assert!(params.contains(&expected), "falta {expected} en {:?}", params);
        }
        assert!(filters.iter().any(|f| f.param == "genreIds" && f.value == "1"));
        assert!(filters.iter().any(|f| f.param == "year" && f.value == "2024"));
        assert!(filters.iter().any(|f| f.param == "categoryIds" && f.kind == "text"));
        for bogus in ["isFrameDub", "page", "id", "profile_id", "name"] {
            assert!(!params.contains(&bogus), "parámetro no filtrable expuesto: {bogus}");
        }
    }

    #[test]
    fn scalar_api_lists_become_filter_options() {
        let years: serde_json::Value =
            serde_json::from_str(r#"[{"year":2026,"sort_order":1},{"year":2025,"sort_order":2}]"#).unwrap();
        let result = SiteAnalyzer::parse_api_list(
            "https://api.example/user/api/v1/filters/years",
            &years,
        )
        .unwrap();
        assert_eq!(result.options.len(), 2);
        assert_eq!(result.options[0].value, "2026");
        assert_eq!(result.options[0].label, "2026");

        let qualities: serde_json::Value =
            serde_json::from_str(r#"[{"quality":"Sd"},{"quality":"Hd"}]"#).unwrap();
        let result = SiteAnalyzer::parse_api_list(
            "https://api.example/user/api/v1/filters/qualities",
            &qualities,
        )
        .unwrap();
        assert_eq!(result.options.len(), 2);
        assert_eq!(result.options[0].value, "Sd");

        let companies: serde_json::Value =
            serde_json::from_str(r#"[{"company_name":"Warner"},{"company_name":"Disney"}]"#).unwrap();
        let result = SiteAnalyzer::parse_api_list(
            "https://api.example/user/api/v1/filters/copyright-holders",
            &companies,
        )
        .unwrap();
        assert_eq!(result.options.len(), 2);
        assert_eq!(result.options[1].label, "Disney");
    }

    #[test]
    fn catalog_card_genre_selector_is_detected_without_site_specific_rules() {
        let html = r#"<html><body>
            <article class="card"><a href="/anime/1"><h3>Uno</h3></a>
                <div class="genres">
                    <a href="/genero/accion">Acción</a>
                    <a href="/genero/drama">Drama</a>
                </div>
            </article>
            <article class="card"><a href="/anime/2"><h3>Dos</h3></a>
                <div class="genres"><a href="/genero/accion">Acción</a></div>
            </article>
        </body></html>"#;
        let doc = Html::parse_document(html);
        let selector = SiteAnalyzer::infer_genre_selector(&doc, "article.card");
        assert!(selector.contains("href*='/genero/'"), "{}", selector);

        // Sin tarjetas que inspeccionar no se declara ningún selector.
        let empty = SiteAnalyzer::infer_genre_selector(&doc, "article.no-such-card");
        assert!(empty.is_empty(), "{}", empty);
    }
