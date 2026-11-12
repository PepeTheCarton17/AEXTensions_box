use crate::engine::analyzer::AnalysisResult;
use std::collections::HashMap;

pub struct ExtensionBundle {
    pub pkg_id: String,
    pub class_name: String,
    pub name: String,
    pub lang: String,
    pub ext_type: String,
    pub version: String,
    pub version_code: i32,
    pub files: HashMap<String, Vec<u8>>,
}

pub struct ExtensionGenerator;

impl ExtensionGenerator {
    pub fn generate(analysis: &AnalysisResult, custom_name: Option<&str>, target_lang: Option<&str>) -> ExtensionBundle {
        let raw_name = custom_name.unwrap_or(&analysis.name);
        let safe_name = sanitize_class_name(raw_name);
        let pkg_slug = safe_name.to_lowercase();
        let lang = target_lang
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or(if !analysis.lang.is_empty() { &analysis.lang } else { "es" });
        let class_name = safe_name.clone();
        let pkg_id = format!("eu.kanade.tachiyomi.animeextension.{}.{}", lang, pkg_slug);

        let mut files = HashMap::new();

        // 1. Archivo Kotlin Principal
        let kt_path = format!("src/eu/kanade/tachiyomi/animeextension/{}/{}/{}.kt", lang, pkg_slug, class_name);
        let kt_content = generate_kotlin_source(analysis, &safe_name, &pkg_slug, lang);
        files.insert(kt_path, kt_content.into_bytes());

        // 2. Archivo Kotlin de Filtros
        let filters_path = format!("src/eu/kanade/tachiyomi/animeextension/{}/{}/{}Filters.kt", lang, pkg_slug, class_name);
        let filters_content = generate_kotlin_filters(&safe_name, &pkg_slug, lang);
        files.insert(filters_path, filters_content.into_bytes());

        // 3. AndroidManifest.xml
        let manifest_content = generate_android_manifest(&pkg_id, &class_name, &safe_name);
        files.insert("AndroidManifest.xml".to_string(), manifest_content.into_bytes());

        // 4. build.gradle.kts
        let gradle_content = generate_build_gradle(&pkg_id, 1, "16.1");
        files.insert("build.gradle.kts".to_string(), gradle_content.into_bytes());

        // 5. GitHub Actions workflow
        let workflow_content = generate_github_workflow();
        files.insert(".github/workflows/build_push.yml".to_string(), workflow_content.into_bytes());

        // 6. Generador de Index del Repo (generate-repo-index.py)
        let index_script = generate_repo_index_script();
        files.insert(".scripts/generate-repo-index.py".to_string(), index_script.into_bytes());

        ExtensionBundle {
            pkg_id,
            class_name,
            name: safe_name,
            lang: lang.to_string(),
            ext_type: analysis.ext_type.clone(),
            version: "16.1".to_string(),
            version_code: 1,
            files,
        }
    }
}

struct I18nData {
    pref_audio_title: &'static str,
    pref_audio_entries: &'static [&'static str],
    pref_quality_title: &'static str,
    pref_server_blacklist_title: &'static str,
    pref_server_blacklist_summary: &'static str,
    pref_genre_blacklist_title: &'static str,
    pref_genre_blacklist_summary: &'static str,
    filter_header: &'static str,
    filter_audio_title: &'static str,
    filter_audio_entries: &'static [&'static str],
    filter_genre_title: &'static str,
    filter_genre_blacklist_title: &'static str,
    genres: &'static [(&'static str, &'static str)],
}

fn get_i18n(lang: &str) -> I18nData {
    match lang.to_lowercase().as_str() {
        "en" => I18nData {
            pref_audio_title: "Preferred Audio (SUB / DUB)",
            pref_audio_entries: &["Show All", "Prioritize Subbed", "Prioritize Dubbed (DUB)"],
            pref_quality_title: "Preferred Quality",
            pref_server_blacklist_title: "Server Blacklist (Hide)",
            pref_server_blacklist_summary: "Disable servers in player (e.g. Zilla)",
            pref_genre_blacklist_title: "Genre Blacklist (# - Hide)",
            pref_genre_blacklist_summary: "Permanently hide anime with these genres or tags (#) in catalog",
            filter_header: "Search & Catalog Filters",
            filter_audio_title: "Audio / Dubbing",
            filter_audio_entries: &["All", "Subtitled (SUB)", "Dubbed (DUB)"],
            filter_genre_title: "Genre (#)",
            filter_genre_blacklist_title: "Genre Blacklist (# - Exclude)",
            genres: &[
                ("Action", "action"),
                ("Adventure", "adventure"),
                ("Sci-Fi", "sci-fi"),
                ("Comedy", "comedy"),
                ("Sports", "sports"),
                ("Drama", "drama"),
                ("Ecchi", "ecchi"),
                ("Fantasy", "fantasy"),
                ("Gore", "gore"),
                ("Harem", "harem"),
                ("Isekai", "isekai"),
                ("Mecha", "mecha"),
                ("Romance", "romance"),
                ("Shounen", "shounen"),
                ("Supernatural", "supernatural"),
                ("Horror", "horror"),
            ],
        },
        "pt-br" | "pt" => I18nData {
            pref_audio_title: "Áudio Preferido (LEG / DUB)",
            pref_audio_entries: &["Mostrar Todos", "Priorizar Legendado", "Priorizar Dublado (DUB)"],
            pref_quality_title: "Qualidade Preferida",
            pref_server_blacklist_title: "Lista Negra de Servidores (Ocultar)",
            pref_server_blacklist_summary: "Desative servidores no reprodutor (ex. Zilla)",
            pref_genre_blacklist_title: "Lista Negra de Gêneros (# - Ocultar)",
            pref_genre_blacklist_summary: "Oculta permanentemente animes com estes gêneros (#) no catálogo",
            filter_header: "Filtros de busca e catálogo",
            filter_audio_title: "Áudio / Dublagem",
            filter_audio_entries: &["Todos", "Legendado (LEG)", "Dublado (DUB)"],
            filter_genre_title: "Gênero (#)",
            filter_genre_blacklist_title: "Lista Negra de Gêneros (# - Excluir)",
            genres: &[
                ("Ação", "acao"),
                ("Aventura", "aventura"),
                ("Ficção Científica", "ficcao-cientifica"),
                ("Comédia", "comedia"),
                ("Esportes", "esportes"),
                ("Drama", "drama"),
                ("Ecchi", "ecchi"),
                ("Fantasia", "fantasia"),
                ("Gore", "gore"),
                ("Harém", "harem"),
                ("Isekai", "isekai"),
                ("Mecha", "mecha"),
                ("Romance", "romance"),
                ("Shounen", "shounen"),
                ("Sobrenatural", "sobrenatural"),
                ("Terror", "terror"),
            ],
        },
        _ => I18nData {
            pref_audio_title: "Audio Preferido (SUB / DUB)",
            pref_audio_entries: &["Mostrar Todos", "Priorizar Subtitulado", "Priorizar Doblaje (DUB)"],
            pref_quality_title: "Calidad Preferida",
            pref_server_blacklist_title: "Lista Negra de Servidores (Ocultar)",
            pref_server_blacklist_summary: "Desactiva servidores en el reproductor (ej. Zilla)",
            pref_genre_blacklist_title: "Lista Negra de Géneros (# - Ocultar)",
            pref_genre_blacklist_summary: "Oculta permanentemente animes con estos géneros (#) en el catálogo",
            filter_header: "Filtros de búsqueda y catálogo",
            filter_audio_title: "Audio / Doblaje",
            filter_audio_entries: &["Todos", "Subtitulado (SUB)", "Doblado (DUB)"],
            filter_genre_title: "Género (#)",
            filter_genre_blacklist_title: "Lista Negra de Géneros (# - Excluir)",
            genres: &[
                ("Acción", "accion"),
                ("Aventura", "aventura"),
                ("Ciencia Ficción", "ciencia-ficcion"),
                ("Comedia", "comedia"),
                ("Deportes", "deportes"),
                ("Drama", "drama"),
                ("Ecchi", "ecchi"),
                ("Fantasía", "fantasia"),
                ("Gore", "gore"),
                ("Harem", "harem"),
                ("Isekai", "isekai"),
                ("Mecha", "mecha"),
                ("Romance", "romance"),
                ("Sci-Fi", "sci-fi"),
                ("Shounen", "shounen"),
                ("Sobrenatural", "sobrenatural"),
                ("Terror", "terror"),
            ],
        },
    }
}

fn sanitize_class_name(name: &str) -> String {
    let clean: String = name.chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .collect();
    let parts: Vec<&str> = clean.split_whitespace().collect();
    if parts.is_empty() {
        return "CustomExtension".to_string();
    }
    parts.iter().map(|s| {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str().to_lowercase().as_str(),
        }
    }).collect()
}

fn generate_kotlin_source(analysis: &AnalysisResult, class_name: &str, pkg_slug: &str, lang: &str) -> String {
    let base_url = &analysis.url;
    let name = class_name;
    let card_sel = &analysis.card_selector;
    let title_sel = &analysis.title_selector;
    let next_sel = &analysis.next_page_selector;
    let i18n = get_i18n(lang);

    let pref_audio_title = i18n.pref_audio_title;
    let pref_audio_entries = i18n.pref_audio_entries.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ");
    let pref_server_blacklist_title = i18n.pref_server_blacklist_title;
    let pref_server_blacklist_summary = i18n.pref_server_blacklist_summary;
    let pref_genre_blacklist_title = i18n.pref_genre_blacklist_title;
    let pref_genre_blacklist_summary = i18n.pref_genre_blacklist_summary;

    let genre_entries = i18n.genres.iter().map(|(g, _)| format!("\"{}\"", g)).collect::<Vec<_>>().join(", ");
    let genre_values = i18n.genres.iter().map(|(_, v)| format!("\"{}\"", v)).collect::<Vec<_>>().join(", ");

    format!(r#"package eu.kanade.tachiyomi.animeextension.{lang}.{pkg_slug}

import android.app.Application
import android.content.SharedPreferences
import androidx.preference.ListPreference
import androidx.preference.MultiSelectListPreference
import androidx.preference.PreferenceScreen
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
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import org.jsoup.nodes.Document
import org.jsoup.nodes.Element
import uy.kohesive.injekt.Injekt
import uy.kohesive.injekt.api.get

class {class_name} : ConfigurableAnimeSource, ParsedAnimeHttpSource() {{

    override val name = "{name}"

    override val baseUrl = "{base_url}"

    override val lang = "{lang}"

    override val supportsLatest = true

    override val client: OkHttpClient = network.cloudflareClient

    private val preferences: SharedPreferences by lazy {{
        Injekt.get<Application>().getSharedPreferences("source_$id", Application.MODE_PRIVATE)
    }}

    override fun headersBuilder(): Headers.Builder = Headers.Builder()
        .add("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .add("Referer", "$baseUrl/")

    // ============================== POPULAR ANIME ==============================
    override fun popularAnimeRequest(page: Int): Request = GET("$baseUrl/animes?page=$page", headers)

    override fun popularAnimeSelector(): String = "{card_sel}"

    override fun popularAnimeFromElement(element: Element): SAnime = SAnime.create().apply {{
        val link = element.selectFirst("a")
        setUrlWithoutDomain(link?.attr("href") ?: "")
        title = element.selectFirst("{title_sel}")?.text() ?: link?.attr("title") ?: "Sin Título"
        thumbnail_url = element.selectFirst("img")?.attr("abs:src")
            ?: element.selectFirst("img")?.attr("abs:data-src")
    }}

    override fun popularAnimeNextPageSelector(): String = "{next_sel}"

    override fun popularAnimeParse(response: Response): AnimesPage =
        filterBlacklistedAnimes(super.popularAnimeParse(response))

    // ============================== LATEST UPDATES ==============================
    override fun latestUpdatesRequest(page: Int): Request = GET("$baseUrl/recientes?page=$page", headers)

    override fun latestUpdatesSelector(): String = "{card_sel}"

    override fun latestUpdatesFromElement(element: Element): SAnime = popularAnimeFromElement(element)

    override fun latestUpdatesNextPageSelector(): String = "{next_sel}"

    override fun latestUpdatesParse(response: Response): AnimesPage =
        filterBlacklistedAnimes(super.latestUpdatesParse(response))

    // ============================== SEARCH ANIME ==============================
    override fun searchAnimeRequest(page: Int, query: String, filters: AnimeFilterList): Request {{
        val cleanQuery = query.trim()
        val audioFilter = filters.filterIsInstance<{class_name}Filters.AudioFilter>().firstOrNull()
        val isDubSelected = audioFilter?.let {{ it.vals[it.state].second == "dub" }} ?: false

        val finalQuery = when {{
            isDubSelected && cleanQuery.isNotBlank() -> "$cleanQuery latino"
            isDubSelected -> "latino"
            else -> cleanQuery
        }}

        val url = if (finalQuery.isNotBlank()) {{
            "$baseUrl/animes?search=$finalQuery&page=$page"
        }} else {{
            "$baseUrl/animes?page=$page"
        }}
        return GET(url, headers)
    }}

    override fun searchAnimeSelector(): String = "{card_sel}"

    override fun searchAnimeFromElement(element: Element): SAnime = popularAnimeFromElement(element)

    override fun searchAnimeNextPageSelector(): String = "{next_sel}"

    override fun searchAnimeParse(response: Response): AnimesPage =
        filterBlacklistedAnimes(super.searchAnimeParse(response))

    private fun filterBlacklistedAnimes(page: AnimesPage): AnimesPage {{
        val blacklistedGenres = preferences.getStringSet("genre_blacklist", emptySet()) ?: emptySet()
        if (blacklistedGenres.isEmpty()) return page

        val filtered = page.animes.filterNot {{ anime ->
            val titleLower = anime.title.lowercase()
            val genreLower = anime.genre?.lowercase() ?: ""
            blacklistedGenres.any {{ bg ->
                bg.isNotEmpty() && (titleLower.contains(bg) || genreLower.contains(bg))
            }}
        }}
        return AnimesPage(filtered, page.hasNextPage)
    }}

    // ============================== DETAILS ==============================
    override fun animeDetailsParse(document: Document): SAnime = SAnime.create().apply {{
        title = document.selectFirst("h1, .title, .entry-title")?.text() ?: "Anime"
        description = document.selectFirst("p.sinopsis, div.description, .entry-content p")?.text()
        genre = document.select("a[href*=/genero/], a[href*=/genre/], .genres a").joinToString(", ") {{ it.text() }}
        status = SAnime.UNKNOWN
        thumbnail_url = document.selectFirst("img.poster, .poster img")?.attr("abs:src")
    }}

    // ============================== EPISODES ==============================
    override fun episodeListSelector(): String = "li.episode, a[href*=/ver/], div.episodios a"

    override fun episodeFromElement(element: Element): SEpisode = SEpisode.create().apply {{
        val link = if (element.tagName() == "a") element else element.selectFirst("a")
        setUrlWithoutDomain(link?.attr("href") ?: "")
        name = element.selectFirst(".title, span")?.text() ?: link?.text() ?: "Episodio"
        val numRegex = Regex("""(?:episodio|capitulo|cap|ep)[\\s-]*(\\d+)""", RegexOption.IGNORE_CASE)
        val match = numRegex.find(name)
        episode_number = match?.groupValues?.get(1)?.toFloatOrNull() ?: 1f
    }}

    // ============================== VIDEO LIST & EXTRACTORS ==============================
    override fun videoListParse(response: Response): List<Video> {{
        val document = response.asJsoup()
        val videoList = mutableListOf<Video>()

        // Lista Negra de servidores configurada por el usuario
        val blacklist = preferences.getStringSet("server_blacklist", emptySet()) ?: emptySet()

        // Extracción de servidores desde iframes
        val iframeElements = document.select("iframe[src]")
        for (iframe in iframeElements) {{
            val src = iframe.attr("abs:src")
            when {{
                src.contains("mp4upload.com") -> {{
                    if (!blacklist.contains("mp4upload")) {{
                        videoList.add(Video(src, "[SUB] MP4Upload", src, headers))
                    }}
                }}
                src.contains("voe.sx") -> {{
                    if (!blacklist.contains("voe")) {{
                        videoList.add(Video(src, "[SUB] Voe", src, headers))
                    }}
                }}
                src.contains("player.zilla-networks.com") -> {{
                    if (!blacklist.contains("zilla")) {{
                        val zHeaders = Headers.Builder()
                            .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
                            .set("Referer", src)
                            .set("Origin", "https://player.zilla-networks.com")
                            .set("Sec-Fetch-Mode", "cors")
                            .set("Sec-Fetch-Site", "same-origin")
                            .set("Sec-Fetch-Dest", "empty")
                            .set("Accept", "*/*")
                            .build()
                        videoList.add(Video(src, "[SUB] Zilla HLS", src, zHeaders))
                    }}
                }}
                else -> {{
                    if (!blacklist.contains("direct")) {{
                        videoList.add(Video(src, "Server Direct", src, headers))
                    }}
                }}
            }}
        }}

        return sortVideos(videoList)
    }}

    override fun videoFromElement(element: Element): Video = throw UnsupportedOperationException()
    override fun videoListSelector(): String = throw UnsupportedOperationException()
    override fun videoUrlParse(document: Document): String = throw UnsupportedOperationException()

    // ============================== PREFERENCES & SORTING ==============================
    override fun setupPreferenceScreen(screen: PreferenceScreen) {{
        val audioPref = ListPreference(screen.context).apply {{
            key = "preferred_audio"
            title = "{pref_audio_title}"
            entries = arrayOf({pref_audio_entries})
            entryValues = arrayOf("all", "sub", "dub")
            setDefaultValue("all")
            summary = "%s"
        }}
        screen.addPreference(audioPref)

        val serverBlacklistPref = MultiSelectListPreference(screen.context).apply {{
            key = "server_blacklist"
            title = "{pref_server_blacklist_title}"
            entries = arrayOf("MP4Upload", "Voe", "Zilla (HLS)", "Directos")
            entryValues = arrayOf("mp4upload", "voe", "zilla", "direct")
            setDefaultValue(emptySet<String>())
            summary = "{pref_server_blacklist_summary}"
        }}
        screen.addPreference(serverBlacklistPref)

        val genreBlacklistPref = MultiSelectListPreference(screen.context).apply {{
            key = "genre_blacklist"
            title = "{pref_genre_blacklist_title}"
            entries = arrayOf({genre_entries})
            entryValues = arrayOf({genre_values})
            setDefaultValue(emptySet<String>())
            summary = "{pref_genre_blacklist_summary}"
        }}
        screen.addPreference(genreBlacklistPref)
    }}

    private fun sortVideos(videos: List<Video>): List<Video> {{
        val prefAudio = preferences.getString("preferred_audio", "all")
        return videos.sortedWith(
            compareByDescending<Video> {{
                when (prefAudio) {{
                    "dub" -> it.quality.contains("[DUB]")
                    "sub" -> it.quality.contains("[SUB]")
                    else -> true
                }}
            }}
        )
    }}
}}
"#)
}

fn generate_kotlin_filters(class_name: &str, pkg_slug: &str, lang: &str) -> String {
    let i18n = get_i18n(lang);
    let filter_header = i18n.filter_header;
    let filter_audio_title = i18n.filter_audio_title;
    let filter_genre_title = i18n.filter_genre_title;
    let filter_genre_blacklist_title = i18n.filter_genre_blacklist_title;
    let audio_entries = i18n.filter_audio_entries.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ");

    let genres_list = i18n.genres.iter()
        .map(|(name, id)| format!("        Genre(\"{}\", \"{}\")", name, id))
        .collect::<Vec<_>>()
        .join(",\n");

    format!(r#"package eu.kanade.tachiyomi.animeextension.{lang}.{pkg_slug}

import eu.kanade.tachiyomi.animesource.model.AnimeFilter
import eu.kanade.tachiyomi.animesource.model.AnimeFilterList

object {class_name}Filters {{
    class Header(name: String) : AnimeFilter.Header(name)

    class Genre(val name: String, val id: String)
    class GenreFilter(name: String, val genres: List<Genre>) :
        AnimeFilter.Select<String>(name, genres.map {{ it.name }}.toTypedArray())

    class BlacklistGenreFilter(name: String, val genres: List<Genre>) :
        AnimeFilter.Select<String>(name, genres.map {{ it.name }}.toTypedArray())

    class AudioFilter : AnimeFilter.Select<String>(
        "{filter_audio_title}",
        arrayOf({audio_entries})
    ) {{
        val vals = arrayOf(
            Pair("all", ""),
            Pair("sub", "sub"),
            Pair("dub", "dub")
        )
    }}

    val DEFAULT_GENRES = listOf(
{genres_list}
    )

    fun getFilterList(): AnimeFilterList = AnimeFilterList(
        Header("{filter_header}"),
        AudioFilter(),
        GenreFilter("{filter_genre_title}", DEFAULT_GENRES),
        BlacklistGenreFilter("{filter_genre_blacklist_title}", DEFAULT_GENRES)
    )
}}
"#)
}

fn generate_android_manifest(pkg_id: &str, class_name: &str, app_label: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="{pkg_id}">

    <uses-feature android:name="tachiyomi.animeextension" />

    <application
        android:label="{app_label}"
        android:icon="@mipmap/ic_launcher">
        <meta-data
            android:name="tachiyomi.animeextension.class"
            android:value="{pkg_id}.{class_name}" />
        <meta-data
            android:name="tachiyomi.animeextension.factory"
            android:value="" />
        <meta-data
            android:name="tachiyomi.animeextension.hasSettings"
            android:value="true" />
        <meta-data
            android:name="tachiyomi.animeextension.nsfw"
            android:value="0" />
    </application>
</manifest>
"#)
}

fn generate_build_gradle(pkg_id: &str, version_code: i32, version_name: &str) -> String {
    format!(r#"plugins {{
    id("com.android.application")
    kotlin("android")
    kotlin("plugin.serialization")
}}

android {{
    namespace = "{pkg_id}"
    compileSdk = 34

    defaultConfig {{
        applicationId = "{pkg_id}"
        minSdk = 21
        targetSdk = 34
        versionCode = {version_code}
        versionName = "{version_name}"
    }}

    signingConfigs {{
        create("release") {{
            storeFile = rootProject.file("signingkey.jks")
            storePassword = "nekoyomi_keystore_pass"
            keyAlias = "nekoyomi"
            keyPassword = "nekoyomi_keystore_pass"
        }}
    }}

    buildTypes {{
        getByName("release") {{
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("release")
        }}
    }}

    compileOptions {{
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }}
    kotlinOptions {{
        jvmTarget = "1.8"
    }}
}}

dependencies {{
    implementation(project(":core"))
    implementation("androidx.preference:preference-ktx:1.2.1")
    implementation("org.jsoup:jsoup:1.17.2")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.6.3")
}}
"#)
}

fn generate_github_workflow() -> String {
    r#"name: Build and Publish Nekoyomi Extension

on:
  push:
    branches: [main]
  workflow_dispatch:

jobs:
  build:
    name: Build APK and Update Repo
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code
        uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Set up JDK 17
        uses: actions/setup-java@v4
        with:
          java-version: '17'
          distribution: 'temurin'

      - name: Setup Android SDK
        uses: android-actions/setup-android@v3

      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.10'

      - name: Build Extension APK
        run: |
          chmod +x gradlew || true
          ./gradlew assembleRelease --stacktrace || true

      - name: Generate Repo Index
        run: |
          python3 .scripts/generate-repo-index.py

      - name: Deploy to GitHub Pages
        uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./repo_dist
          publish_branch: gh-pages
          force_orphan: true
"#.to_string()
}

fn generate_repo_index_script() -> String {
    r#"import os
import json
import glob
import hashlib
import zipfile

def main():
    os.makedirs("repo_dist", exist_ok=True)
    # Busca apks generados
    apks = glob.glob("**/build/outputs/apk/release/*.apk", recursive=True)
    print(f"Encontrados {len(apks)} APKs")
    
    entries = []
    for apk in apks:
        filename = os.path.basename(apk)
        dest = os.path.join("repo_dist", filename)
        with open(apk, "rb") as f_in, open(dest, "wb") as f_out:
            f_out.write(f_in.read())
            
        with open(dest, "rb") as f:
            h = hashlib.sha256(f.read()).hexdigest()
            
        entries.append({
            "name": filename.replace(".apk", ""),
            "pkg": "eu.kanade.tachiyomi.animeextension.es",
            "apk": filename,
            "lang": "es",
            "code": 1,
            "version": "16.1",
            "sha256": h
        })

    with open("repo_dist/index.min.json", "w", encoding="utf-8") as f:
        json.dump(entries, f, indent=2)

    repo_meta = {
        "meta": {
            "name": "AEXT Extensions Repository",
            "website": "https://github.com",
            "signingKeyFingerprint": "5170d5885c347f7d1f56ff4fc8ee4bb85e1975e580e5576067fa20320a2e7c41"
        }
    }
    with open("repo_dist/repo.json", "w", encoding="utf-8") as f:
        json.dump(repo_meta, f, indent=2)

if __name__ == "__main__":
    main()
"#.to_string()
}
