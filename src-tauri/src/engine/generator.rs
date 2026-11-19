use crate::engine::analyzer::{AnalysisResult, SiteAnalyzer, SiteCapabilities};
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
                generate_kotlin_filters(
                    &class_name,
                    &pkg_slug,
                    lang,
                    &pkg_lang,
                    pkg_prefix,
                    &analysis.capabilities,
                ),
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

        // 5b. Wrapper fijado a Gradle 9.5.1. Sin esto, la compilación local usa
        // cualquier Gradle que haya en el PATH y la versión puede no coincidir
        // con la que soporta el AGP/Kotlin generados.
        files.insert("gradlew".to_string(), GRADLEW_SH.as_bytes().to_vec());
        files.insert("gradlew.bat".to_string(), GRADLEW_BAT.as_bytes().to_vec());
        files.insert(
            "gradle/wrapper/gradle-wrapper.properties".to_string(),
            GRADLE_WRAPPER_PROPERTIES.as_bytes().to_vec(),
        );
        files.insert(
            "gradle/wrapper/gradle-wrapper.jar".to_string(),
            decode_base64(GRADLE_WRAPPER_JAR_B64),
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

        // AEXT NO genera README.md: el README pertenece al repositorio y puede
        // contener la URL, el disclaimer y las notas del usuario. Sobrescribirlo
        // en cada extensión (y poner el nombre de la generación en el título)
        // destruía su contenido. El índice gh-pages/index.min.json ya aporta los
        // enlaces de instalación; no hace falta un README por extensión.

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
    pref_audio_summary: &'static str,
    pref_api_title: &'static str,
    pref_api_entries: &'static [&'static str],
    /// Etiqueta del aviso que enumera los métodos encontrados de reproducción.
    methods_title: &'static str,
    pref_server_title: &'static str,
    pref_server_summary: &'static str,
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

/// Devuelve (mostrar_selector, sub_verificado, dub_verificado). Sin un máster
/// HLS abierto por AEXT no se genera ningún control SUB/DUB: el DOM o un texto
/// "DUB" en el HTML no es evidencia de una pista real.
fn verified_audio_selection(caps: &SiteCapabilities) -> (bool, bool, bool) {
    let sub = caps.master_verified && caps.sub_verified;
    let dub = caps.master_verified && caps.dub_verified;
    (sub || dub, sub, dub)
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

    // ============================== VERIFICACIÓN DE STREAMS ==============================
    // Un HTTP 200 en el máster no garantiza que el reproductor pueda abrir el
    // stream: pueden fallar las cabeceras, la clave AES-128 o un segmento.
    // Comprobamos la cadena completa antes de entregar el Video al reproductor.
    private data class StreamProbe(
        val ok: Boolean,
        val detail: String,
        val size: Int = 0,
        val body: String = "",
        val inconclusive: Boolean = false,
        // Pistas reales declaradas por el máster HLS. Se usan para etiquetar
        // el vídeo con idiomas detectados, nunca con un DUB supuesto.
        val audioLangs: List<String> = emptyList(),
        val subLangs: List<String> = emptyList(),
    )

    /** Lee `#EXT-X-MEDIA` y devuelve (audio, subtítulos) con su idioma real. */
    private fun hlsTracks(body: String): Pair<List<String>, List<String>> {
        val audio = linkedSetOf<String>()
        val subs = linkedSetOf<String>()
        for (line in body.lineSequence()) {
            if (!line.startsWith("#EXT-X-MEDIA")) continue
            val type = Regex("""TYPE=([A-Z]+)""").find(line)?.groupValues?.get(1)?.uppercase() ?: continue
            val lang = Regex("""LANGUAGE="([^"]+)"""").find(line)?.groupValues?.get(1)?.trim().orEmpty()
            val name = Regex("""NAME="([^"]+)"""").find(line)?.groupValues?.get(1)?.trim().orEmpty()
            val code = lang.takeIf { it.isNotEmpty() && it != "und" } ?: name
            if (code.isEmpty()) continue
            if (type == "AUDIO") audio.add(code) else if (type == "SUBTITLES") subs.add(code)
        }
        return audio.toList() to subs.toList()
    }

    private val browserHeaders: Headers by lazy {
        Headers.Builder()
            .set("User-Agent", userAgent)
            .set("Referer", baseUrl)
            .set("Accept", "*/*")
            .set("Accept-Language", "es-ES,es;q=0.9,en;q=0.8")
            .build()
    }

    private val hlsKeyRegex = Regex("#EXT-X-KEY[^\\r\\n]*URI=\"([^\"]+)\"")

    private fun isPlaylist(url: String): Boolean =
        url.substringBefore("?").endsWith(".m3u8", ignoreCase = true)

    private fun probeOnce(url: String, extra: Headers, readBody: Boolean = false): StreamProbe = try {
        val request = Request.Builder().url(url).headers(extra).header("Range", "bytes=0-4095").build()
        client.newCall(request).execute().use { response ->
            val body = if (readBody || isPlaylist(url)) response.body?.string().orEmpty() else ""
            val contentType = response.header("Content-Type").orEmpty()
            val shortUrl = url.substringBefore('?')
            Log.d("AEXT", "probe $shortUrl -> HTTP ${response.code} $contentType (${body.length} bytes)")
            val tracks = if (body.contains("#EXT-X-MEDIA")) hlsTracks(body) else emptyList<String>() to emptyList<String>()
            // Un 200 con una playlist vacía o una página de error es el fallo
            // típico que el reproductor resume como `loading failed`. Lo
            // convertimos en un error concreto y trazable.
            val detail = when {
                !response.isSuccessful -> if (response.code == 403) "HTTP 403 (token/región rechazados)" else "HTTP ${response.code}"
                contentType.startsWith("text/html", ignoreCase = true) -> "HTML en vez de stream"
                body.contains("BOT_SESSION_BLOCKED", ignoreCase = true) -> "sesión bloqueada por el sitio"
                isPlaylist(url) && !body.contains("#EXTM3U") -> "playlist no válida"
                isPlaylist(url) && !body.contains("#EXT-X-STREAM-INF") && !body.contains("#EXTINF") ->
                    "playlist vacía (${body.length} bytes)"
                else -> ""
            }
            if (detail.isNotEmpty()) {
                Log.w("AEXT", "probe fallida $shortUrl: $detail")
                StreamProbe(false, detail, body.length, body)
            } else {
                StreamProbe(
                    true, "HTTP ${response.code}", body.length, body,
                    audioLangs = tracks.first, subLangs = tracks.second,
                )
            }
        }
    } catch (e: Exception) {
        // Un fallo de red no prueba que el stream esté roto: conservamos el
        // candidato y dejamos constancia de que no se pudo verificar.
        Log.w("AEXT", "probe de red falló: ${url.substringBefore('?')} -> ${e.message}", e)
        StreamProbe(false, e.message ?: "error de red", inconclusive = true)
    }

    private fun checkStream(url: String, extra: Headers): StreamProbe {
        val first = probeOnce(url, extra, readBody = true)
        if (!first.ok || !isPlaylist(url)) return first
        val childLine = first.body.lineSequence()
            .map { it.trim() }
            .firstOrNull { it.isNotBlank() && !it.startsWith("#") }
            ?: return StreamProbe(false, "máster sin variantes")
        val childUrl = absolutize(url, childLine)
        Log.d("AEXT", "variante ${childUrl.substringBefore('?')}")
        val child = probeOnce(childUrl, extra, readBody = true)
        if (!child.ok && !child.inconclusive) {
            return StreamProbe(false, "variante: ${child.detail}", inconclusive = child.inconclusive)
        }
        if (child.ok && !child.body.contains("#EXTINF") && !child.body.contains("#EXT-X-STREAM-INF")) {
            return StreamProbe(false, "variante vacía (${child.body.length} bytes)")
        }
        val keyRef = hlsKeyRegex.find(child.body)?.groupValues?.get(1)
            ?: hlsKeyRegex.find(first.body)?.groupValues?.get(1)
        if (keyRef != null) {
            val key = probeOnce(absolutize(childUrl, keyRef), extra, readBody = true)
            if (!key.ok && !key.inconclusive) return StreamProbe(false, "clave AES: ${key.detail}")
            if (key.ok && key.size == 0) {
                return StreamProbe(false, "clave AES vacía")
            }
        }
        Log.d("AEXT", "HLS verificado ${url.substringBefore('?')}")
        return StreamProbe(
            true, "HLS verificado", first.size,
            audioLangs = (first.audioLangs + child.audioLangs).distinct(),
            subLangs = (first.subLangs + child.subLangs).distinct(),
        )
    }

    private fun capabilityRank(video: Video): Int {
        val url = video.videoUrl.orEmpty()
        val format = when {
            isPlaylist(url) -> "hls"
            url.contains(".mpd", ignoreCase = true) -> "dash"
            url.contains(".mp4", ignoreCase = true) -> "mp4"
            else -> ""
        }
        return if (format.isNotEmpty() && siteStreamFormats.contains(format)) 0 else 1
    }

    /** Verifica cada candidato; si falla con sus cabeceras, reintenta con
     *  cabeceras de navegador y entrega el Video con las que sí funcionaron.
     *  Si ninguno funciona, lanza un error concreto en vez de dejar que el
     *  reproductor muestre un genérico `loading failed`. */
    /** Etiqueta con idiomas REALES del HLS. Si no hay pistas declaradas, no añade nada. */
    private fun labelWithTracks(video: Video, probe: StreamProbe): Video {
        val parts = mutableListOf<String>()
        if (probe.audioLangs.isNotEmpty()) parts.add("audio " + probe.audioLangs.joinToString("/"))
        if (probe.subLangs.isNotEmpty()) parts.add("subs " + probe.subLangs.joinToString("/"))
        if (probe.inconclusive) parts.add("sin verificar")
        if (parts.isEmpty()) return video
        return video.copy(quality = video.quality + " · " + parts.joinToString(" · "))
    }

    private fun verifyCandidates(candidates: List<Video>): List<Video> {
        if (candidates.isEmpty()) return candidates
        val verified = mutableListOf<Video>()
        val failures = mutableListOf<String>()
        for (video in candidates.sortedBy(::capabilityRank)) {
            val url = video.videoUrl ?: continue
            val own = checkStream(url, video.headers ?: headers)
            if (own.ok || own.inconclusive) {
                if (own.inconclusive) {
                    Log.w("AEXT", "candidato entregado sin verificación de red: ${url.substringBefore('?')} (${own.detail})")
                }
                verified.add(labelWithTracks(video, own))
                continue
            }
            val browser = checkStream(url, browserHeaders)
            if (browser.ok || browser.inconclusive) {
                val chosen = if (browser.ok) {
                    labelWithTracks(video, browser).copy(headers = browserHeaders)
                } else {
                    labelWithTracks(video, browser)
                }
                if (browser.inconclusive) {
                    Log.w("AEXT", "candidato entregado con cabeceras de navegador sin verificar: ${url.substringBefore('?')}")
                }
                verified.add(chosen)
                continue
            }
            val failure = "${url.substringBefore('?')}: ${own.detail}; con cabeceras de navegador: ${browser.detail}"
            failures.add(failure)
            Log.w("AEXT", "stream descartado: $failure")
        }
        if (verified.isEmpty()) {
            val message = "AEXT no pudo verificar ningún stream (${failures.take(3).joinToString(" | ")})"
            Log.e("AEXT", message)
            throw Exception(message)
        }
        return distinctVideos(verified)
    }

    // ============================== ESTRATEGIAS DE RESOLUCIÓN ==============================
    // Un sitio puede exponer el stream por sesión API, por GraphQL o como
    // embebido en HTML. AEXT no asume una sola ruta: prueba varias en orden y
    // aprende cuál funciona: guarda aciertos/fallos por estrategia y ordena
    // las siguientes reproducciones por probabilidad de éxito (suavizado de
    // Laplace). Así, si el sitio cambia de API, la estrategia que empieza a
    // fallar baja de puesto automáticamente sin regenerar la extensión.
    private data class VideoContext(val url: String, val html: String)

    private class VideoStrategy(
        val id: String,
        val priority: Int,
        val resolve: (VideoContext) -> List<Video>?,
    )

    private fun strategyStats(id: String): Pair<Int, Int> {
        val raw = preferences.getString("aext_strategy_$id", "0,0") ?: "0,0"
        val parts = raw.split(",")
        return (parts.getOrNull(0)?.toIntOrNull() ?: 0) to (parts.getOrNull(1)?.toIntOrNull() ?: 0)
    }

    private fun recordStrategy(id: String, success: Boolean) {
        val (ok, fail) = strategyStats(id)
        preferences.edit()
            .putString("aext_strategy_$id", if (success) "${ok + 1},$fail" else "$ok,${fail + 1}")
            .apply()
    }

    private fun strategyPriority(id: String): Int {
        // El caso detectado fija el orden inicial; las estadísticas reales lo
        // corrigen después. Así no empezamos probando una ruta imposible.
        if (sitePlayerCases.contains("session-bootstrap")) {
            return when (id) {
                "api-session" -> 0
                "api-graphql" -> 1
                else -> 2
            }
        }
        if (sitePlayerCases.contains("graphql-persisted")) {
            return when (id) {
                "api-graphql" -> 0
                "api-session" -> 1
                else -> 2
            }
        }
        if (sitePlayerCases.contains("iframe-player") || sitePlayerCases.contains("dash-mpd")) {
            return when (id) {
                "html-embeds" -> 0
                "api-session" -> 1
                else -> 2
            }
        }
        return when (id) {
            "api-session" -> 0
            "api-graphql" -> 1
            else -> 2
        }
    }

    /** El usuario puede forzar la ruta real; `auto` es la recomendación. */
    private fun apiMode(): String = preferences.getString("api_mode", "auto")
        ?.takeIf { it in setOf("auto", "session", "graphql", "html") } ?: "auto"

    private fun apiModeLabel(mode: String): String = when (mode) {
        "session" -> "sesión web"
        "graphql" -> "GraphQL interno"
        "html" -> "scraping HTML"
        else -> "Automático"
    }

    private fun orderedStrategies(): List<VideoStrategy> {
        val all = listOf(
            VideoStrategy("api-session", strategyPriority("api-session")) { ctx -> apiSessionVideos(ctx) },
            VideoStrategy("api-graphql", strategyPriority("api-graphql")) { ctx -> plutoGraphqlVideos(ctx) },
            VideoStrategy("html-embeds", strategyPriority("html-embeds")) { ctx -> htmlEmbedVideos(ctx) },
        )
        // Un modo explícito NO es una preferencia decorativa: limita de verdad
        // qué resolutores se ejecutan, para poder diagnosticar fallos por API.
        val selected = when (apiMode()) {
            "session" -> all.filter { it.id == "api-session" }
            "graphql" -> all.filter { it.id == "api-graphql" }
            "html" -> all.filter { it.id == "html-embeds" }
            else -> all
        }
        return selected.sortedWith(
            compareByDescending<VideoStrategy> { s ->
                val (ok, fail) = strategyStats(s.id)
                (ok + 1.0) / (ok + fail + 2.0)
            }.thenBy { it.priority }
        )
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
        val finalUrl = response.request.url.toString()
        // El cuerpo se lee una sola vez: el HTML se reutiliza en todas las
        // estrategias y `asJsoup()` también lo consumiría.
        val html = response.body?.string().orEmpty()
        val ctx = VideoContext(finalUrl, html)
        val errors = mutableListOf<String>()
        val strategies = orderedStrategies()
        Log.d("AEXT", "videoListParse $finalUrl · modo ${apiMode()} · métodos ${strategies.map { it.id }} · HTML ${html.length} bytes")

        for (strategy in strategies) {
            val candidates = try {
                strategy.resolve(ctx)
            } catch (e: Exception) {
                val message = "${strategy.id}: ${e.message ?: "error"}"
                Log.e("AEXT", "resolutor $message", e)
                errors.add(message)
                recordStrategy(strategy.id, false)
                null
            }
            if (candidates == null) {
                Log.d("AEXT", "${strategy.id}: no aplicable a esta página")
                continue // estrategia no aplicable a este sitio
            }
            if (candidates.isEmpty()) {
                recordStrategy(strategy.id, false)
                errors.add("${strategy.id}: sin candidatos")
                Log.e("AEXT", "${strategy.id}: sin candidatos")
                continue
            }
            val verified = try {
                verifyCandidates(candidates)
            } catch (e: Exception) {
                val message = "${strategy.id}: ${e.message ?: "sin stream válido"}"
                Log.e("AEXT", "verificación $message", e)
                errors.add(message)
                recordStrategy(strategy.id, false)
                continue
            }
            recordStrategy(strategy.id, true)
            Log.d("AEXT", "${strategy.id}: ${verified.size} streams verificados")
            return verified
        }
        val mode = apiMode()
        val detail = if (errors.isEmpty()) "ningún método aplicable a esta página" else errors.take(4).joinToString(" | ")
        val message = if (mode == "auto") {
            "AEXT no pudo resolver ningún stream ($detail)"
        } else {
            "AEXT no pudo resolver ningún stream con el modo ${apiModeLabel(mode)} ($detail). Cambia la API preferida a Automático en los ajustes de la extensión."
        }
        Log.e("AEXT", message)
        throw Exception(message)
    }

    private fun htmlEmbedVideos(ctx: VideoContext): List<Video> {
        val html = ctx.html
        val document = Jsoup.parse(html, ctx.url)
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
            val absolute = absolutize(ctx.url, src)
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

    // ---- Estrategia de sesión: boot/start -> stitcher HLS (Pluto, ref. 2026-11-18) ----
    // El reproductor web no expone un .m3u8 en el HTML: ejecuta boot/start,
    // obtiene sessionToken + stitcherParams y construye el máster con jwt.
    // Las variantes heredan el jwt en la query y la clave AES-128 es pública,
    // así que el máster completo (con la pista de audio) funciona en MPV.
    private data class PlutoSession(val stitcher: String, val params: String, val token: String)

    private fun plutoContentId(ctx: VideoContext): Pair<String, String>? {
        // La web sirve películas en `/movies/<id>/` (plural) y episodios en
        // `/shows/<show>/episode/<id>/`. El regex anterior sólo aceptaba
        // `/movie/` y descartaba las películas antes de llamar a boot/start.
        val fromUrl = Regex("""(episodes?|movies?)/([0-9a-f]{24})""", RegexOption.IGNORE_CASE)
            .find(ctx.url)?.let {
                val raw = it.groupValues[1].lowercase()
                (if (raw.startsWith("movie")) "movie" else "episode") to it.groupValues[2]
            }
        if (fromUrl != null) return fromUrl

        // El estado de Next.js/RSC serializa contentId tanto con comillas
        // normales como escapadas (`\"contentId\"`). Normalizamos antes de leer.
        val flat = ctx.html.replace("\\\"", "\"")
        val id = Regex("contentId\\s*[:=]\\s*[\"']([0-9a-f]{24})[\"']", RegexOption.IGNORE_CASE)
            .find(flat)?.groupValues?.get(1)
            ?: Regex("data-content-id\\s*=\\s*[\"']([0-9a-f]{24})[\"']", RegexOption.IGNORE_CASE)
                .find(flat)?.groupValues?.get(1)
            ?: return null
        val declaredMovie = Regex("\"contentType\"\\s*:\\s*\"(movie|episode)\"", RegexOption.IGNORE_CASE)
            .find(flat)?.groupValues?.get(1)?.equals("movie", ignoreCase = true) == true
        val kind = if (declaredMovie || ctx.url.contains("/movie", ignoreCase = true)) "movie" else "episode"
        return kind to id
    }

    private fun plutoJsonField(body: String, name: String): String? =
        Regex("\"$name\"\\s*:\\s*\"([^\"]+)\"").find(body)?.groupValues?.get(1)

    private fun plutoBootSession(): PlutoSession {
        val url = "https://boot.pluto.tv/v4/start".toHttpUrl().newBuilder().apply {
            addQueryParameter("appName", "web")
            addQueryParameter("appVersion", siteAppVersion)
            addQueryParameter("deviceVersion", "124.0.0.0")
            addQueryParameter("deviceModel", "web")
            addQueryParameter("deviceMake", "chrome")
            addQueryParameter("deviceType", "web")
            addQueryParameter("clientID", java.util.UUID.randomUUID().toString())
            addQueryParameter("clientModelNumber", "1.0.0")
            addQueryParameter("serverSideAds", "false")
            addQueryParameter("clientTime", System.currentTimeMillis().toString())
        }.build()
        val h = headers.newBuilder()
            .set("User-Agent", plutoBrowserUa)
            .set("Referer", "https://pluto.tv/")
            .set("Accept", "application/json")
            .build()
        plutoFreshClient.newCall(Request.Builder().url(url).headers(h).build()).execute().use { r ->
            if (!r.isSuccessful) {
                val hint = if (r.code == 403) " (región o sesión rechazadas)" else ""
                throw Exception("Pluto boot/start HTTP ${r.code}$hint")
            }
            val body = r.body?.string().orEmpty()
            if (body.contains("BOT_SESSION_BLOCKED", ignoreCase = true)) {
                throw Exception("Pluto boot/start bloqueó la sesión del reproductor")
            }
            Log.d("AEXT", "Pluto boot/start OK (${body.length} bytes)")
            val token = plutoJsonField(body, "sessionToken")
                ?: throw Exception("Pluto boot/start sin sessionToken")
            val params = plutoJsonField(body, "stitcherParams")
                ?: throw Exception("Pluto boot/start sin stitcherParams")
            val stitcher = plutoJsonField(body, "stitcher")
                ?: "https://service-stitcher.clusters.pluto.tv"
            return PlutoSession(
                stitcher.trimEnd('/'),
                params.replace("\\u0026", "&").replace("\\/", "/").replace("\\u003d", "="),
                token,
            )
        }
    }

    /** Región resuelta por el propio JWT de boot/start (no por query params). */
    private fun plutoSessionRegion(token: String): String? {
        val parts = token.split(".")
        if (parts.size < 2) return null
        return try {
            val payload = String(
                android.util.Base64.decode(
                    parts[1], android.util.Base64.URL_SAFE or android.util.Base64.NO_PADDING
                ),
                Charsets.UTF_8,
            )
            Regex("\"country\"\\s*:\\s*\"([^\"]+)\"").find(payload)?.groupValues?.get(1)
                ?: Regex("\"activeRegion\"\\s*:\\s*\"([^\"]+)\"").find(payload)?.groupValues?.get(1)
        } catch (_: Exception) {
            null
        }
    }

    private fun apiSessionVideos(ctx: VideoContext): List<Video>? {
        if (!ctx.url.contains("pluto.tv", ignoreCase = true)) return null
        val (kind, id) = plutoContentId(ctx)
            ?: throw Exception("No se pudo extraer el contentId de Pluto TV de ${ctx.url.substringBefore('?')} ni del estado del sitio")
        Log.d("AEXT", "Pluto contentId=$kind/$id")
        val session = plutoBootSession()
        val segment = if (kind == "movie") "movie" else "episode"
        val master = "${session.stitcher}/v2/stitch/hls/$segment/$id/master.m3u8" +
            "?${session.params}&jwt=${Uri.encode(session.token)}" +
            "&includeExtendedEvents=true&masterJWTPassthrough=true"
        // La región depende de la IP: la leemos del token, no de un ajuste falso.
        val tokenRegion = plutoSessionRegion(session.token)
        if (tokenRegion != null && siteRegion.isNotBlank() && !tokenRegion.equals(siteRegion, ignoreCase = true)) {
            Log.w("AEXT", "región del token ($tokenRegion) distinta de la detectada ($siteRegion): el catálogo y las pistas pueden cambiar")
        }
        val region = tokenRegion ?: siteRegion.takeIf { it.isNotBlank() }
        val locale = siteLocale.takeIf { it.isNotBlank() }
        val context = listOfNotNull(region, locale).joinToString(" · ")
        val label = if (context.isBlank()) "Pluto TV" else "Pluto TV ($context)"
        return listOf(Video(master, label, "HLS", browserHeaders))
    }

    // ---- Estrategia de respaldo: GraphQL legado con hashes persistentes ----
    // Las páginas de episodio no contienen .m3u8 ni iframe: el stream se
    // resuelve con GraphQL. PtvStart entrega un JWT y la cookie CBS_COM;
    // StreamingUrl (con esa cookie) entrega la ruta del máster HLS; el máster
    // se descarga con Authorization: Bearer <jwt>. Las variantes ya llevan el
    // jwt en la query, así que el reproductor no necesita reenviar el header.
    private val plutoGql = "https://pluto.tv/api/tn/app-shell/graphql/"
    private val plutoPlayout = "https://ipv4.pluto.tv/api/tn/video/playout/"
    private val plutoHashPtvStart = "1f49cc7143a332451387d2145bcbddca135fdc970042f748d3e423e484ec4ecc"
    private val plutoHashStreaming = "d2210e84a51ed4a4382c15720a7366ee955d6354ee76039f58a34ecf274134b7"

    // Pluto bloquea los User-Agent de okhttp con BOT_SESSION_BLOCKED. Forzamos
    // cabeceras de navegador en cada petición GraphQL (el client base podría
    // sobrescribir el UA) y una variante reforzada para los reintentos.
    private val plutoBrowserUa = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36"

    // network.cloudflareClient puede arrastrar un CookieJar con un CBS_COM de
    // sesiones anteriores. Si Pluto ve la cookie en el request, no la reemite
    // en la respuesta y no podemos leerla. Con un cliente sin cookies cada
    // PtvStart devuelve una cookie fresca en x-external-set-cookie.
    private val plutoFreshClient: OkHttpClient by lazy {
        client.newBuilder().cookieJar(okhttp3.CookieJar.NO_COOKIES).build()
    }

    private fun plutoGqlGet(
        operation: String,
        hash: String,
        variables: String,
        source: String,
        cookie: String? = null,
        browser: Boolean = false,
        cacheBust: Boolean = false,
    ): Response {
        val url = plutoGql.toHttpUrl().newBuilder().apply {
            addQueryParameter("operationName", operation)
            addQueryParameter("variables", variables)
            addQueryParameter("extensions", "{\"tnPersistedDocumentHash\":\"$hash\"}")
            if (cacheBust) addQueryParameter("_cb", System.currentTimeMillis().toString())
        }.build()
        val h = headers.newBuilder()
            .set("User-Agent", plutoBrowserUa)
            .set("Referer", "https://pluto.tv/")
            .set("Accept", "application/json")
            .set("Accept-Language", "es-MX,es;q=0.9,en;q=0.8")
            .add("apollo-require-preflight", "true")
            .add("x-apollo-operation-name", operation)
            .add("request-source", source)
        if (browser) {
            h.set("Origin", "https://pluto.tv")
                .set("Sec-Fetch-Dest", "empty")
                .set("Sec-Fetch-Mode", "cors")
                .set("Sec-Fetch-Site", "same-origin")
                .set("Cache-Control", "no-cache")
        }
        if (cookie != null) h.add("Cookie", cookie)
        return plutoFreshClient.newCall(Request.Builder().url(url).headers(h.build()).get().build()).execute()
    }

    /**
     * CBS_COM no siempre viene en x-external-set-cookie: algunos nodos CDN
     * sirven el body con el JWT desde caché y omiten esa cabecera. La buscamos
     * también en el resto de cookies y en el propio JSON.
     */
    private fun plutoCbsCookie(resp: Response, body: String): String? {
        val re = Regex("""CBS_COM=([^;,"'\s]+)""")
        fun extract(value: String): String? = re.find(value)?.groupValues?.get(1)
        resp.header("x-external-set-cookie")?.let { extract(it) }?.let { return it }
        for (value in resp.headers("Set-Cookie")) extract(value)?.let { return it }
        for (i in 0 until resp.headers.size) {
            val name = resp.headers.name(i)
            if (name.contains("cookie", ignoreCase = true)) {
                extract(resp.headers.value(i))?.let { return it }
            }
        }
        // Si el cliente ya envió CBS_COM en el request, reutilizamos ese valor.
        resp.request.header("Cookie")?.let { extract(it) }?.let { return it }
        return extract(body)
    }

    /** La home de Pluto entrega CBS_COM en un Set-Cookie estándar. */
    private fun plutoCbsFromHome(): String? {
        var found: String? = null
        try {
            val h = headers.newBuilder().set("Cache-Control", "no-cache").build()
            plutoFreshClient.newCall(Request.Builder().url("https://pluto.tv/").headers(h).build())
                .execute().use { r ->
                    for (value in r.headers("Set-Cookie")) {
                        Regex("""CBS_COM=([^;]+)""").find(value)?.groupValues?.get(1)?.let { found = it }
                    }
                }
        } catch (_: Exception) {
        }
        return found
    }

    /** PtvStart con reintento ante BOT_SESSION_BLOCKED o cookie ausente. */
    private fun plutoPtvStart(): Pair<String, String> {
        var result: Pair<String, String>? = null
        var lastError = ""
        for (attempt in 0 until 2) {
            plutoGqlGet(
                "PtvStart", plutoHashPtvStart, "{\"params\":{}}", "ptvStart",
                browser = attempt == 1, cacheBust = attempt == 1
            ).use { r ->
                val body = r.body?.string().orEmpty()
                if (!r.isSuccessful) {
                    lastError = "Pluto PtvStart HTTP ${r.code}"
                } else if (body.contains("BOT_SESSION_BLOCKED")) {
                    lastError = "Pluto PtvStart bloqueó la sesión (BOT_SESSION_BLOCKED)"
                } else {
                    val token = Regex("\"jwt\"\\s*:\\s*\"([^\"]+)\"").find(body)?.groupValues?.get(1)
                    if (token == null) {
                        lastError = "Pluto PtvStart no devolvió JWT"
                    } else {
                        val cbs = plutoCbsCookie(r, body) ?: plutoCbsFromHome()
                        if (cbs == null) {
                            // Texto nuevo: si aparece, confirma que se ejecuta
                            // esta versión y que agotamos todas las fuentes.
                            lastError = "Pluto PtvStart sin CBS_COM tras reintento (JWT recibido)"
                        } else {
                            result = token to "CBS_COM=$cbs"
                        }
                    }
                }
            }
            if (result != null) break
        }
        return result ?: throw Exception(lastError.ifBlank { "Pluto PtvStart no devolvió JWT ni CBS_COM" })
    }

    private fun plutoGraphqlVideos(ctx: VideoContext): List<Video>? {
        if (!ctx.url.contains("pluto.tv", ignoreCase = true)) return null
        val (_, contentId) = plutoContentId(ctx)
            ?: throw Exception("No se pudo extraer el contentId de Pluto TV para GraphQL")

        // 1) PtvStart -> JWT + CBS_COM (reintento y extracción robusta).
        val (jwt, cbsCookie) = plutoPtvStart()

        // 2) StreamingUrl con la cookie CBS_COM -> ruta del máster.
        val path = plutoGqlGet(
            "StreamingUrl", plutoHashStreaming,
            "{\"params\":{\"appVersion\":\"$siteAppVersion\",\"contentId\":\"$contentId\",\"contentType\":\"vod\",\"drm\":\"\",\"drmLevel\":\"\",\"streamType\":\"stitcher\"}}",
            "streaming-url-request", cbsCookie
        ).use { resp ->
            if (!resp.isSuccessful) throw Exception("Pluto StreamingUrl HTTP ${resp.code}")
            val body = resp.body?.string().orEmpty()
            if (!Regex("\"success\"\\s*:\\s*true").containsMatchIn(body))
                throw Exception("Pluto StreamingUrl devolvió success=false")
            Regex("\"path\"\\s*:\\s*\"([^\"]+)\"").find(body)?.groupValues?.get(1)
                ?: throw Exception("Pluto StreamingUrl sin stitcherPaths")
        }

        // 3) Máster HLS completo. El máster admite ?jwt= en la query, así que
        //    no hace falta enviar Authorization al reproductor. MPV gestiona
        //    las variantes y, sobre todo, la pista de audio (#EXT-X-MEDIA),
        //    que se perdía si sólo le pasábamos una variante suelta.
        val masterUrl = plutoPlayout + path.trimStart('/')
        val masterVideoUrl = "$masterUrl?jwt=${Uri.encode(jwt)}"
        return listOf(Video(masterVideoUrl, "Pluto TV", "HLS", headers))
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

    /** Sin evidencia explícita NO se etiqueta nada: mejor silencio que un DUB falso. */
    private fun detectAudio(html: String, url: String): String {
        val idx = html.indexOf(url)
        if (idx <= 0) return ""
        val before = html.substring(0, idx).uppercase()
        val dubAt = maxOf(
            before.lastIndexOf("DUB"),
            before.lastIndexOf("LAT"),
            before.lastIndexOf("CAST"),
            before.lastIndexOf("AUDIO LATINO")
        )
        val subAt = before.lastIndexOf("SUB")
        return when {
            dubAt >= 0 && dubAt > subAt -> "DUB"
            subAt >= 0 -> "SUB"
            else -> ""
        }
    }

    private fun resolveVideos(server: String, url: String, audio: String, baseHeaders: Headers): List<Video> {
        val prefix = if (audio.isBlank()) "" else "[$audio] "
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
    let site_cap_formats = analysis.capabilities.stream_formats.iter()
        .map(|f| format!("\"{}\"", f))
        .collect::<Vec<_>>()
        .join(", ");
    // boot/start admite cualquier versión, pero usar la que el sitio declara
    // hace que la sesión generada sea indistinguible de la del reproductor web.
    let site_app_version = analysis.capabilities.app_version
        .as_deref()
        .filter(|v| !v.is_empty())
        .unwrap_or("10.12.1096");
    let site_player_cases = analysis.capabilities.player_cases.iter()
        .map(|c| format!("\"{}\"", c))
        .collect::<Vec<_>>()
        .join(", ");
    let site_region = analysis.capabilities.region.as_deref().unwrap_or("");
    let site_locale = analysis.capabilities.locale.as_deref().unwrap_or("");

    // SUB/DUB sólo se ofrece si AEXT abrió un máster real y encontró pistas.
    let (show_audio, sub_ok, dub_ok) = verified_audio_selection(&analysis.capabilities);
    let mut audio_labels: Vec<&str> = vec![i18n.pref_audio_entries[0]];
    let mut audio_values: Vec<&str> = vec!["all"];
    if sub_ok {
        audio_labels.push(i18n.pref_audio_entries[1]);
        audio_values.push("sub");
    }
    if dub_ok {
        audio_labels.push(i18n.pref_audio_entries[2]);
        audio_values.push("dub");
    }
    let pref_audio_title = i18n.pref_audio_title;
    let pref_audio_entries = audio_labels.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ");
    let pref_audio_values = audio_values.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ");
    let pref_audio_summary = i18n.pref_audio_summary;
    let audio_pref_block = if show_audio {
        format!(
            "        val audioPref = ListPreference(screen.context).apply {{\n            key = \"preferred_audio\"\n            title = \"{pref_audio_title}\"\n            entries = arrayOf({pref_audio_entries})\n            entryValues = arrayOf({pref_audio_values})\n            setDefaultValue(\"all\")\n            summary = \"{pref_audio_summary} — %s\"\n        }}\n        screen.addPreference(audioPref)\n\n"
        )
    } else {
        String::new()
    };
    let search_audio_block = if show_audio {
        format!(
            r#"        val audioFilter = filters.filterIsInstance<{class_name}Filters.AudioFilter>().firstOrNull()
        val isDubSelected = audioFilter?.let {{ it.vals[it.state].second == "dub" }} ?: false

        val finalQuery = when {{
            isDubSelected && cleanQuery.isNotBlank() -> "$cleanQuery latino"
            isDubSelected -> "latino"
            else -> cleanQuery
        }}
"#,
            class_name = class_name
        )
    } else {
        "        val finalQuery = cleanQuery\n".to_string()
    };

    let pref_api_title = i18n.pref_api_title;
    let pref_api_entries = i18n.pref_api_entries.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ");
    let methods = if analysis.capabilities.playback_methods.is_empty() {
        SiteAnalyzer::playback_methods(&analysis.capabilities)
    } else {
        analysis.capabilities.playback_methods.clone()
    };
    let methods_title = i18n.methods_title;
    let methods_summary = methods.join(", ");
    let escape_kt = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let methods_entries = methods
        .iter()
        .map(|m| format!("\"{}\"", escape_kt(m)))
        .collect::<Vec<_>>()
        .join(", ");
    let pref_server_title = i18n.pref_server_title;
    let pref_server_summary = i18n.pref_server_summary;
    let pref_quality_title = i18n.pref_quality_title;
    let pref_server_blacklist_title = i18n.pref_server_blacklist_title;
    let pref_server_blacklist_summary = i18n.pref_server_blacklist_summary;
    let pref_genre_blacklist_title = i18n.pref_genre_blacklist_title;
    let pref_genre_blacklist_summary = i18n.pref_genre_blacklist_summary;

    format!(r#"package {pkg_prefix}.{pkg_lang}.{pkg_slug}

import android.app.Application
import android.content.SharedPreferences
import android.net.Uri
import android.util.Log
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
{search_audio_block}
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
            isEnabled = false
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
"#, video_engine = VIDEO_ENGINE_KT, site_cap_formats = site_cap_formats, site_app_version = site_app_version, site_player_cases = site_player_cases, site_region = site_region, site_locale = site_locale, search_audio_block = search_audio_block, audio_pref_block = audio_pref_block, pref_api_title = pref_api_title, pref_api_entries = pref_api_entries, methods_title = methods_title, methods_summary = methods_summary, methods_entries = methods_entries, pref_server_summary = pref_server_summary)
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

fn generate_kotlin_filters(
    class_name: &str,
    pkg_slug: &str,
    lang: &str,
    pkg_lang: &str,
    pkg_prefix: &str,
    caps: &SiteCapabilities,
) -> String {
    let i18n = get_i18n(lang);
    let filter_header = i18n.filter_header;
    let filter_genre_title = i18n.filter_genre_title;
    let (show_audio, sub_ok, dub_ok) = verified_audio_selection(caps);
    let mut audio_labels: Vec<&str> = vec![i18n.filter_audio_entries[0]];
    let mut audio_vals: Vec<(&str, &str)> = vec![("all", "")];
    if sub_ok {
        audio_labels.push(i18n.filter_audio_entries[1]);
        audio_vals.push(("sub", "sub"));
    }
    if dub_ok {
        audio_labels.push(i18n.filter_audio_entries[2]);
        audio_vals.push(("dub", "dub"));
    }
    let audio_entries = audio_labels.iter().map(|s| format!("\"{}\"", s)).collect::<Vec<_>>().join(", ");
    let audio_vals_kt = audio_vals
        .iter()
        .map(|(k, v)| format!("Pair(\"{}\", \"{}\")", k, v))
        .collect::<Vec<_>>()
        .join(",\n            ");
    let audio_filter_class = if show_audio {
        let filter_audio_title = i18n.filter_audio_title;
        format!(
            r#"    class AudioFilter : AnimeFilter.Select<String>(
        "{filter_audio_title}",
        arrayOf({audio_entries})
    ) {{
        val vals = arrayOf(
            {audio_vals_kt}
        )
    }}

"#
        )
    } else {
        String::new()
    };
    let audio_filter_item = if show_audio {
        "        AudioFilter(),\n".to_string()
    } else {
        String::new()
    };

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

{audio_filter_class}    val DEFAULT_GENRES = listOf(
{genres_list}
    )

    fun getFilterList(): AnimeFilterList = AnimeFilterList(
        Header("{filter_header}"),
{audio_filter_item}        GenreFilter("{filter_genre_title}", DEFAULT_GENRES)
    )
}}
"#, audio_filter_class = audio_filter_class, audio_filter_item = audio_filter_item)
}

fn generate_android_manifest(pkg_id: &str, class_name: &str, app_label: &str, ext_type: &str) -> String {
    let tag = if ext_type == "manga" {
        "tachiyomi.extension"
    } else {
        "tachiyomi.animeextension"
    };

    format!(r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">

    <!-- Marcador usado por Nekoyomi/Aniyomi para distinguir extensiones de apps normales. -->
    <uses-feature android:name="{tag}" />

    <application
        android:label="{app_label}"
        android:icon="@mipmap/ic_launcher"
        android:allowBackup="true"
        android:supportsRtl="true">
        <meta-data
            android:name="{tag}"
            android:value="true" />
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

#[cfg(test)]
mod manifest_tests {
    use super::generate_android_manifest;

    #[test]
    fn anime_manifest_declares_extension_marker() {
        let manifest = generate_android_manifest(
            "eu.kanade.tachiyomi.animeextension.es.pluto",
            "Pluto",
            "Pluto",
            "anime",
        );

        assert!(manifest.contains(r#"<uses-feature android:name="tachiyomi.animeextension" />"#));
        assert!(manifest.contains(r#"android:name="tachiyomi.animeextension""#));
        assert!(manifest.contains(r#"android:value="true""#));
        assert!(manifest.contains(
            r#"android:value="eu.kanade.tachiyomi.animeextension.es.pluto.Pluto""#
        ));
    }

    #[test]
    fn manga_manifest_declares_extension_marker() {
        let manifest = generate_android_manifest(
            "eu.kanade.tachiyomi.extension.es.manga",
            "Manga",
            "Manga",
            "manga",
        );

        assert!(manifest.contains(r#"<uses-feature android:name="tachiyomi.extension" />"#));
        assert!(manifest.contains(r#"android:name="tachiyomi.extension""#));
    }
}

#[cfg(test)]
mod video_engine_tests {
    use super::{generate_kotlin_filters, generate_kotlin_source, VIDEO_ENGINE_KT};
    use crate::engine::analyzer::{AnalysisResult, SiteCapabilities};

    fn pluto_fixture() -> AnalysisResult {
        AnalysisResult {
            url: "https://pluto.tv".to_string(),
            entry_url: "https://pluto.tv/latam/movies/693ace640c94b06078d2a33f/".to_string(),
            name: "Pluto TV".to_string(),
            lang: "es".to_string(),
            ext_type: "anime".to_string(),
            cms: "generic".to_string(),
            favicon_url: None,
            detected_hosts: vec!["pluto.tv".to_string()],
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
                app_version: Some("10.12.1096".to_string()),
                region: Some("MX".to_string()),
                locale: Some("es-419".to_string()),
                player_cases: vec!["session-bootstrap".to_string()],
                ..Default::default()
            },
        }
    }

    #[test]
    fn ptv_start_is_resilient_to_bot_block_and_missing_cookie() {
        // El UA de okhttp provoca BOT_SESSION_BLOCKED y algunos CDN omiten
        // x-external-set-cookie; el generador debe contemplar ambos casos.
        for marker in [
            "plutoBrowserUa",
            "BOT_SESSION_BLOCKED",
            "plutoCbsCookie",
            "resp.headers(\"Set-Cookie\")",
            "plutoCbsFromHome",
            "plutoPtvStart()",
            "cacheBust",
            "plutoFreshClient",
            "CookieJar.NO_COOKIES",
            "resp.request.header(\"Cookie\")",
            "sin CBS_COM tras reintento",
            "masterVideoUrl = \"$masterUrl?jwt=${Uri.encode(jwt)}\"",
        ] {
            assert!(
                VIDEO_ENGINE_KT.contains(marker),
                "falta el marcador de resiliencia en VIDEO_ENGINE_KT: {}",
                marker
            );
        }
        assert!(VIDEO_ENGINE_KT.contains("val (jwt, cbsCookie) = plutoPtvStart()"),
            "la estrategia GraphQL debe usar el helper resistente, no una llamada directa");
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
            "VideoStrategy(\"api-session\", strategyPriority(\"api-session\"))",
            "VideoStrategy(\"api-graphql\", strategyPriority(\"api-graphql\"))",
            "VideoStrategy(\"html-embeds\", strategyPriority(\"html-embeds\"))",
            "(ok + 1.0) / (ok + fail + 2.0)",
            "thenBy { it.priority }",
            "sitePlayerCases.contains(\"session-bootstrap\")",
            "sitePlayerCases.contains(\"graphql-persisted\")",
            "sitePlayerCases.contains(\"iframe-player\")",
        ] {
            assert!(
                VIDEO_ENGINE_KT.contains(marker),
                "falta el pipeline de estrategias en VIDEO_ENGINE_KT: {}",
                marker
            );
        }
    }

    #[test]
    fn api_mode_really_limits_the_resolvers() {
        for marker in [
            "private fun apiMode(): String",
            "private fun apiModeLabel(mode: String): String",
            "\"session\" -> all.filter { it.id == \"api-session\" }",
            "\"graphql\" -> all.filter { it.id == \"api-graphql\" }",
            "\"html\" -> all.filter { it.id == \"html-embeds\" }",
            "los ajustes de la extensión",
        ] {
            assert!(
                VIDEO_ENGINE_KT.contains(marker),
                "la preferencia de API debe cambiar la ruta real, no sólo ordenar: {}",
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
            "episodes?|movies?",
            "ctx.html.replace(\"\\\\\\\"\", \"\\\"\")",
            "siteRegion.takeIf { it.isNotBlank() }",
            "siteLocale.takeIf { it.isNotBlank() }",
        ] {
            assert!(
                VIDEO_ENGINE_KT.contains(marker),
                "falta detección honesta de idiomas/región/contentId: {}",
                marker
            );
        }
    }

    #[test]
    fn generated_extension_exposes_api_mode_and_detected_region() {
        let kt = generate_kotlin_source(
            &pluto_fixture(), "Pluto TV", "PlutoTv", "pluto", "es", "es",
            "eu.kanade.tachiyomi.animeextension",
        );
        for marker in [
            "key = \"api_mode\"",
            "entryValues = arrayOf(\"auto\", \"session\", \"graphql\", \"html\")",
            "private val siteRegion = \"MX\"",
            "private val siteLocale = \"es-419\"",
            "private val siteAppVersion = \"10.12.1096\"",
            "title = \"API preferida\"",
            "Métodos disponibles encontrados de reproducción",
            "summary = \"Sesión web",
        ] {
            assert!(
                kt.contains(marker),
                "el Kotlin generado debe exponer el modo API y la región detectada: {}",
                marker
            );
        }
    }

    #[test]
    fn session_strategy_builds_the_full_stitcher_master() {
        for marker in [
            "private data class PlutoSession",
            "https://boot.pluto.tv/v4/start",
            "addQueryParameter(\"appVersion\", siteAppVersion)",
            "plutoJsonField(body, \"sessionToken\")",
            "plutoJsonField(body, \"stitcherParams\")",
            "includeExtendedEvents=true&masterJWTPassthrough=true",
            "Video(master, label, \"HLS\", browserHeaders)",
            "private fun plutoSessionRegion(token: String): String?",
            "private fun apiSessionVideos(ctx: VideoContext): List<Video>?",
            "private fun plutoGraphqlVideos(ctx: VideoContext): List<Video>?",
            "Jsoup.parse(html, ctx.url)",
        ] {
            assert!(
                VIDEO_ENGINE_KT.contains(marker),
                "falta la estrategia de sesión en VIDEO_ENGINE_KT: {}",
                marker
            );
        }
    }

    #[test]
    fn sub_dub_ui_is_only_generated_when_a_real_master_verifies_tracks() {
        let unverified = pluto_fixture();
        let kt = generate_kotlin_source(
            &unverified, "Pluto TV", "PlutoTv", "pluto", "es", "es",
            "eu.kanade.tachiyomi.animeextension",
        );
        let filters = generate_kotlin_filters(
            "PlutoTv", "pluto", "es", "es", "eu.kanade.tachiyomi.animeextension",
            &unverified.capabilities,
        );
        assert!(!kt.contains("key = \"preferred_audio\""), "sin verificación no se ofrece SUB/DUB");
        assert!(!kt.contains("AudioFilter"), "sin verificación no se genera el filtro de audio");
        assert!(!filters.contains("AudioFilter"), "sin verificación no se genera AudioFilter");
        assert!(kt.contains("val finalQuery = cleanQuery"), "la búsqueda no debe inventar 'latino'");

        let mut verified = pluto_fixture();
        verified.capabilities.master_verified = true;
        verified.capabilities.sub_verified = true;
        let kt = generate_kotlin_source(
            &verified, "Pluto TV", "PlutoTv", "pluto", "es", "es",
            "eu.kanade.tachiyomi.animeextension",
        );
        let filters = generate_kotlin_filters(
            "PlutoTv", "pluto", "es", "es", "eu.kanade.tachiyomi.animeextension",
            &verified.capabilities,
        );
        assert!(kt.contains("key = \"preferred_audio\""), "con SUB verificado sí se ofrece el selector");
        assert!(filters.contains("class AudioFilter"), "con SUB verificado sí se genera el filtro");
        assert!(!kt.contains("entryValues = arrayOf(\"all\", \"sub\", \"dub\")"), "DUB no verificado no se ofrece");
        assert!(kt.contains("entryValues = arrayOf(\"all\", \"sub\")"), "sólo la opción verificada");
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

    // El lint de release intenta resolver dependencias transitivas muertas del
    // extensions-lib antiguo (injekt-core de JitPack) y rompe la compilación.
    // Las extensiones no necesitan lint vital; la compilación Kotlin basta.
    lint {{
        checkReleaseBuilds = false
    }}

    compileOptions {{
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }}

    kotlin {{
        compilerOptions {{
            jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_1_8
            freeCompilerArgs.add("-opt-in=kotlinx.serialization.ExperimentalSerializationApi")
        }}
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

// AGP 9 lleva Kotlin integrado (el plugin `kotlin("android")` ya no existe) y
// funciona con las JDK modernas que traen las versiones nuevas de Android
// Studio (JBR 25/27). Con AGP 8.2.2 + Gradle 8.5, esas mismas JDK fallan con
// un error críptico que solo muestra el número de versión ("25.0.3").
pub const ROOT_BUILD_GRADLE: &str = r#"plugins {
    id("com.android.application") version "9.2.1" apply false
    id("org.jetbrains.kotlin.plugin.serialization") version "2.3.21" apply false
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
  # No cancelar runs: cada commit de AEXT debe compilarse y verificarse. Con
  # `true`, los commits rápidos cancelaban el run útil y no había generación.
  cancel-in-progress: false

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

      - name: Set up JDK 21
        uses: actions/setup-java@v5
        with:
          java-version: '21'
          distribution: 'temurin'

      - name: Setup Gradle
        uses: gradle/actions/setup-gradle@v3
        with:
          gradle-version: 9.5.1

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

/// README mínimo y estable para repositorios creados por AEXT.
///
/// No incluye el nombre de la extensión: el mismo contenido sirve para un repo
/// con una o con veinte extensiones y no cambia en cada generación. Si el repo
/// ya tiene un README propio, AEXT lo respeta y no usa esta plantilla.
pub fn generate_repo_readme(owner: &str, repo: &str, html_url: &str) -> String {
    let install_url = format!(
        "https://raw.githubusercontent.com/{}/{}/gh-pages/index.min.json",
        owner, repo
    );
    format!(
        r#"# Extensiones Nekoyomi

Repositorio de extensiones para Nekoyomi / Aniyomi.

## Instalación

Añade esta URL como repositorio de extensiones en Nekoyomi o Aniyomi:

{install_url}

## Información

- Repositorio: {html_url}
- El índice de extensiones se publica en la rama `gh-pages` mediante GitHub Actions.

## Descargo de responsabilidad

Las extensiones se distribuyen «tal cual», sin garantía de funcionamiento. El contenido de los sitios web de terceros es responsabilidad de sus respectivos propietarios.

## Licencia

El código de este repositorio se distribuye bajo licencia MIT salvo que se indique lo contrario en un archivo `LICENSE`.
"#
    )
}

pub fn decode_base64(input: &str) -> Vec<u8> {
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

// Lanzador mínimo del wrapper: fuerza la versión de Gradle declarada en
// gradle-wrapper.properties para que no dependa de la Gradle del PATH.
pub const GRADLEW_SH: &str = r#"#!/bin/sh
set -e
APP_HOME=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if [ -n "$JAVA_HOME" ]; then
  JAVA_EXE="$JAVA_HOME/bin/java"
else
  JAVA_EXE=java
fi
exec "$JAVA_EXE" -classpath "$APP_HOME/gradle/wrapper/gradle-wrapper.jar" org.gradle.wrapper.GradleWrapperMain "$@"
"#;

pub const GRADLEW_BAT: &str = r#"@echo off
setlocal
set APP_HOME=%~dp0
if defined JAVA_HOME (
  set JAVA_EXE=%JAVA_HOME%\bin\java.exe
) else (
  set JAVA_EXE=java.exe
)
"%JAVA_EXE%" -classpath "%APP_HOME%gradle\wrapper\gradle-wrapper.jar" org.gradle.wrapper.GradleWrapperMain %*
exit /b %ERRORLEVEL%
"#;

pub const GRADLE_WRAPPER_PROPERTIES: &str = r#"distributionBase=GRADLE_USER_HOME
distributionPath=wrapper/dists
distributionUrl=https\://services.gradle.org/distributions/gradle-9.5.1-bin.zip
networkTimeout=10000
validateDistributionUrl=true
zipStoreBase=GRADLE_USER_HOME
zipStorePath=wrapper/dists
"#;

pub const GRADLE_WRAPPER_JAR_B64: &str = "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L2dyYWRsZS9jbGkvQ29tbWFuZExpbmVQYXJzZXIkQWZ0ZXJPcHRpb25zLmNsYXNzVVQFAAEAAAAAUEsBAhQAFAAICAgAAAAhAB9e8LctAwAAXQcAADwACQAAAAAAAAAAAAAA1hUAAG9yZy9ncmFkbGUvY2xpL0NvbW1hbmRMaW5lUGFyc2VyJEJlZm9yZUZpcnN0U3ViQ29tbWFuZC5jbGFzc1VUBQABAAAAAFBLAQIUABQACAgIAAAAIQBYgpgP2AYAAGIOAAA9AAkAAAAAAAAAAAAAAHYZAABvcmcvZ3JhZGxlL2NsaS9Db21tYW5kTGluZVBhcnNlciRLbm93bk9wdGlvblBhcnNlclN0YXRlLmNsYXNzVVQFAAEAAAAAUEsBAhQAFAAICAgAAAAhANImcdNMAgAAlwQAADwACQAAAAAAAAAAAAAAwiAAAG9yZy9ncmFkbGUvY2xpL0NvbW1hbmRMaW5lUGFyc2VyJE1pc3NpbmdPcHRpb25BcmdTdGF0ZS5jbGFzc1VUBQABAAAAAFBLAQIUABQACAgIAAAAIQAvy2DF1wIAAEoFAAA9AAkAAAAAAAAAAAAAAIEjAABvcmcvZ3JhZGxlL2NsaS9Db21tYW5kTGluZVBhcnNlciRPcHRpb25Bd2FyZVBhcnNlclN0YXRlLmNsYXNzVVQFAAEAAAAAUEsBAhQAFAAICAgAAAAhAAb4QlWkAQAAfQIAADgACQAAAAAAAAAAAAAAzCYAAG9yZy9ncmFkbGUvY2xpL0NvbW1hbmRMaW5lUGFyc2VyJE9wdGlvblBhcnNlclN0YXRlLmNsYXNzVVQFAAEAAAAAUEsBAhQAFAAICAgAAAAhAIETaMkNAgAAQwMAADMACQAAAAAAAAAAAAAA3ygAAG9yZy9ncmFkbGUvY2xpL0NvbW1hbmRMaW5lUGFyc2VyJE9wdGlvblN0cmluZy5jbGFzc1VUBQABAAAAAFBLAQIUABQACAgIAAAAIQDZoNqjpwEAAM4CAAAyAAkAAAAAAAAAAAAAAFYrAABvcmcvZ3JhZGxlL2NsaS9Db21tYW5kTGluZVBhcnNlciRQYXJzZXJTdGF0ZS5jbGFzc1VUBQABAAAAAFBLAQIUABQACAgIAAAAIQAAjGddcwIAAMcEAAA/AAkAAAAAAAAAAAAAAGYtAABvcmcvZ3JhZGxlL2NsaS9Db21tYW5kTGluZVBhcnNlciRVbmtub3duT3B0aW9uUGFyc2VyU3RhdGUuY2xhc3NVVAUAAQAAAABQSwECFAAUAAgICAAAACEAxbgfPrEEAABjCAAAJgAJAAAAAAAAAAAAAABPMAAAb3JnL2dyYWRsZS9jbGkvQ29tbWFuZExpbmVQYXJzZXIuY2xhc3NVVAUAAQAAAABQSwECFAAUAAgICAAAACEA829HuTsEAADhBwAAJgAJAAAAAAAAAAAAAABdNQAAb3JnL2dyYWRsZS9jbGkvUGFyc2VkQ29tbWFuZExpbmUuY2xhc3NVVAUAAQAAAABQSwECFAAUAAgICAAAACEAyioNQVMBAACsAQAALAAJAAAAAAAAAAAAAAD1OQAAb3JnL2dyYWRsZS9jbGkvUGFyc2VkQ29tbWFuZExpbmVPcHRpb24uY2xhc3NVVAUAAQAAAABQSwECFAAUAAgICAAAACEA2JZpfgEDAACcBAAAMwAJAAAAAAAAAAAAAACrOwAAb3JnL2dyYWRsZS9pbnRlcm5hbC9maWxlL1BhdGhUcmF2ZXJzYWxDaGVja2VyLmNsYXNzVVQFAAEAAAAAUEsBAhQAFAAICAgAAAAhAFAafz3CAQAAowIAAD4ACQAAAAAAAAAAAAAAFj8AAG9yZy9ncmFkbGUvdXRpbC9pbnRlcm5hbC9XcmFwcGVyRGlzdHJpYnV0aW9uVXJsQ29udmVydGVyLmNsYXNzVVQFAAEAAAAAUEsBAhQAFAAICAgAAAAhAM8OjbmPAQAAHgIAAC8ACQAAAAAAAAAAAAAATUEAAG9yZy9ncmFkbGUvd3JhcHBlci9Cb290c3RyYXBNYWluU3RhcnRlciQxLmNsYXNzVVQFAAEAAAAAUEsBAhQAFAAICAgAAAAhACZHXY8pAwAA5AQAAEEACQAAAAAAAAAAAAAAQkMAAG9yZy9ncmFkbGUvd3JhcHBlci9Eb3dubG9hZCREZWZhdWx0RG93bmxvYWRQcm9ncmVzc0xpc3RlbmVyLmNsYXNzVVQFAAEAAAAAUEsBAhQAFAAICAgAAAAhAFSneKkfAwAAowUAADQACQAAAAAAAAAAAAAA40YAAG9yZy9ncmFkbGUvd3JhcHBlci9Eb3dubG9hZCRQcm94eUF1dGhlbnRpY2F0b3IuY2xhc3NVVAUAAQAAAABQSwECFAAUAAgICAAAACEAjLvDsGYJAAA1EgAAIQAJAAAAAAAAAAAAAABtSgAAb3JnL2dyYWRsZS93cmFwcGVyL0Rvd25sb2FkLmNsYXNzVVQFAAEAAAAAUEsBAhQAFAAICAgAAAAhAPGMLh9/AQAA9QEAADMACQAAAAAAAAAAAAAAK1QAAG9yZy9ncmFkbGUvd3JhcHBlci9FeGNsdXNpdmVGaWxlQWNjZXNzTWFuYWdlci5jbGFzc1VUBQABAAAAAFBLAQIUABQACAgIAAAAIQALfn7P2QEAALICAAAtAAkAAAAAAAAAAAAAABRWAABvcmcvZ3JhZGxlL3dyYXBwZXIvR3JhZGxlVXNlckhvbWVMb29rdXAuY2xhc3NVVAUAAQAAAABQSwECFAAUAAgICAAAACEAq+86IP8UAACNKQAAKgAJAAAAAAAAAAAAAABRWAAAb3JnL2dyYWRsZS93cmFwcGVyL0dyYWRsZVdyYXBwZXJNYWluLmNsYXNzVVQFAAEAAAAAUEsBAhQAFAAICAgAAAAhADThlEzDCwAAuhUAACIACQAAAAAAAAAAAAAAsW0AAG9yZy9ncmFkbGUvd3JhcHBlci9JbnN0YWxsJDEuY2xhc3NVVAUAAQAAAABQSwECFAAUAAgICAAAACEAkyTtAGoBAADnAQAALQAJAAAAAAAAAAAAAADNeQAAb3JnL2dyYWRsZS93cmFwcGVyL0luc3RhbGwkSW5zdGFsbENoZWNrLmNsYXNzVVQFAAEAAAAAUEsBAhQAFAAICAgAAAAhAEtDILafDgAAvhsAACAACQAAAAAAAAAAAAAAm3sAAG9yZy9ncmFkbGUvd3JhcHBlci9JbnN0YWxsLmNsYXNzVVQFAAEAAAAAUEsBAhQAFAAICAgAAAAhAOqbsNk8AgAAHgQAAB8ACQAAAAAAAAAAAAAAkYoAAG9yZy9ncmFkbGUvd3JhcHBlci9Mb2dnZXIuY2xhc3NVVAUAAQAAAABQSwECFAAUAAgICAAAACEAp18O2iQBAABqAQAAJgAJAAAAAAAAAAAAAAAjjQAAb3JnL2dyYWRsZS93cmFwcGVyL1BhdGhBc3NlbWJsZXIuY2xhc3NVVAUAAQAAAABQSwECFAAUAAgICAAAACEAeqxO8JwDAABOBgAAMAAJAAAAAAAAAAAAAACkjgAAb3JnL2dyYWRsZS93cmFwcGVyL1N5c3RlbVByb3BlcnRpZXNIYW5kbGVyLmNsYXNzVVQFAAEAAAAAUEsBAhQAFAAICAgAAAAhAGni1S3vAgAAUAYAAC0ACQAAAAAAAAAAAAAAp5IAAG9yZy9ncmFkbGUvd3JhcHBlci9XcmFwcGVyQ29uZmlndXJhdGlvbi5jbGFzc1VUBQABAAAAAFBLAQIUABQACAgIAAAAIQCuENrgUwYAAMUMAAAoAAkAAAAAAAAAAAAAAPqVAABvcmcvZ3JhZGxlL3dyYXBwZXIvV3JhcHBlckV4ZWN1dG9yLmNsYXNzVVQFAAEAAAAAUEsFBgAAAAAhACEABA0AAKycAAAAAA==";

const KEYSTORE_B64: &str = "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";
