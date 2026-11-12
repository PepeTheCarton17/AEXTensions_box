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

        // 7. Keystore permanente determinista (signingkey.jks) para evitar "Firma diferente"
        let keystore_bytes = decode_base64(KEYSTORE_B64);
        files.insert("signingkey.jks".to_string(), keystore_bytes);

        // 8. README.md oficial con Disclaimer legal y enlaces de 1 clic para Nekoyomi
        let readme_content = generate_repo_readme(&safe_name, lang, &analysis.url);
        files.insert("README.md".to_string(), readme_content.into_bytes());

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

#[allow(dead_code)]
struct I18nData {
    pref_audio_title: &'static str,
    pref_audio_entries: &'static [&'static str],
    #[allow(dead_code)]
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

    format!(r#"package eu.kanade.tachiyomi.animeextension.{lang}.{pkg_slug}

import android.app.Application
import android.content.SharedPreferences
import androidx.preference.EditTextPreference
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
        val rawBlacklist = preferences.getString("genre_blacklist_text", "") ?: ""
        val blacklistWords = rawBlacklist
            .split(",")
            .map {{ it.trim().lowercase() }}
            .filter {{ it.isNotEmpty() }}

        if (blacklistWords.isEmpty()) return page

        val filtered = page.animes.filterNot {{ anime ->
            val genreLower = anime.genre?.lowercase() ?: ""
            val titleLower = anime.title.lowercase()
            blacklistWords.any {{ word ->
                (genreLower.isNotEmpty() && genreLower.contains(word)) || titleLower.contains(word)
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

        val genreBlacklistPref = EditTextPreference(screen.context).apply {{
            key = "genre_blacklist_text"
            title = "{pref_genre_blacklist_title}"
            setDefaultValue("")
            summary = "{pref_genre_blacklist_summary}"
            dialogTitle = "{pref_genre_blacklist_title}"
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
        GenreFilter("{filter_genre_title}", DEFAULT_GENRES)
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
            storePassword = "unboxpassword"
            keyAlias = "unbox"
            keyPassword = "unboxpassword"
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
        uses: actions/setup-java@v5
        with:
          java-version: '17'
          distribution: 'temurin'

      - name: Setup Android SDK
        uses: android-actions/setup-android@v3

      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.11'

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
from pathlib import Path

DEFAULT_FINGERPRINT = "2f4cdf7ed264f6e6aa0d7655b7d6222df6d9f0ffde924f0eea4c57be2dc8c75c"

def extract_signing_fingerprint_from_apk(apk_path: str) -> str:
    try:
        with zipfile.ZipFile(apk_path) as z:
            for item in z.namelist():
                if item.startswith("META-INF/") and (item.endswith(".RSA") or item.endswith(".DSA")):
                    data = z.read(item)
                    idx = data.find(b'\xa0')
                    if idx != -1:
                        cert_start = data.find(b'\x30\x82', idx)
                        if cert_start != -1:
                            length = int.from_bytes(data[cert_start+2:cert_start+4], 'big') + 4
                            cert_der = data[cert_start:cert_start+length]
                            return hashlib.sha256(cert_der).hexdigest()
    except Exception as e:
        print(f"[!] Extracción de huella omitida: {e}")
    return DEFAULT_FINGERPRINT

def main():
    os.makedirs("repo_dist", exist_ok=True)
    apks = glob.glob("**/build/outputs/apk/release/*.apk", recursive=True)
    print(f"Encontrados {len(apks)} APKs")
    
    entries = []
    fingerprint = DEFAULT_FINGERPRINT

    for apk in apks:
        filename = os.path.basename(apk)
        dest = os.path.join("repo_dist", filename)
        with open(apk, "rb") as f_in, open(dest, "wb") as f_out:
            f_out.write(f_in.read())
            
        with open(dest, "rb") as f:
            h = hashlib.sha256(f.read()).hexdigest()

        extracted_fp = extract_signing_fingerprint_from_apk(dest)
        if extracted_fp and extracted_fp != DEFAULT_FINGERPRINT:
            fingerprint = extracted_fp
            
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
            "signingKeyFingerprint": fingerprint
        }
    }
    with open("repo_dist/repo.json", "w", encoding="utf-8") as f:
        json.dump(repo_meta, f, indent=2)

if __name__ == "__main__":
    main()
"#.to_string()
}

fn generate_repo_readme(name: &str, lang: &str, base_url: &str) -> String {
    format!(r#"# {name} - Nekoyomi Extension Repository

Repositorio de extensiones para **Nekoyomi** y **Aniyomi** generado automáticamente con [AEXT](https://github.com/PepeTheCarton17/AEXTensions_box).

---

## Instalación en 1 Clic

| Aplicación | Enlace Directo (Deep Link) |
| :--- | :--- |
| **Nekoyomi** | [Añadir Repositorio a Nekoyomi](nekoyomi://add-repo?url=https%3A%2F%2Fraw.githubusercontent.com%2FPepeTheCarton17%2F{{REPO_NAME}}%2Fgh-pages%2Findex.min.json) |
| **Aniyomi** | [Añadir Repositorio a Aniyomi](aniyomi://add-repo?url=https%3A%2F%2Fraw.githubusercontent.com%2FPepeTheCarton17%2F{{REPO_NAME}}%2Fgh-pages%2Findex.min.json) |

### Instalación Manual:
1. Abre **Nekoyomi** o **Aniyomi**.
2. Ve a **Más** → **Ajustes** → **Explorar** → **Repositorios de extensiones**.
3. Presiona el botón **+** y agrega la URL de `index.min.json` de la rama `gh-pages`.

---

## Información de la Extensión
- **Nombre:** {name}
- **Idioma:** {lang}
- **Fuente original:** {base_url}
- **Firma:** Determinista e inmutable (`signingkey.jks`) para permitir actualizaciones continuas sin error de firma.

---

## Aviso Legal / Disclaimer

> **IMPORTANTE**: Este repositorio y el software provisto son creados **exclusivamente con fines educativos y de investigación tecnológica**.
>
> 1. **Sin Alojamiento de Medios**: Este repositorio, sus autores y las extensiones distribuidas **NO alojan, transmiten, almacenan, redistribuyen ni poseen** ningún archivo multimedia, stream de video, audio o contenido protegido por derechos de autor.
> 2. **Servidores de Terceros**: Las extensiones funcionan como scrapers y analizadores web que indexan enlaces públicos provistos directamente por sitios web de terceros independientes y no afiliados.
> 3. **Propiedad Intelectual**: Todas las marcas comerciales, nombres de series, imágenes de portada y logotipos pertenecen a sus respectivos titulares y propietarios de derechos de autor.
> 4. **Responsabilidad**: Los autores no se responsabilizan por el uso indebido que terceros puedan dar a estas herramientas, siendo el usuario el único responsable de cumplir la legislación vigente en su país.
"#)
}

fn decode_base64(input: &str) -> Vec<u8> {
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for c in input.chars() {
        let val = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => continue,
        };
        buffer = (buffer << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(((buffer >> bits) & 0xFF) as u8);
        }
    }
    output
}

const KEYSTORE_B64: &str = "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";
