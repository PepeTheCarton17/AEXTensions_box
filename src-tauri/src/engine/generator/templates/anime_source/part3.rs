/// Parte 3 de la plantilla de la fuente Kotlin de anime.

macro_rules! anime_source_kt_part3 {
    () => { r#"
    // ============================== DETAILS ==============================
    override fun animeDetailsParse(document: Document): SAnime = SAnime.create().apply {{
        title = document.selectFirst("h1, .title, .entry-title")?.text()?.trim()
            ?.takeIf {{ it.isNotEmpty() }}
            ?: document.selectFirst("meta[property=og:title]")?.attr("content")
            ?: "Anime"
        description = document.selectFirst("p.sinopsis, div.description, .entry-content p")?.text()?.trim()
            ?.takeIf {{ it.isNotEmpty() }}
            ?: document.selectFirst("meta[name=description], meta[property=og:description]")?.attr("content")
        genre = document.select("a[href*=/genero/], a[href*=/genre/], .genres a").joinToString(", ") {{ it.text() }}
        status = SAnime.UNKNOWN
        thumbnail_url = document.selectFirst("img.poster, .poster img")?.attr("abs:src")
            ?: document.selectFirst("meta[property=og:image]")?.attr("content")
            ?: document.selectFirst("article img, main img")?.attr("abs:src")
    }}

    // ============================== EPISODES ==============================
    override fun episodeListSelector(): String = "{episode_sel}"

    override fun episodeFromElement(element: Element): SEpisode = SEpisode.create().apply {{
        // La tarjeta puede ser el propio enlace.
        val link = if (element.tagName() == "a") element else element.selectFirst("a")
        setUrlWithoutDomain(link?.attr("href") ?: "")
        // Nombre: un título explícito o los dos primeros textos cortos. En
        // tarjetas como las de Pluto TV el primero es "T1 E1" y el segundo el
        // título del episodio; así no se cuela la sinopsis completa.
        val explicitTitle = element.selectFirst(".title, [class*=episodeTitle], h3, h4")
            ?.text()?.trim()?.takeIf {{ it.isNotEmpty() }}
        val shortTexts = element.select("span")
            .map {{ it.text().trim() }}
            .filter {{ it.isNotEmpty() && it.length <= 60 }}
            .take(2)
            .joinToString(" ")
        name = explicitTitle
            ?: shortTexts.takeIf {{ it.isNotEmpty() }}
            ?: link?.text()?.trim()?.takeIf {{ it.isNotEmpty() }}
            ?: "Episodio"
        val seasonEpisode = Regex("""[TS]\s*\d+\s*[Ee]\s*(\d+)""", RegexOption.IGNORE_CASE).find(name)
        val wordEpisode = Regex("""(?:episodio|capitulo|capítulo|ep)\s*[-#:]?\s*(\d+)""", RegexOption.IGNORE_CASE).find(name)
        episode_number = (seasonEpisode?.groupValues?.get(1) ?: wordEpisode?.groupValues?.get(1))
            ?.toFloatOrNull() ?: 1f
    }}

    override fun videoFromElement(element: Element): Video = throw UnsupportedOperationException()
    override fun videoListSelector(): String = throw UnsupportedOperationException()
    override fun videoUrlParse(document: Document): String = throw UnsupportedOperationException()

    // Capacidades detectadas por AEXT en el análisis del sitio. No sustituyen
    // a la verificación real: sólo ordenan los candidatos a probar.
    private val siteStreamFormats = setOf<String>({site_cap_formats})
    private val siteAppVersion = "{site_app_version}"
    private val sitePlayerCases = setOf<String>({site_player_cases})
    private val siteRegion = "{site_region}"
    private val siteLocale = "{site_locale}"

{video_engine}

    // ============================== PREFERENCES ==============================
    override fun setupPreferenceScreen(screen: PreferenceScreen) {{
{audio_pref_block}        val apiPref = ListPreference(screen.context).apply {{
            key = "api_mode"
            title = "{pref_api_title}"
            entries = arrayOf({pref_api_entries})
            entryValues = arrayOf("auto", "session", "graphql", "html")
            setDefaultValue("auto")
            summary = "%s"
        }}
        screen.addPreference(apiPref)

        val methodsPref = ListPreference(screen.context).apply {{
            key = "aext_methods_found"
            title = "{methods_title}"
            entries = arrayOf({methods_entries})
            entryValues = arrayOf({methods_entries})
            summary = "{methods_summary}"
            setDefaultValue(arrayOf({methods_entries}).first())
        }}
        screen.addPreference(methodsPref)

        val serverPref = ListPreference(screen.context).apply {{
            key = "preferred_server"
            title = "{pref_server_title}"
            entries = arrayOf("Any", "MP4Upload", "Voe", "Byse (StreamWish)", "Zilla (HLS)", "Direct")
            entryValues = arrayOf("any", "mp4upload", "voe", "byse", "zilla", "direct")
            setDefaultValue("any")
            summary = "{pref_server_summary} — %s"
        }}
        screen.addPreference(serverPref)

        val qualityPref = ListPreference(screen.context).apply {{
            key = "preferred_quality"
            title = "{pref_quality_title}"
            entries = arrayOf("1080p", "720p", "480p", "360p")
            entryValues = arrayOf("1080p", "720p", "480p", "360p")
            setDefaultValue("1080p")
            summary = "%s"
        }}
        screen.addPreference(qualityPref)

        val serverBlacklistPref = MultiSelectListPreference(screen.context).apply {{
            key = "server_blacklist"
            title = "{pref_server_blacklist_title}"
            entries = arrayOf("MP4Upload", "Voe", "Byse (StreamWish)", "Zilla (HLS)", "Directos")
            entryValues = arrayOf("mp4upload", "voe", "byse", "zilla", "direct")
            setDefaultValue(emptySet<String>())
            summary = "{pref_server_blacklist_summary}"
        }}
        screen.addPreference(serverBlacklistPref)

        val genreBlacklistPref = EditTextPreference(screen.context).apply {{
            key = "genre_blacklist_text"
            title = "{pref_genre_blacklist_title}"
            setDefaultValue("")
            summary = "{pref_genre_blacklist_summary}"
            dialogTitle = "{pref_genre_blacklist_title}"
        }}
        screen.addPreference(genreBlacklistPref)
    }}
}}
"# };
}
pub(crate) use anime_source_kt_part3;
