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
    /// Ruta del módulo Gradle dentro del repo, p. ej. `src/es/animeav1`.
    pub module_path: String,
    pub files: HashMap<String, Vec<u8>>,
}

pub struct ExtensionGenerator;

impl ExtensionGenerator {
    pub fn generate(
        analysis: &AnalysisResult,
        custom_name: Option<&str>,
        target_lang: Option<&str>,
        custom_ext_type: Option<&str>,
    ) -> ExtensionBundle {
        Self::generate_with_version(analysis, custom_name, target_lang, custom_ext_type, 1, "16.1")
    }

    pub fn generate_with_version(
        analysis: &AnalysisResult,
        custom_name: Option<&str>,
        target_lang: Option<&str>,
        custom_ext_type: Option<&str>,
        version_code: i32,
        version_name: &str,
    ) -> ExtensionBundle {
        let raw_name = custom_name
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or(&analysis.name);
        let class_name = sanitize_class_name(raw_name);
        let pkg_slug = sanitize_pkg_slug(raw_name);
        let lang = target_lang
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or(if !analysis.lang.is_empty() { &analysis.lang } else { "es" });
        let display_name = sanitize_display_name(raw_name);

        // El código de idioma visible puede ser "pt-br", pero un paquete Kotlin
        // no admite guiones: usamos una variante segura para package/paths.
        let pkg_lang_raw: String = lang
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_lowercase();
        let pkg_lang = if pkg_lang_raw.is_empty() {
            "all".to_string()
        } else {
            pkg_lang_raw
        };

        let ext_type = custom_ext_type
            .unwrap_or(&analysis.ext_type)
            .to_lowercase();
        let ext_type = if ext_type == "manga" { "manga".to_string() } else { "anime".to_string() };

        let pkg_prefix = if ext_type == "manga" {
            "eu.kanade.tachiyomi.extension"
        } else {
            "eu.kanade.tachiyomi.animeextension"
        };
        let pkg_id = format!("{}.{}.{}", pkg_prefix, pkg_lang, pkg_slug);
        let source_dir = pkg_prefix.replace('.', "/");
        // Estructura multi-módulo habitual en los repos de extensiones de
        // Aniyomi/Nekoyomi: cada extensión es un módulo Gradle propio.
        let module_path = format!("src/{}/{}", pkg_lang, pkg_slug);

        let mut files = HashMap::new();

        // 1. Archivo Kotlin Principal
        let kt_path = format!(
            "{}/src/main/java/{}/{}/{}.kt",
            module_path, source_dir, pkg_lang, class_name
        );
        let (kt_content, filters_content) = if ext_type == "manga" {
            (
                generate_manga_source(
                    analysis,
                    &display_name,
                    &class_name,
                    pkg_slug.as_str(),
                    lang,
                    &pkg_lang,
                    pkg_prefix,
                ),
                generate_manga_filters(&class_name, &pkg_slug, &pkg_lang, pkg_prefix),
            )
        } else {
            (
                generate_kotlin_source(
                    analysis,
                    &display_name,
                    &class_name,
                    &pkg_slug,
                    lang,
                    &pkg_lang,
                    pkg_prefix,
                ),
                generate_kotlin_filters(&class_name, &pkg_slug, lang, &pkg_lang, pkg_prefix),
            )
        };
        files.insert(kt_path, kt_content.into_bytes());

        // 2. Archivo Kotlin de Filtros
        let filters_path = format!(
            "{}/src/main/java/{}/{}/{}Filters.kt",
            module_path, source_dir, pkg_lang, class_name
        );
        files.insert(filters_path, filters_content.into_bytes());

        // 3. AndroidManifest.xml (dentro del módulo)
        let manifest_content = generate_android_manifest(&pkg_id, &class_name, &display_name, &ext_type);
        files.insert(
            format!("{}/src/main/AndroidManifest.xml", module_path),
            manifest_content.into_bytes(),
        );

        // 4. build.gradle.kts del módulo
        let gradle_content = generate_build_gradle(
            &pkg_id,
            version_code,
            version_name,
            &pkg_lang,
            &pkg_slug,
            &ext_type,
        );
        files.insert(
            format!("{}/build.gradle.kts", module_path),
            gradle_content.into_bytes(),
        );

        // 5. Archivos raíz del proyecto Gradle (sin esto, Actions no compila).
        files.insert(
            "build.gradle.kts".to_string(),
            ROOT_BUILD_GRADLE.as_bytes().to_vec(),
        );
        files.insert(
            "settings.gradle.kts".to_string(),
            generate_settings_gradle(&pkg_lang, &pkg_slug).into_bytes(),
        );
        files.insert(
            "gradle.properties".to_string(),
            GRADLE_PROPERTIES.as_bytes().to_vec(),
        );
        files.insert(
            ".gitignore".to_string(),
            GITIGNORE.as_bytes().to_vec(),
        );

        // 6. GitHub Actions workflow
        let workflow_content = generate_github_workflow();
        files.insert(".github/workflows/build_push.yml".to_string(), workflow_content.into_bytes());

        // 7. Generador de Index del Repo (generate-repo-index.py)
        let index_script = generate_repo_index_script();
        files.insert(".github/scripts/generate-repo-index.py".to_string(), index_script.into_bytes());

        // 8. Keystore permanente determinista (signingkey.jks) para evitar "Firma diferente"
        //    Es una clave propia de AEXT, autofirmada y pública por diseño: vive en el
        //    repositorio generado para que las actualizaciones conserven la firma.
        let keystore_bytes = decode_base64(KEYSTORE_B64);
        files.insert("signingkey.jks".to_string(), keystore_bytes);

        // 9. README.md oficial con Disclaimer legal y enlaces de 1 clic para Nekoyomi
        let readme_content = generate_repo_readme(&display_name, lang, &analysis.url, version_name);
        files.insert("README.md".to_string(), readme_content.into_bytes());

        ExtensionBundle {
            pkg_id,
            class_name,
            name: display_name,
            lang: lang.to_string(),
            ext_type: ext_type.clone(),
            version: version_name.to_string(),
            version_code,
            module_path,
            files,
        }
    }
}

#[allow(dead_code)]
struct I18nData {
    pref_audio_title: &'static str,
    pref_audio_entries: &'static [&'static str],
    pref_server_title: &'static str,
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
            pref_server_title: "Preferred Server (Default stream)",
            pref_quality_title: "Preferred Quality (Default stream)",
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
            pref_server_title: "Servidor Preferido (Stream Padrão)",
            pref_quality_title: "Qualidade Preferida (Stream Padrão)",
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
            pref_server_title: "Servidor Preferido (Stream por Defecto)",
            pref_quality_title: "Calidad Preferida (Stream por Defecto)",
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

fn transliterate_char(c: char) -> Option<char> {
    match c {
        'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' | 'Á' | 'À' | 'Ä' | 'Â' | 'Ã' | 'Å' => Some('a'),
        'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => Some('e'),
        'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => Some('i'),
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'Ó' | 'Ò' | 'Ö' | 'Ô' | 'Õ' => Some('o'),
        'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => Some('u'),
        'ñ' | 'Ñ' => Some('n'),
        'ç' | 'Ç' => Some('c'),
        'ý' | 'ÿ' | 'Ý' | 'Ÿ' => Some('y'),
        'æ' | 'Æ' => Some('a'),
        'ø' | 'Ø' => Some('o'),
        'ß' => Some('s'),
        _ => None,
    }
}

fn normalize_words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();

    for ch in name.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            Some(ch)
        } else if ch.is_alphabetic() {
            transliterate_char(ch)
        } else {
            None
        };

        match mapped {
            Some(c) if c.is_ascii_alphanumeric() => current.push(c),
            _ => {
                if !current.is_empty() {
                    words.push(std::mem::take(&mut current));
                }
            }
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn sanitize_class_name(name: &str) -> String {
    let words = normalize_words(name);
    if words.is_empty() {
        return "CustomExtension".to_string();
    }

    let mut out = String::new();
    for word in words {
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(&chars.as_str().to_ascii_lowercase());
        }
    }

    if out.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        out.insert_str(0, "Ext");
    }
    out
}

fn sanitize_pkg_slug(name: &str) -> String {
    let words = normalize_words(name);
    let slug = words
        .iter()
        .map(|w| w.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("");

    if slug.is_empty() {
        "customextension".to_string()
    } else if slug.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        format!("ext{}", slug)
    } else {
        slug
    }
}

fn sanitize_display_name(name: &str) -> String {
    let words = normalize_words(name);
    if words.is_empty() {
        return "Custom Extension".to_string();
    }
    words
        .iter()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => {
                    let mut s = String::new();
                    s.push(first.to_ascii_uppercase());
                    s.push_str(&chars.as_str().to_ascii_lowercase());
                    s
                }
                None => String::new(),
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Motor de vídeo que se inyecta en cada extensión de anime generada.
///
/// Sustituye al antiguo `Video(src_iframe, ...)`: ahora resuelve el embed real
/// de cada servidor, detecta todas las calidades disponibles y adjunta las
/// cabeceras que Cloudflare/Zilla exigen (evita el 403 y el salto al final).
const VIDEO_ENGINE_KT: &str = r##"    // ============================== VIDEO LIST & EXTRACTORS ==============================

    private val userAgent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36"

    // `(?![a-zA-Z])` evita falsos positivos como `300px`/`250px` del CSS.
    private val resolutionRegex = Regex("""(\d{3,4})p(?![a-zA-Z])""", RegexOption.IGNORE_CASE)

    private fun normalizeQuality(raw: String?): String? {
        val height = raw?.toIntOrNull() ?: return null
        return if (height in 144..4320) height.toString() + "p" else null
    }

    /**
     * Mapa url -> audio ("SUB"/"DUB") leyendo los bloques `SUB:[{server:...,url:...}]`
     * del payload. Sin esto, un iframe del reproductor por defecto puede etiquetarse
     * como DUB solo porque la pestaña DUB del DOM aparece mas cerca en el HTML.
     */
    private fun audioBlocks(html: String): Map<String, String> {
        val map = mutableMapOf<String, String>()
        val pairRegex = Regex("""server\s*:\\?\s*["']([^"']+)["']\s*,\s*url\s*:\s*\\?\s*["'](https?://[^"']+)["']""")
        val labels = listOf("SUB" to "SUB", "DUB" to "DUB", "LAT" to "DUB", "CAST" to "DUB")
        for ((label, audio) in labels) {
            val blockRegex = Regex("""$label\s*:\s*\[([^\]]*)]""")
            for (block in blockRegex.findAll(html)) {
                for (match in pairRegex.findAll(block.groupValues[1])) {
                    map[match.groupValues[2].substringBefore("?")] = audio
                }
            }
        }
        return map
    }

    override fun videoListParse(response: Response): List<Video> {
        val html = response.body?.string().orEmpty()
        val document = response.asJsoup()
        val videos = mutableListOf<Video>()
        val blacklist = preferences.getStringSet("server_blacklist", emptySet()) ?: emptySet()

        val embeds = linkedSetOf<Pair<String, String>>()

        document.select("iframe[src], iframe[data-src]").forEach { el ->
            val src = el.attr("abs:src").ifBlank { el.attr("abs:data-src") }
            if (src.startsWith("http")) embeds.add(el.attr("data-server").ifBlank { "" } to src)
        }

        document.select("video[src], video source[src]").forEach { el ->
            val src = el.attr("abs:src")
            if (src.startsWith("http")) embeds.add("" to src)
        }

        val dataAttrs = listOf("data-video", "data-url", "data-embed", "data-src")
        document.select("[data-video], [data-url], [data-embed]").forEach { el ->
            val src = dataAttrs.firstNotNullOfOrNull { name ->
                el.attr(name).takeIf { it.isNotBlank() }
            } ?: return@forEach
            val absolute = absolutize(baseUrl, src)
            if (absolute.startsWith("http")) {
                embeds.add(el.attr("data-server").ifBlank { el.text().trim() } to absolute)
            }
        }

        val serverPatterns = listOf(
            Regex("""server\s*:\s*["']([^"']+)["']\s*,\s*url\s*:\s*["'](https?://[^"']+)["']"""),
            Regex("""["']server["']\s*:\s*["']([^"']+)["']\s*,\s*["']url["']\s*:\s*["'](https?://[^"']+)["']"""),
            Regex("""(?:server|host|name)\s*:\s*["']([^"']+)["'][^{}]{0,160}?(?:url|file|embed|link)\s*:\s*["'](https?://[^"']+)["']"""),
            Regex("""["'](?:server|host|name)["']\s*:\s*["']([^"']+)["'][^{}]{0,160}?["'](?:url|file|embed|link)["']\s*:\s*["'](https?://[^"']+)["']""")
        )
        for (pattern in serverPatterns) {
            for (match in pattern.findAll(html)) {
                embeds.add(match.groupValues[1] to match.groupValues[2])
            }
        }

        // Playlists HLS embebidas directamente en el HTML (payload tipo SvelteKit / JSON).
        val rawPlaylists = Regex("""(https?://[^"'\s\\]+\.m3u8[^"'\s\\]*)""")
        for (match in rawPlaylists.findAll(html)) {
            embeds.add("" to match.groupValues[1].replace("\\/", "/"))
        }

        val seen = mutableSetOf<String>()
        val audioMap = audioBlocks(html)
        for ((declared, embedUrl) in embeds) {
            val key = embedUrl.substringBefore("?").lowercase()
            if (!seen.add(key)) continue

            val server = declared.ifBlank { serverFromUrl(embedUrl) }
            if (isBlacklisted(blacklist, server, embedUrl)) continue

            val audio = audioMap[embedUrl.substringBefore("?")] ?: detectAudio(html, embedUrl)
            runCatching {
                videos.addAll(resolveVideos(server, embedUrl, audio, headers))
            }
        }

        return sortVideos(videos)
    }

    private fun serverFromUrl(url: String): String {
        val host = url.substringAfter("://", url).substringBefore("/").lowercase()
        return when {
            host.contains("mp4upload") -> "MP4Upload"
            host.contains("voe") -> "Voe"
            host.contains("streamwish") || host.contains("byse") || host.contains("swhoi") -> "Byse"
            host.contains("zilla") -> "Zilla"
            host.contains("mega") -> "Mega"
            host.contains("upnshare") -> "UPNShare"
            host.contains("filemoon") -> "Filemoon"
            host.contains("dood") -> "Dood"
            else -> "Direct"
        }
    }

    private fun isBlacklisted(blacklist: Set<String>, server: String, url: String): Boolean {
        if (blacklist.isEmpty()) return false
        val s = server.lowercase()
        val u = url.lowercase()
        return blacklist.any { raw ->
            val key = raw.lowercase()
            key.isNotEmpty() && (
                s.contains(key) || u.contains(key) ||
                    (key == "zilla" && (s.contains("hls") || u.contains("zilla"))) ||
                    (key == "byse" && (s.contains("streamwish") || u.contains("streamwish"))) ||
                    (key == "direct" && s.contains("direct"))
                )
        }
    }

    private fun detectAudio(html: String, url: String): String {
        val idx = html.indexOf(url)
        if (idx <= 0) return "SUB"
        val before = html.substring(0, idx).uppercase()
        val dubAt = maxOf(
            before.lastIndexOf("DUB"),
            before.lastIndexOf("LAT"),
            before.lastIndexOf("CAST"),
            before.lastIndexOf("AUDIO LATINO")
        )
        val subAt = before.lastIndexOf("SUB")
        return if (dubAt > subAt) "DUB" else "SUB"
    }

    private fun resolveVideos(server: String, url: String, audio: String, baseHeaders: Headers): List<Video> {
        val prefix = "[$audio] "
        val s = server.lowercase()
        val u = url.lowercase()
        return when {
            s.contains("mp4upload") || u.contains("mp4upload") ->
                mp4uploadVideos(url, prefix + "MP4Upload - ", baseHeaders)
            s.contains("voe") || u.contains("voe.sx") ->
                voeVideos(url, prefix + "Voe - ", baseHeaders)
            s.contains("byse") || s.contains("streamwish") || u.contains("streamwish") || u.contains("byse") ->
                streamWishVideos(url, prefix + "Byse - ", baseHeaders)
            s.contains("zilla") || u.contains("zilla-networks") ->
                zillaVideos(url, prefix + "Zilla - ")
            u.contains("mega.nz") || u.contains("mega.co.nz") || s.contains("mega") -> emptyList()
            u.contains(".m3u8") -> hlsVideos(url, prefix, baseHeaders)
            u.contains(".mp4") -> listOf(Video(url, prefix + "MP4", url, baseHeaders))
            else -> genericVideos(url, prefix + server + " - ", baseHeaders)
        }
    }

    private fun embedHeaders(url: String, origin: String, baseHeaders: Headers): Headers =
        baseHeaders.newBuilder()
            .set("Referer", url)
            .set("Origin", origin)
            .set("Accept", "*/*")
            .set("Accept-Language", "es-ES,es;q=0.9,en;q=0.8")
            .set("Sec-Fetch-Dest", "empty")
            .set("Sec-Fetch-Mode", "cors")
            .set("Sec-Fetch-Site", "cross-site")
            .build()

    private fun fetchText(url: String, extra: Headers): String = runCatching {
        val request = Request.Builder().url(url).headers(extra).build()
        client.newCall(request).execute().use { response -> response.body?.string().orEmpty() }
    }.getOrDefault("")

    private fun absolutize(base: String, ref: String): String {
        val clean = ref.trim()
        if (clean.startsWith("http://") || clean.startsWith("https://")) return clean
        if (clean.startsWith("//")) return "https:" + clean
        val origin = originOf(base)
        if (clean.startsWith("/")) return origin + clean
        return base.substringBeforeLast('/', base) + "/" + clean
    }

    private fun originOf(url: String): String {
        val schemeEnd = url.indexOf("://")
        if (schemeEnd < 0) return ""
        val hostEnd = url.indexOf('/', schemeEnd + 3)
        return if (hostEnd < 0) url else url.substring(0, hostEnd)
    }

    private fun isMediaUrl(url: String): Boolean =
        url.contains(".mp4") || url.contains(".m3u8")

    private fun qualityFromUrlOnly(url: String): String =
        normalizeQuality(resolutionRegex.find(url)?.groupValues?.get(1)) ?: "Auto"

    private fun qualityFromBody(url: String, body: String): String {
        normalizeQuality(resolutionRegex.find(url)?.groupValues?.get(1))?.let { return it }
        normalizeQuality(resolutionRegex.find(body)?.groupValues?.get(1))?.let { return it }
        return "Auto"
    }

    private fun distinctVideos(videos: List<Video>): List<Video> {
        val seen = mutableSetOf<String>()
        // `videoUrl` es nullable en extensions-lib: sin `?` esto no compila.
        return videos.filter { seen.add(it.videoUrl?.substringBefore("?") ?: "") }
    }

    private fun mp4uploadVideos(url: String, prefix: String, baseHeaders: Headers): List<Video> {
        val videoHeaders = embedHeaders(url, "https://www.mp4upload.com", baseHeaders)
        val body = fetchText(url, videoHeaders)
        if (body.isBlank()) return emptyList()

        val srcRegex = Regex("""["']?(?:src|file|stream)["']?\s*[:=]\s*["']([^"']+)["']""")
        val labelRegex = Regex("""["']?(?:label|quality|res|resolution|size)["']?\s*[:=]\s*["']?(\d{3,4})p?(?![a-zA-Z])""")
        val objectRegex = Regex("""\{[^{}]*\}""")

        val videos = mutableListOf<Video>()
        for (obj in objectRegex.findAll(body)) {
            val chunk = obj.value
            val raw = srcRegex.find(chunk)?.groupValues?.get(1) ?: continue
            if (!isMediaUrl(raw)) continue
            val absolute = absolutize(url, raw)
            val label = normalizeQuality(labelRegex.find(chunk)?.groupValues?.get(1))
                ?: qualityFromBody(absolute, chunk)
            videos.add(Video(absolute, prefix + label, absolute, videoHeaders))
        }

        if (videos.isEmpty()) {
            val raw = srcRegex.findAll(body).map { it.groupValues[1] }.firstOrNull { isMediaUrl(it) }
            if (raw != null) {
                val absolute = absolutize(url, raw)
                videos.add(Video(absolute, prefix + qualityFromBody(absolute, body), absolute, videoHeaders))
            }
        }

        return distinctVideos(videos)
    }

    private fun isDecoyUrl(url: String): Boolean =
        url.contains("test-videos.co.uk") || url.contains("bigbuckbunny")

    private fun voeVideos(url: String, prefix: String, baseHeaders: Headers): List<Video> {
        var pageUrl = url
        var voeHeaders = embedHeaders(pageUrl, "https://voe.sx", baseHeaders)
        var body = fetchText(pageUrl, voeHeaders)
        if (body.isBlank()) return emptyList()

        // Voe rota de dominio con un redirect por JS: window.location.href = "https://<mirror>/e/<id>".
        if (!body.contains("\"hls\"") && !body.contains(".m3u8")) {
            val redirect = Regex("""window\.location\.href\s*=\s*['"](https?://[^'"]+)['"]""")
                .find(body)?.groupValues?.get(1)
            if (redirect != null && redirect != pageUrl) {
                pageUrl = redirect
                voeHeaders = embedHeaders(pageUrl, "https://voe.sx", baseHeaders)
                body = fetchText(pageUrl, voeHeaders)
            }
        }

        val hls = Regex("""["']hls["']\s*:\s*["']([^"']+)["']""").find(body)?.groupValues?.get(1)
            ?: Regex("""["'](?:file|source|stream|video_url|direct_access_url)["']\s*:\s*["']([^"']+m3u8[^"']*)["']""").find(body)?.groupValues?.get(1)
        val mp4 = Regex("""["']mp4["']\s*:\s*["']([^"']+)["']""").find(body)?.groupValues?.get(1)
            ?: Regex("""var\s+source\s*=\s*['"]([^'"]+)['"]""").find(body)?.groupValues?.get(1)

        val videos = mutableListOf<Video>()
        if (hls != null) videos.addAll(hlsVideos(absolutize(pageUrl, hls), prefix, baseHeaders, voeHeaders))
        if (mp4 != null) {
            val absolute = absolutize(pageUrl, mp4)
            if (!isDecoyUrl(absolute)) {
                videos.add(Video(absolute, prefix + qualityFromBody(absolute, body), absolute, voeHeaders))
            }
        }
        return distinctVideos(videos)
    }

    private fun streamWishVideos(url: String, prefix: String, baseHeaders: Headers): List<Video> {
        val hdrs = embedHeaders(url, originOf(url), baseHeaders)
        val body = fetchText(url, hdrs)
        if (body.isBlank()) return emptyList()

        val raw = Regex("""["']?(?:file|source|hls)["']?\s*[:=]\s*["']([^"']+(?:m3u8|mp4)[^"']*)["']""")
            .find(body)?.groupValues?.get(1) ?: return emptyList()
        val absolute = absolutize(url, raw)
        return if (absolute.contains(".m3u8")) {
            hlsVideos(absolute, prefix, baseHeaders, hdrs)
        } else {
            listOf(Video(absolute, prefix + qualityFromBody(absolute, body), absolute, hdrs))
        }
    }

    private fun zillaVideos(url: String, prefix: String): List<Video> {
        val hash = url.substringAfterLast("/play/").substringAfterLast("/m3u8/")
            .substringBefore("?").substringBefore("/").trim()
        if (hash.length < 16) return emptyList()
        val playlist = "https://player.zilla-networks.com/m3u8/$hash"

        // Cabeceras exactas que Cloudflare exige en Zilla: sin Origin/Referer
        // correctos devuelve 403 o corta el segmento y el player salta al final.
        val zillaHeaders = Headers.Builder()
            .set("User-Agent", userAgent)
            .set("Referer", "https://player.zilla-networks.com/play/$hash")
            .set("Origin", "https://player.zilla-networks.com")
            .set("Accept", "*/*")
            .set("Accept-Language", "es-ES,es;q=0.9,en;q=0.8")
            .set("Sec-Fetch-Dest", "empty")
            .set("Sec-Fetch-Mode", "cors")
            .set("Sec-Fetch-Site", "same-origin")
            .build()

        // Warm-up: deja que cloudflareClient resuelva un posible reto y guarde la
        // cookie cf_clearance en el mismo jar que usan playlist y segmentos.
        fetchText("https://player.zilla-networks.com/play/$hash", zillaHeaders)

        return hlsVideos(playlist, prefix, zillaHeaders, zillaHeaders)
    }

    private fun genericVideos(url: String, prefix: String, baseHeaders: Headers): List<Video> {
        var pageUrl = url
        var hdrs = embedHeaders(pageUrl, originOf(pageUrl), baseHeaders)
        var body = fetchText(pageUrl, hdrs)

        val mediaRegex = Regex("""["']?(?:file|source|src|hls|playlist)["']?\s*[:=]\s*["']([^"']+(?:\.m3u8|\.mp4)[^"']*)["']""")

        // Hosts que rotan de dominio con un redirect por JS.
        if (!mediaRegex.containsMatchIn(body)) {
            val redirect = Regex("""window\.location\.href\s*=\s*['"](https?://[^'"]+)['"]""")
                .find(body)?.groupValues?.get(1)
            if (redirect != null && redirect != pageUrl) {
                pageUrl = redirect
                hdrs = embedHeaders(pageUrl, originOf(pageUrl), baseHeaders)
                body = fetchText(pageUrl, hdrs)
            }
        }
        if (body.isBlank()) return emptyList()

        val raw = mediaRegex.find(body)?.groupValues?.get(1)
            ?: Regex("""(https?://[^"'\s\\]+\.m3u8[^"'\s\\]*)""").find(body)?.groupValues?.get(1)
            ?: Regex("""(https?://[^"'\s\\]+\.mp4[^"'\s\\]*)""").find(body)?.groupValues?.get(1)
            ?: return emptyList()

        val absolute = absolutize(pageUrl, raw.replace("\\/", "/"))
        return if (absolute.contains(".m3u8")) {
            hlsVideos(absolute, prefix, baseHeaders, hdrs)
        } else {
            listOf(Video(absolute, prefix + qualityFromBody(absolute, body), absolute, hdrs))
        }
    }

    /**
     * Expande un master HLS (`#EXT-X-STREAM-INF`) en una entrada `Video` por
     * variante, con su resolución y bitrate reales. Asi el reproductor muestra
     * TODAS las calidades del m3u8 en vez de una sola entrada opaca.
     */
    private fun hlsVideos(url: String, prefix: String, baseHeaders: Headers, extra: Headers = baseHeaders): List<Video> {
        val body = fetchText(url, extra)
        if (body.isBlank() || !body.contains("#EXTM3U")) {
            return listOf(Video(url, prefix + "HLS", url, extra))
        }
        if (!body.contains("#EXT-X-STREAM-INF")) {
            return listOf(Video(url, prefix + qualityFromBody(url, body), url, extra))
        }

        val videos = mutableListOf<Video>()
        var height: String? = null
        var bandwidth: Long? = null
        for (rawLine in body.lines()) {
            val line = rawLine.trim()
            if (line.startsWith("#EXT-X-STREAM-INF")) {
                height = normalizeQuality(Regex("""RESOLUTION=\d+x(\d{3,4})""").find(line)?.groupValues?.get(1))
                bandwidth = Regex("""BANDWIDTH=(\d+)""").find(line)?.groupValues?.get(1)?.toLongOrNull()
            } else if (line.isNotEmpty() && !line.startsWith("#")) {
                val currentHeight = height ?: continue
                val currentBandwidth = bandwidth
                val variant = absolutize(url, line)
                val bitrate = if (currentBandwidth != null && currentBandwidth > 0) {
                    " (" + (currentBandwidth / 1000) + " kbps)"
                } else {
                    ""
                }
                videos.add(Video(variant, prefix + currentHeight + bitrate, variant, extra))
                height = null
                bandwidth = null
            }
        }

        return videos.ifEmpty { listOf(Video(url, prefix + "HLS", url, extra)) }
    }

    private fun sortVideos(videos: List<Video>): List<Video> {
        val prefAudio = preferences.getString("preferred_audio", "all") ?: "all"
        val prefServer = preferences.getString("preferred_server", "any") ?: "any"
        val prefQuality = preferences.getString("preferred_quality", "1080p") ?: "1080p"
        return videos.sortedWith(
            compareBy<Video>(
                { audioRank(it.quality, prefAudio) },
                { serverRank(it.quality, prefServer) },
                { qualityRank(it.quality, prefQuality) },
                { -qualityHeight(it.quality) }
            )
        )
    }

    /** 0 = calidad preferida, 1 = desconocida, 2 = mayor (downscale), 3 = menor (upscale). */
    private fun qualityRank(quality: String, pref: String): Int {
        val target = pref.removeSuffix("p").toIntOrNull() ?: return 0
        val height = qualityHeight(quality)
        if (height == 0) return 1
        if (height == target) return 0
        return if (height > target) 2 else 3
    }

    private fun audioRank(quality: String, pref: String): Int = when (pref) {
        "sub" -> if (quality.contains("[SUB]")) 0 else 1
        "dub" -> if (quality.contains("[DUB]")) 0 else 1
        else -> 0
    }

    private fun serverRank(quality: String, pref: String): Int {
        val needle = when (pref) {
            "mp4upload" -> "MP4Upload"
            "voe" -> "Voe"
            "byse" -> "Byse"
            "zilla" -> "Zilla"
            "direct" -> "Direct"
            else -> return 0
        }
        return if (quality.contains(needle, ignoreCase = true)) 0 else 1
    }

    private fun qualityHeight(quality: String): Int {
        val raw = resolutionRegex.find(quality)?.groupValues?.get(1)?.toIntOrNull() ?: return 0
        return if (raw in 144..4320) raw else 0
    }
"##;

fn kt_path_literal(value: Option<&str>) -> String {
    value
        .unwrap_or("")
        .trim()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(['\n', '\r', '\t'], " ")
}

fn generate_kotlin_source(analysis: &AnalysisResult, display_name: &str, class_name: &str, pkg_slug: &str, lang: &str, pkg_lang: &str, pkg_prefix: &str) -> String {
    let base_url = &analysis.url;
    let name = display_name;
    let card_sel = &analysis.card_selector;
    let title_sel = if analysis.title_selector.trim().is_empty() {
        "img".to_string()
    } else {
        analysis.title_selector.clone()
    };
    let next_sel = &analysis.next_page_selector;
    // Selector de episodios comprobado en una ficha real; si el sondeo no pudo
    // abrir una, se mantiene el genérico.
    let episode_sel = if analysis.episode_selector.trim().is_empty() {
        "li.episode, a[href*=/ver/], div.episodios a".to_string()
    } else {
        analysis.episode_selector.clone()
    };
    let i18n = get_i18n(lang);

    // Rutas reales detectadas. Si no se detectaron, se dejan vacías y el
    // código generado usa la URL de entrada en vez de inventar `/animes`.
    let popular_path = kt_path_literal(analysis.popular_path.as_deref());
    let latest_path = kt_path_literal(analysis.latest_path.as_deref());
    let search_path = kt_path_literal(analysis.search_path.as_deref());
    let search_param = kt_path_literal(analysis.search_param.as_deref());
    let page_style = analysis.page_style.as_str();
    let supports_latest = if latest_path.is_empty() { "false" } else { "true" };

    let pref_audio_title = i18n.pref_audio_title;
    let pref_audio_entries = i18n.pref_audio_entries.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ");
    let pref_server_title = i18n.pref_server_title;
    let pref_quality_title = i18n.pref_quality_title;
    let pref_server_blacklist_title = i18n.pref_server_blacklist_title;
    let pref_server_blacklist_summary = i18n.pref_server_blacklist_summary;
    let pref_genre_blacklist_title = i18n.pref_genre_blacklist_title;
    let pref_genre_blacklist_summary = i18n.pref_genre_blacklist_summary;

    format!(r#"package {pkg_prefix}.{pkg_lang}.{pkg_slug}

import android.app.Application
import android.content.SharedPreferences
import android.net.Uri
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

class {class_name} : ParsedAnimeHttpSource(), ConfigurableAnimeSource {{

    override val name = "{name}"

    override val baseUrl = "{base_url}"

    override val lang = "{lang}"

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

    private val preferences: SharedPreferences by lazy {{
        Injekt.get<Application>().getSharedPreferences("source_$id", 0)
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
        title = element.selectFirst("{title_sel}")?.text()?.trim()?.takeIf {{ it.isNotEmpty() }}
            ?: link?.attr("title")?.trim()?.takeIf {{ it.isNotEmpty() }}
            ?: element.selectFirst("img[alt]")?.attr("alt")?.trim()?.takeIf {{ it.isNotEmpty() }}
            ?: "Sin Título"
        thumbnail_url = element.selectFirst("img")
            ?.let {{ img ->
                img.attr("abs:src")
                    .ifBlank {{ img.attr("abs:data-src") }}
                    .ifBlank {{ img.attr("abs:data-lazy-src") }}
            }}
            ?.takeIf {{ it.isNotEmpty() }}
    }}

    override fun popularAnimeNextPageSelector(): String? = "{next_sel}".ifBlank {{ null }}

    override fun popularAnimeParse(response: Response): AnimesPage =
        filterBlacklistedAnimes(super.popularAnimeParse(response))

    // ============================== LATEST UPDATES ==============================
    override fun latestUpdatesRequest(page: Int): Request = GET(pageUrl(latestPath, page), headers)

    override fun latestUpdatesSelector(): String = "{card_sel}"

    override fun latestUpdatesFromElement(element: Element): SAnime = popularAnimeFromElement(element)

    override fun latestUpdatesNextPageSelector(): String? = "{next_sel}".ifBlank {{ null }}

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
            searchUrl(finalQuery, page)
        }} else {{
            pageUrl(listingPath, page)
        }}
        return GET(url, headers)
    }}

    override fun searchAnimeSelector(): String = "{card_sel}"

    override fun searchAnimeFromElement(element: Element): SAnime = popularAnimeFromElement(element)

    override fun searchAnimeNextPageSelector(): String? = "{next_sel}".ifBlank {{ null }}

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

    override fun getFilterList(): AnimeFilterList = {class_name}Filters.getFilterList()

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

{video_engine}

    // ============================== PREFERENCES ==============================
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

        val serverPref = ListPreference(screen.context).apply {{
            key = "preferred_server"
            title = "{pref_server_title}"
            entries = arrayOf("Any", "MP4Upload", "Voe", "Byse (StreamWish)", "Zilla (HLS)", "Direct")
            entryValues = arrayOf("any", "mp4upload", "voe", "byse", "zilla", "direct")
            setDefaultValue("any")
            summary = "%s"
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
"#, video_engine = VIDEO_ENGINE_KT)
}

fn generate_manga_source(
    analysis: &AnalysisResult,
    display_name: &str,
    class_name: &str,
    pkg_slug: &str,
    lang: &str,
    pkg_lang: &str,
    pkg_prefix: &str,
) -> String {
    let base_url = &analysis.url;
    let name = display_name;
    let card_sel = &analysis.card_selector;
    let title_sel = if analysis.title_selector.trim().is_empty() {
        "img".to_string()
    } else {
        analysis.title_selector.clone()
    };
    let next_sel = &analysis.next_page_selector;

    let popular_path = kt_path_literal(analysis.popular_path.as_deref());
    let latest_path = kt_path_literal(analysis.latest_path.as_deref());
    let search_path = kt_path_literal(analysis.search_path.as_deref());
    let search_param = kt_path_literal(analysis.search_param.as_deref());
    let page_style = analysis.page_style.as_str();
    let supports_latest = if latest_path.is_empty() { "false" } else { "true" };

    format!(r#"package {pkg_prefix}.{pkg_lang}.{pkg_slug}

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

    override val lang = "{lang}"

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
"#)
}

fn generate_manga_filters(
    class_name: &str,
    pkg_slug: &str,
    pkg_lang: &str,
    pkg_prefix: &str,
) -> String {
    format!(r#"package {pkg_prefix}.{pkg_lang}.{pkg_slug}

import eu.kanade.tachiyomi.source.model.FilterList

object {class_name}Filters {{
    fun getFilterList(): FilterList = FilterList()
}}
"#)
}

fn generate_kotlin_filters(class_name: &str, pkg_slug: &str, lang: &str, pkg_lang: &str, pkg_prefix: &str) -> String {
    let i18n = get_i18n(lang);
    let filter_header = i18n.filter_header;
    let filter_audio_title = i18n.filter_audio_title;
    let filter_genre_title = i18n.filter_genre_title;
    let audio_entries = i18n.filter_audio_entries.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ");

    let genres_list = i18n.genres.iter()
        .map(|(name, id)| format!("        Genre(\"{}\", \"{}\")", name, id))
        .collect::<Vec<_>>()
        .join(",\n");

    format!(r#"package {pkg_prefix}.{pkg_lang}.{pkg_slug}

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

fn generate_android_manifest(pkg_id: &str, class_name: &str, app_label: &str, ext_type: &str) -> String {
    let tag = if ext_type == "manga" {
        "tachiyomi.extension"
    } else {
        "tachiyomi.animeextension"
    };

    format!(r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">

    <application
        android:label="{app_label}"
        android:icon="@mipmap/ic_launcher"
        android:allowBackup="true"
        android:supportsRtl="true">
        <meta-data
            android:name="{tag}.class"
            android:value="{pkg_id}.{class_name}" />
        <meta-data
            android:name="{tag}.hasSettings"
            android:value="true" />
        <meta-data
            android:name="{tag}.nsfw"
            android:value="0" />
    </application>
</manifest>
"#)
}

fn generate_build_gradle(
    pkg_id: &str,
    version_code: i32,
    version_name: &str,
    pkg_lang: &str,
    pkg_slug: &str,
    ext_type: &str,
) -> String {
    // extensions-lib de Aniyomi solo trae `animesource`. Los scrapers de manga
    // usan la API `eu.kanade.tachiyomi.source` de Mihon/Tachiyomi, que está en
    // la librería original; sin esto el proyecto de manga no compila.
    let extensions_lib = if ext_type == "manga" {
        "com.github.tachiyomiorg:extensions-lib:1.4.4"
    } else {
        "com.github.aniyomiorg:extensions-lib:14"
    };
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

    base {{
        archivesName.set("aniyomi-{pkg_lang}.{pkg_slug}-v{version_name}")
    }}

    signingConfigs {{
        create("release") {{
            storeFile = rootProject.file("signingkey.jks")
            storePassword = "aextsigning"
            keyAlias = "aext"
            keyPassword = "aextsigning"
        }}
    }}

    buildTypes {{
        release {{
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("release")
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
        }}
    }}

    compileOptions {{
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }}

    kotlinOptions {{
        jvmTarget = "1.8"
        freeCompilerArgs += "-opt-in=kotlinx.serialization.ExperimentalSerializationApi"
    }}
}}

dependencies {{
    compileOnly("{extensions_lib}")
    compileOnly("androidx.preference:preference:1.2.1")
    compileOnly("uy.kohesive.injekt:injekt-core:1.16.1")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.6.2")
    implementation("org.jsoup:jsoup:1.17.2")
    implementation("com.squareup.okhttp3:okhttp:5.0.0-alpha.12")
}}
"#)
}

fn generate_settings_gradle(pkg_lang: &str, pkg_slug: &str) -> String {
    format!(r#"pluginManagement {{
    repositories {{
        gradlePluginPortal()
        google()
        mavenCentral()
    }}
}}

dependencyResolutionManagement {{
    repositoriesMode.set(RepositoriesMode.PREFER_SETTINGS)
    repositories {{
        google()
        mavenCentral()
        maven(url = "https://jitpack.io")
    }}
}}

rootProject.name = "nekoyomi_extensions"

include(":src:{pkg_lang}:{pkg_slug}")
"#)
}

const ROOT_BUILD_GRADLE: &str = r#"plugins {
    id("com.android.application") version "8.2.2" apply false
    id("org.jetbrains.kotlin.android") version "1.9.22" apply false
    id("org.jetbrains.kotlin.plugin.serialization") version "1.9.22" apply false
}

tasks.register("clean", Delete::class) {
    delete(rootProject.layout.buildDirectory)
}
"#;

const GRADLE_PROPERTIES: &str = r#"org.gradle.jvmargs=-Xmx2048m -Dfile.encoding=UTF-8
android.useAndroidX=true
android.enableJetifier=true
kotlin.code.style=official
"#;

const GITIGNORE: &str = r#".gradle/
build/
**/build/
.idea/
*.iml
local.properties
__pycache__/
*.pyc
"#;


fn generate_github_workflow() -> String {
    r#"name: Build and Publish Nekoyomi Extension

on:
  push:
  workflow_dispatch:

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

permissions:
  contents: write
  pages: write
  id-token: write

jobs:
  build:
    name: Build APK and Update Repo
    runs-on: ubuntu-latest
    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Set up JDK 17
        uses: actions/setup-java@v5
        with:
          java-version: '17'
          distribution: 'temurin'

      - name: Setup Gradle
        uses: gradle/actions/setup-gradle@v3
        with:
          gradle-version: 8.5

      - name: Build Extension APK
        run: gradle assembleRelease --stacktrace

      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.11'

      - name: Generate Repo Index
        run: |
          python3 .github/scripts/generate-repo-index.py --out repo_dist

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
    r#"#!/usr/bin/env python3
"""Genera el repositorio de extensiones para Nekoyomi / Aniyomi.

Lee los APK compilados por Gradle y produce:
  - index.json / index.min.json (formato JSON, el canónico)
  - repo.json (con huella de firma)
  - copia de APKs e iconos para descarga directa

No se publica `index.pb`: Nekoyomi puede descargarlo con prioridad y un
protobuf inválido (JSON copiado) rompería el repositorio. Sin `index.pb` y con
`index_v2` en null, los clientes usan `index.min.json`, que sí es válido.
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

DEFAULT_FINGERPRINT = "fb69d41317176e8690fb80115730b31964748aabbf2fd4fc5b830888ae950306"


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as handle:
        while True:
            chunk = handle.read(8192)
            if not chunk:
                break
            h.update(chunk)
    return h.hexdigest()


def repair_zip_and_fingerprint(apk_path: Path) -> str:
    """Intenta leer la huella del certificado del APK con keytool."""
    try:
        result = subprocess.run(
            ["keytool", "-printcert", "-jarfile", str(apk_path)],
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode == 0:
            match = re.search(r"SHA256:\s*([A-Fa-f0-9:]+)", result.stdout)
            if match:
                clean = match.group(1).replace(":", "").lower().strip()
                if len(clean) == 64:
                    return clean
    except Exception as error:
        print(f"[!] keytool no disponible: {error}")
    return DEFAULT_FINGERPRINT


def source_id(pkg: str) -> int:
    digest = hashlib.sha256(pkg.encode("utf-8")).hexdigest()
    return int(digest[:15], 16) % (2 ** 62)


def read_module_meta(apk_path: Path) -> dict:
    meta = {
        "pkg": "",
        "lang": "es",
        "slug": "",
        "code": 1,
        "version": "16.1",
        "name": None,
        "baseUrl": "",
    }

    apk_path = apk_path.resolve()
    module = None
    for parent in apk_path.parents:
        gradle = parent / "build.gradle.kts"
        if not gradle.exists():
            continue
        module = parent
        text = gradle.read_text(encoding="utf-8", errors="ignore")

        match = re.search(r'applicationId\s*=\s*"([^"]+)"', text)
        if match:
            meta["pkg"] = match.group(1)
        match = re.search(r"versionCode\s*=\s*(\d+)", text)
        if match:
            meta["code"] = int(match.group(1))
        match = re.search(r'versionName\s*=\s*"([^"]+)"', text)
        if match:
            meta["version"] = match.group(1)
        break

    if module is not None:
        parts = list(module.parts)
        if "src" in parts:
            index = parts.index("src")
            if len(parts) >= index + 3:
                meta["lang"] = parts[index + 1]
                meta["slug"] = parts[index + 2]

        for kotlin in sorted(module.rglob("*.kt")):
            text = kotlin.read_text(encoding="utf-8", errors="ignore")
            match = re.search(r'override\s+val\s+name\s*=\s*"([^"]+)"', text)
            if match and not meta["name"]:
                meta["name"] = match.group(1)
            match = re.search(r'override\s+val\s+baseUrl\s*=\s*"([^"]+)"', text)
            if match and not meta["baseUrl"]:
                meta["baseUrl"] = match.group(1)
            if meta["name"] and meta["baseUrl"]:
                break

    filename = apk_path.name
    match = re.search(
        r"(?:aniyomi|tachiyomi)-([a-zA-Z0-9_-]+)\.([a-zA-Z0-9_.-]+)-v([0-9.]+)(?:-release)?\.apk",
        filename,
    )
    if match:
        if not meta["lang"]:
            meta["lang"] = match.group(1)
        if not meta["slug"]:
            meta["slug"] = match.group(2)
        if not meta["version"]:
            meta["version"] = match.group(3)

    if not meta["pkg"]:
        prefix = "eu.kanade.tachiyomi.animeextension"
        meta["pkg"] = f"{prefix}.{meta['lang']}.{meta['slug'] or 'extension'}"

    if not meta["slug"]:
        meta["slug"] = meta["pkg"].split(".")[-1]

    return meta


def find_icon(apk_path: Path) -> Path | None:
    apk_path = apk_path.resolve()
    for parent in apk_path.parents:
        if (parent / "build.gradle.kts").exists():
            icon = parent / "src" / "main" / "res" / "mipmap-xxxhdpi" / "ic_launcher.png"
            if icon.exists():
                return icon
            icon = parent / "src" / "main" / "res" / "mipmap-xxhdpi" / "ic_launcher.png"
            if icon.exists():
                return icon
            return None
    return None


def main() -> None:
    parser = argparse.ArgumentParser(description="Generador de índice de repositorio Nekoyomi")
    parser.add_argument("--out", default="repo_dist", help="Carpeta de salida publicada en gh-pages")
    parser.add_argument("--repo-name", default="Nekoyomi Extensions", help="Nombre visible del repositorio")
    parser.add_argument("--repo-url", default="https://github.com", help="URL pública del repositorio")
    args = parser.parse_args()

    output_dir = Path(args.out)
    if not output_dir.is_absolute():
        output_dir = ROOT / output_dir
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "apk").mkdir(parents=True, exist_ok=True)
    (output_dir / "icon").mkdir(parents=True, exist_ok=True)

    # Se busca desde la raíz del repositorio, no desde el directorio actual: así
    # el script funciona tanto en Actions como ejecutado a mano desde cualquier carpeta.
    apk_files = sorted((ROOT / "src").rglob("build/outputs/apk/release/*.apk"))
    print(f"[*] APKs encontrados: {len(apk_files)}")

    extensions = []
    fingerprint = DEFAULT_FINGERPRINT
    generic_icon = output_dir / "icon" / "ic_launcher.png"

    for apk in apk_files:
        meta = read_module_meta(apk)
        filename = apk.name

        shutil.copy2(apk, output_dir / filename)
        shutil.copy2(apk, output_dir / "apk" / filename)

        icon = find_icon(apk)
        if icon is not None:
            shutil.copy2(icon, generic_icon)
            shutil.copy2(icon, output_dir / "icon" / f"{meta['pkg']}.png")

        if fingerprint == DEFAULT_FINGERPRINT:
            fingerprint = repair_zip_and_fingerprint(apk)

        display_name = meta["name"] or meta["slug"]
        extensions.append(
            {
                "name": f"Aniyomi: {display_name}",
                "pkg": meta["pkg"],
                "apk": filename,
                "lang": meta["lang"],
                "code": meta["code"],
                "version": meta["version"],
                "nsfw": 0,
                "hasReadme": 0,
                "hasChangelog": 0,
                "sha256": sha256_file(apk),
                "size": apk.stat().st_size,
                "sources": [
                    {
                        "name": display_name,
                        "lang": meta["lang"],
                        "id": source_id(meta["pkg"]),
                        "baseUrl": meta["baseUrl"],
                    }
                ],
            }
        )
        print(f"  [+] {display_name} v{meta['version']} ({meta['lang']}) -> {filename}")

    index_json = output_dir / "index.json"
    index_min = output_dir / "index.min.json"
    with open(index_json, "w", encoding="utf-8") as handle:
        json.dump(extensions, handle, indent=2, ensure_ascii=False)
    with open(index_min, "w", encoding="utf-8") as handle:
        json.dump(extensions, handle, separators=(",", ":"), ensure_ascii=False)

    # No se escribe index.pb a propósito (ver docstring): un pb inválido hace
    # que los clientes que lo piden primero descarten el repositorio entero.
    stale_pb = output_dir / "index.pb"
    if stale_pb.exists():
        stale_pb.unlink()

    repo_meta = {
        "index_v2": None,
        "meta": {
            "name": args.repo_name,
            "shortName": args.repo_name.lower().replace(" ", ""),
            # GitHub Actions expone "owner/repo"; fuera de Actions se deja el
            # valor de --repo-url si se pasó.
            "website": (
                f"https://github.com/{os.environ['GITHUB_REPOSITORY']}"
                if os.environ.get("GITHUB_REPOSITORY")
                else args.repo_url
            ),
            "signingKeyFingerprint": fingerprint,
        },
    }
    with open(output_dir / "repo.json", "w", encoding="utf-8") as handle:
        json.dump(repo_meta, handle, indent=2, ensure_ascii=False)

    print(f"[+] Índice generado en {output_dir} (fingerprint {fingerprint})")
    if not extensions:
        print("[!] No se generó ningún APK. Revisa el paso 'Build Extension APK'.")


if __name__ == "__main__":
    main()
"#.to_string()
}

fn generate_repo_readme(name: &str, lang: &str, base_url: &str, version: &str) -> String {
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
- **Versión:** {version}
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

const KEYSTORE_B64: &str = "/u3+7QAAAAIAAAABAAAAAQAEYWV4dAAAAaHm9EyTAAAFADCCBPwwDAYKKwYBBAEqAhEBAQSCBOos1ncT4LCh/jNFikMLMZIqb1P11AprITV0zKdD8nFnTFgI8SL+6YcDJq8SvFDKdZFsKDr93jP6Ig3uhj+2+ltwJi10vy2O9Xeoo3MSovwwCvA9q42RF54h6zjDWYXo+VciFH03215UpQyRCKitmbhZq5rw32W+xwOT5EXFqhM/AmSt7brlLG9CXX0n8HoMWWLnaiMEaQw2B/oTnYqpYadvxCrxDgd6Vz9CO6FAFjKPlCmEd3G0kXTlu07YNNnYwfRCQ1CrSQ+XUuIjQPMYXARBMsrCQv3K2c3tM2qt0LLvZtj+jgntQ06Akp8qC1jdYEJ1XJ2edUGbr4gMhEsxMP9cr1P4EZHrWqHa7wX8wfMXTaFNAgVsdH9vCxx4Bj3otRRs1iX0xKvFR28b9oMQ+rurKhYG0kc9+R7ZkD0UJ74OpO0kjiCjQkyy9dkP6asetAaDsYgm/wXffl0iBkLcdZCbZeSNp3hmD0XWKFaLDe2bcuZRPj62igg7qU492WJgHvPx1vA3qfIOnYAFgkCISfzFv6nVa46vWM0p9zTKmLz36zmzmBtbLlf4PiINlMKl1U/IWJBtDGjl6mLy8fettFTG2rqevyiRl4Ue5aiJ0v+bWqOeaycvT2zrCM+GJ/M+92LfgibLGwa+QmEx+ayzM6adSLYgDRdn6hW4o8uvZicCJzAbnhI9Ic7cWGN6lDlvPU4FF8NxDv490V4BpOmn3hmEYq3v1VRjRs9/6sabKevDy2wfvJZK1BxbiIbDfDOvregfwoUwzSARwzxg437CEPJzp07nf7FjS/hVdaTxSh+ugNl18YxmeoXBfZx3PpC4E7gX2l4p/Bt+jGXXt+hesJiKmO76BQE75wcjSVgQp0S69KenqemVGEB5RKkcX3Ikrhgxhmgr8urrbFCNgF+M01xB9dtlAgpEadRSdiYQ9JGAlwD8a1BJ8lqNGjquRqzUci4O3nYahoV80YZGsCL5f3sQ9s7UN3qW+rDO00Fj+Yjl7j1WCew2ur+szpTZKA7oqtA3DhiHtR7k6gshGxFduOSW30+58K/y4OsAXxcoY4EN0IPUmdW+cY12/01dUxrRUKELfBEvxvKKBdqhh3Oz4a6CC6YyLN0Vqrp+TL4ZPCnzkTrOutORk/nJNg8TWCnK9v8e1qY3PEkG9wjJofXbriDgMNCGlIt/5vAYdS5YCeMTo1zqUb5jXWMWqQ0ZBAyZ/bsxDrmpjOVH6pFxBC5AoQLbkYENv8sb4oYCLvVR7ne5BuGNay5CL4DUEyO4VwtfT6YS9DBpbonXNFZ2C/dcm8O+CXnIqVUtlwjVSSnZSoRdfHG0Q/b9unXKUeIjdXUwiH0yYiCEyfA+HacVSwwu/e86W0XoEzxWYj2B5qK700U4d6gFxnVPxAx0yYTfnaHcfsCE9queiPSn0N4i5S7cKJvw2hKhRpkInAZOv2aJ9Xo9hmWNLj6k1x1J4HxWCJ21ey1Nq2cgtI5o+KXtJfvj/dInpmfaDK6NKPf+FK15amBzR7CgdeNEnFEfEBXBgruzG7zNl/VeUUZ+qKudVM2aGN7t59KbQ/vU8KGKNBQytVCdM8qDWHGLPQ8+Y00muKb6nXixkmhJZwoU5u/12G/N63WCcsDaNPCxWHpB7x9xIGB9DiEVdDMpA15hvBgP+Yq5/aF4WggSoCA66/OcAAAAAQAFWC41MDkAAAN/MIIDezCCAmOgAwIBAgIIbVKddlSR/iAwDQYJKoZIhvcNAQEMBQAwazELMAkGA1UEBhMCVVMxDjAMBgNVBAgTBUxvY2FsMQ4wDAYDVQQHEwVMb2NhbDENMAsGA1UEChMEQUVYVDETMBEGA1UECxMKRXh0ZW5zaW9uczEYMBYGA1UEAxMPQUVYVCBFeHRlbnNpb25zMCAXDTI2MTExNzAwNDIxMloYDzIwNTYxMTA5MDA0MjEyWjBrMQswCQYDVQQGEwJVUzEOMAwGA1UECBMFTG9jYWwxDjAMBgNVBAcTBUxvY2FsMQ0wCwYDVQQKEwRBRVhUMRMwEQYDVQQLEwpFeHRlbnNpb25zMRgwFgYDVQQDEw9BRVhUIEV4dGVuc2lvbnMwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQCsove4Xoi2kUFLk7uEYXsWhl/qZDNQjeAWUNfge1ohTSubYk1HqgEEJuofhNrMWrfxCHt1rv1bz7cIYCpV8zg0tJeQgQZvQN39HMGEBwE79qA3g5Wo1pH3WOzuTqzaVCILJAqIuu8USY3JM6NcebyQpPSE+cCaeebZBHIYBS/+3bwwVGJPWHG1Ykt37bWL76ukLp0ZyRRJlJ2MTdt+c7k4ZwlQGD46+h4ukVBHwsCsgmw4SRQHbKLpGa5UM0/fkrvPMn83Y8KWPGh8vU7oH7T7ZLDhLFC26/3jGwMBAca96UHFvTFXlYGYdzH8KJLxb49alcnukLKQeiNwB3iHLxxZAgMBAAGjITAfMB0GA1UdDgQWBBTPDrtJgQhmX4mhpq3cYwbItP0OKTANBgkqhkiG9w0BAQwFAAOCAQEAIjZEJwSj83hns1hKjVqH521dyGTEQtOZiQXS7pBdr2hjmQ1rmusoN6Ge95OgP8h0EJVDPwbxWbHzC5d8+D9EeHwe0UyWrYrD5dnsbyksiHgkywBqAscyWxzdPrwRbw1ILZ556c0h4LMSPHJ8dew70n4oQFJxi9O1QO3WHKRas+Ng06wyRrZdAjjELurX/QVqc1bHSqCwKJjbfEeMbT74Sm8Avq3Y/7g6pMDTFdW0YUMQrvl6ZT2t7BYelU/fWNi/kdr35Mb18c+RooaeyibFkGfMzIYYsy7f7eC3wldXh4wwyn0jRoGIv0UWqsL6iGI1QV1lx9tSAeKi3hLeYp5HKtrT7FS9m1AMH1orycc+H4GD7Er+";
