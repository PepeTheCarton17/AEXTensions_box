/// Parte 2 de la plantilla de la fuente Kotlin de anime.

macro_rules! anime_source_kt_part2 {
    () => { r#"        Injekt.get<Application>().getSharedPreferences("source_$id", 0)
    }}

    override fun headersBuilder(): Headers.Builder = Headers.Builder()
        .add("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .add("Referer", "$baseUrl/")

    // ============================== POPULAR ANIME ==============================
    override fun popularAnimeRequest(page: Int): Request = GET(pageUrl(listingPath, page), headers)

    override fun popularAnimeSelector(): String = "{card_sel}"

    override fun popularAnimeFromElement(element: Element): SAnime = SAnime.create().apply {{
        // En muchas portadas la tarjeta ES el enlace (`a.card`): `selectFirst`
        // solo busca descendientes, así que hay que aceptar el propio nodo.
        val link = if (element.tagName() == "a") element else element.selectFirst("a")
        setUrlWithoutDomain(link?.attr("href") ?: "")
        title = pickTitle(element, "{title_sel}")
        thumbnail_url = element.selectFirst("img")
            ?.let {{ img ->
                img.attr("abs:src")
                    .ifBlank {{ img.attr("abs:data-src") }}
                    .ifBlank {{ img.attr("abs:data-lazy-src") }}
            }}
            ?.takeIf {{ it.isNotEmpty() }}
    }}

    override fun popularAnimeNextPageSelector(): String? = "{next_sel}".ifBlank {{ null }}

    override fun popularAnimeParse(response: Response): AnimesPage {{
        val page = filterBlacklistedAnimes(super.popularAnimeParse(response))
        return withJsonLdFallback(page, "")
    }}

    private fun withJsonLdFallback(page: AnimesPage, query: String): AnimesPage {{
        var animes = page.animes
        if (query.isNotBlank()) {{
            val q = query.trim().lowercase()
            animes = animes.filter {{ it.title.lowercase().contains(q) }}
        }}
        if (animes.isEmpty() && jsonLdRecords.isNotEmpty()) {{
            animes = jsonLdRecords
                .map(::recordToAnime)
                .filter {{ anime -> query.isBlank() || anime.title.lowercase().contains(query.trim().lowercase()) }}
            return AnimesPage(animes, false)
        }}
        return AnimesPage(animes, page.hasNextPage)
    }}

    // ============================== LATEST UPDATES ==============================
    override fun latestUpdatesRequest(page: Int): Request = GET(pageUrl(latestPath, page), headers)

    override fun latestUpdatesSelector(): String = "{card_sel}"

    override fun latestUpdatesFromElement(element: Element): SAnime = popularAnimeFromElement(element)

    override fun latestUpdatesNextPageSelector(): String? = "{next_sel}".ifBlank {{ null }}

    override fun latestUpdatesParse(response: Response): AnimesPage {{
        val page = filterBlacklistedAnimes(super.latestUpdatesParse(response))
        return withJsonLdFallback(page, "")
    }}

    // ============================== SEARCH ANIME ==============================
    override fun searchAnimeRequest(page: Int, query: String, filters: AnimeFilterList): Request {{
        val cleanQuery = query.trim()
{exclusion_capture}{search_audio_block}
        val url = if (finalQuery.isNotBlank()) {{
            searchUrl(finalQuery, page)
        }} else {{
            pageUrl(listingPath, page)
        }}
        lastSearchQuery = finalQuery
{filter_block}        return GET(finalUrl, headers)
    }}

    private var lastSearchQuery = ""

    override fun searchAnimeSelector(): String = "{card_sel}"

    override fun searchAnimeFromElement(element: Element): SAnime = popularAnimeFromElement(element)

    override fun searchAnimeNextPageSelector(): String? = "{next_sel}".ifBlank {{ null }}

    override fun searchAnimeParse(response: Response): AnimesPage {{
        val page = filterBlacklistedAnimes(super.searchAnimeParse(response))
        // Sin endpoint de búsqueda, el sitio devuelve el catálogo sin filtrar.
        // Filtrar localmente es honesto: no fingimos que el servidor buscó.
        return withJsonLdFallback(page, lastSearchQuery)
    }}

    private fun filterBlacklistedAnimes(page: AnimesPage): AnimesPage {{
        val rawBlacklist = preferences.getString("genre_blacklist_text", "") ?: ""
        val blacklistWords = rawBlacklist
            .split(",")
            .map {{ it.trim().lowercase() }}
            .filter {{ it.isNotEmpty() }}
        // Valores elegidos en el filtro de exclusión: ids/nombres reales de las
        // listas verificadas de la API del sitio, no palabras escritas a mano.
        val excludedApi = (preferences.getStringSet("excluded_api_values", mutableSetOf<String>()) ?: emptySet())
            .map {{ it.lowercase() }}
            .filter {{ it.isNotBlank() }}

        if (blacklistWords.isEmpty() && excludedApi.isEmpty()) return page

        val filtered = page.animes.filterNot {{ anime ->
            val genreLower = anime.genre?.lowercase() ?: ""
            val genreWords = genreLower.split(",", ";", "|")
                .map {{ it.trim() }}
                .filter {{ it.isNotEmpty() }}
            val excludedHit = excludedApi.any {{ word ->
                genreWords.any {{ genre -> genre == word || genre.contains(word) }}
            }}
            val titleLower = anime.title.lowercase()
            excludedHit || blacklistWords.any {{ word ->
                (genreLower.isNotEmpty() && genreLower.contains(word)) || titleLower.contains(word)
            }}
        }}
        return AnimesPage(filtered, page.hasNextPage)
    }}

    override fun getFilterList(): AnimeFilterList = {class_name}Filters.getFilterList()
"# };
}
pub(crate) use anime_source_kt_part2;
