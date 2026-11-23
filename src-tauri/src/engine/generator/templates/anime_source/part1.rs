/// Parte 1 de la plantilla de la fuente Kotlin de anime.

macro_rules! anime_source_kt_part1 {
    () => { r#"package {pkg_prefix}.{pkg_lang}.{pkg_slug}

import android.app.Application
import android.content.SharedPreferences
import android.net.Uri
import android.util.Log
import androidx.preference.EditTextPreference
import androidx.preference.ListPreference
import androidx.preference.MultiSelectListPreference
import androidx.preference.PreferenceScreen
import eu.kanade.tachiyomi.aext.AextEngineRegistry
import eu.kanade.tachiyomi.aext.AextVideoContext
import eu.kanade.tachiyomi.animesource.ConfigurableAnimeSource
import eu.kanade.tachiyomi.animesource.model.AnimeFilterList
import eu.kanade.tachiyomi.animesource.model.AnimesPage
import eu.kanade.tachiyomi.animesource.model.SAnime
import eu.kanade.tachiyomi.animesource.model.SEpisode
import eu.kanade.tachiyomi.animesource.model.Video
import eu.kanade.tachiyomi.animesource.online.ParsedAnimeHttpSource
import eu.kanade.tachiyomi.network.GET
import eu.kanade.tachiyomi.network.POST
import eu.kanade.tachiyomi.util.asJsoup
import okhttp3.Headers
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import org.jsoup.nodes.Document
import org.jsoup.nodes.Element
import org.jsoup.Jsoup
import uy.kohesive.injekt.Injekt
import uy.kohesive.injekt.api.get

class {class_name} : ParsedAnimeHttpSource(), ConfigurableAnimeSource {{

    override val name = "{name}"

    override val baseUrl = "{base_url}"

    override val lang = "{pkg_lang}"

    override val supportsLatest = {supports_latest}

    override val client: OkHttpClient = network.cloudflareClient

    // Rutas detectadas durante el análisis (vacías si no se encontraron).
    private val listingPath = "{popular_path}"
    private val latestPath = "{latest_path}"
    private val searchPath = "{search_path}"
    private val searchParam = "{search_param}"
    private val pageLanguage = "{detected_language}"

    // Títulos reales declarados por el sitio con schema.org ItemList. Se usan
    // sólo cuando el HTML no expone tarjetas (SPA); nunca se inventan títulos.
    private data class LdRecord(
        val title: String,
        val alternateTitle: String?,
        val language: String?,
        val url: String,
        val image: String?,
        val genre: String? = null
    )

{ld_records_block}

    private fun recordToAnime(record: LdRecord): SAnime = SAnime.create().apply {{
        setUrlWithoutDomain(record.url)
        title = preferredRecordTitle(record)
        thumbnail_url = record.image
        genre = record.genre
    }}

    /** Idioma pedido > alternativa en inglés > nombre original del sitio. */
    private fun preferredRecordTitle(record: LdRecord): String {{
        val alt = record.alternateTitle?.takeIf {{ it.isNotBlank() }}
        return when {{
            alt != null && (lang == "en" || record.language != lang) -> alt
            else -> record.title
        }}
    }}

    /** Prioriza atributos localizados reales; el texto visible manda si la página ya está en el idioma pedido. */
    private fun pickTitle(element: Element, titleSelector: String): String {{
        val text = element.selectFirst(titleSelector)?.text()?.trim().orEmpty()
            .ifBlank {{ element.selectFirst("h2, h3, .title, [class*=title]")?.text()?.trim().orEmpty() }}
        fun attr(name: String): String = element.attr(name).trim()
            .ifBlank {{ element.selectFirst("[$name]")?.attr(name)?.trim().orEmpty() }}
        val attrs = if (lang == "en") {{
            listOf("data-title-en", "title")
        }} else {{
            listOf("data-title-{detected_language}", "data-title-en", "title")
        }}
        if (pageLanguage == lang && text.isNotBlank()) return text
        for (name in attrs) {{
            val value = attr(name)
            if (value.isNotBlank()) return value
        }}
        return text.ifBlank {{ attr("alt") }}.ifBlank {{ "Sin Título" }}
    }}

    /** Etiquetas reales de la tarjeta (género/categoría/tag), nunca inventadas. */
    private fun pickGenres(element: Element, genreSelector: String): String? {{
        if (genreSelector.isBlank()) return null
        val values = runCatching {{
            element.select(genreSelector)
                .map {{ it.text().trim() }}
                .filter {{ it.isNotEmpty() && it.length <= 60 }}
                .distinct()
        }}.getOrDefault(emptyList<String>())
        return values.takeIf {{ it.isNotEmpty() }}?.joinToString(", ")
    }}

    private fun pageUrl(path: String, page: Int): String {{
        val clean = if (path.isEmpty()) "/" else if (path.startsWith("/")) path else "/$path"
        val base = "$baseUrl$clean"
        return when ("{page_style}") {{
            "path" -> if (page <= 1) base else "$baseUrl${{clean.trimEnd('/')}}/page/$page/"
            "query" -> if (page <= 1) base
                else "$base${{if (base.contains("?")) "&" else "?"}}page=$page"
            else -> base
        }}
    }}

    private fun searchUrl(query: String, page: Int): String {{
        val path = if (searchPath.isEmpty()) "/" else if (searchPath.startsWith("/")) searchPath else "/$searchPath"
        if (searchPath.isEmpty() || searchParam.isEmpty()) {{
            return pageUrl(listingPath, page)
        }}
        val base = "$baseUrl$path"
        val sep = if (base.contains("?")) "&" else "?"
        val encoded = Uri.encode(query)
        val first = "$base$sep$searchParam=$encoded"
        return if (page <= 1) first else "$first&page=$page"
    }}

    private val preferences: SharedPreferences by lazy {{
"# };
}
pub(crate) use anime_source_kt_part1;
