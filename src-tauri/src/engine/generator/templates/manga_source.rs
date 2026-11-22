/// Plantilla de la fuente Kotlin de manga.

macro_rules! manga_source_kt_fmt {
    () => { r#"package {pkg_prefix}.{pkg_lang}.{pkg_slug}

import android.net.Uri
import eu.kanade.tachiyomi.network.GET
import eu.kanade.tachiyomi.source.model.FilterList
import eu.kanade.tachiyomi.source.model.Page
import eu.kanade.tachiyomi.source.model.SChapter
import eu.kanade.tachiyomi.source.model.SManga
import eu.kanade.tachiyomi.source.online.ParsedHttpSource
import eu.kanade.tachiyomi.util.asJsoup
import okhttp3.Headers
import okhttp3.OkHttpClient
import okhttp3.Request
import org.jsoup.nodes.Document
import org.jsoup.nodes.Element

class {class_name} : ParsedHttpSource() {{

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

    override fun headersBuilder(): Headers.Builder = super.headersBuilder()
        .add("Referer", "$baseUrl/")

    // ============================== POPULAR ==============================
    override fun popularMangaRequest(page: Int): Request = GET(pageUrl(listingPath, page), headers)

    override fun popularMangaSelector(): String = "{card_sel}"

    override fun popularMangaFromElement(element: Element): SManga = SManga.create().apply {{
        // La tarjeta puede ser el propio enlace (`a.card`), no un contenedor.
        val link = if (element.tagName() == "a") element else element.selectFirst("a")
        setUrlWithoutDomain(link?.attr("href") ?: "")
        title = element.selectFirst("{title_sel}")?.text()?.trim()?.takeIf {{ it.isNotEmpty() }}
            ?: link?.attr("title")?.trim()?.takeIf {{ it.isNotEmpty() }}
            ?: element.selectFirst("img[alt]")?.attr("alt")?.trim()?.takeIf {{ it.isNotEmpty() }}
            ?: link?.text()?.trim()?.takeIf {{ it.isNotEmpty() }}
            ?: "Sin Título"
        thumbnail_url = element.selectFirst("img")
            ?.let {{ img ->
                img.attr("abs:src")
                    .ifBlank {{ img.attr("abs:data-src") }}
                    .ifBlank {{ img.attr("abs:data-lazy-src") }}
            }}
            ?.takeIf {{ it.isNotEmpty() }}
    }}

    override fun popularMangaNextPageSelector(): String? = "{next_sel}".ifBlank {{ null }}

    // ============================== LATEST ==============================
    override fun latestUpdatesRequest(page: Int): Request = GET(pageUrl(latestPath, page), headers)

    override fun latestUpdatesSelector(): String = "{card_sel}"

    override fun latestUpdatesFromElement(element: Element): SManga = popularMangaFromElement(element)

    override fun latestUpdatesNextPageSelector(): String? = "{next_sel}".ifBlank {{ null }}

    // ============================== SEARCH ==============================
    override fun searchMangaRequest(page: Int, query: String, filters: FilterList): Request =
        GET(searchUrl(query.trim(), page), headers)

    override fun searchMangaSelector(): String = "{card_sel}"

    override fun searchMangaFromElement(element: Element): SManga = popularMangaFromElement(element)

    override fun searchMangaNextPageSelector(): String? = "{next_sel}".ifBlank {{ null }}

    override fun getFilterList(): FilterList = {class_name}Filters.getFilterList()

    // ============================== DETAILS ==============================
    override fun mangaDetailsParse(document: Document): SManga = SManga.create().apply {{
        title = document.selectFirst("h1, .post-title h1, .entry-title")?.text() ?: "Manga"
        author = document.selectFirst(".author a, .author-content a")?.text()
        description = document.selectFirst(".description-summary, .summary__content, .entry-content p")?.text()
        genre = document.select("a[href*=/genre/], a[href*=/genero/], .genres a").joinToString(", ") {{ it.text() }}
        status = SManga.UNKNOWN
        thumbnail_url = document.selectFirst("img.poster, .summary_image img")?.attr("abs:src")
    }}

    // ============================== CHAPTERS ==============================
    override fun chapterListSelector(): String = "li.wp-manga-chapter, .chapter-list a, a[href*=/chapter/]"

    override fun chapterFromElement(element: Element): SChapter = SChapter.create().apply {{
        val link = if (element.tagName() == "a") element else element.selectFirst("a")
        setUrlWithoutDomain(link?.attr("href") ?: "")
        name = element.selectFirst(".chapter-name, .title")?.text() ?: link?.text() ?: "Capítulo"
    }}

    // ============================== PAGES ==============================
    override fun pageListParse(document: Document): List<Page> =
        document.select("div.page-break img, div.reading-content img, #readerarea img").mapIndexed {{ index, img ->
            val url = img.attr("abs:data-src").ifEmpty {{ img.attr("abs:src") }}
            Page(index, "", url)
        }}

    override fun imageUrlParse(document: Document): String = ""
}}
"# };
}
pub(crate) use manga_source_kt_fmt;

/// Plantilla del archivo de filtros de manga.
macro_rules! manga_filters_kt_fmt {
    () => { r#"package {pkg_prefix}.{pkg_lang}.{pkg_slug}

import eu.kanade.tachiyomi.source.model.FilterList

object {class_name}Filters {{
    fun getFilterList(): FilterList = FilterList()
}}
"# };
}
pub(crate) use manga_filters_kt_fmt;

