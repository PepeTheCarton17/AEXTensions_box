/// Parte 1 de la plantilla del motor de vídeo Kotlin (emisión de código).

macro_rules! video_engine_part1 {
    () => { r##"    // ============================== VIDEO LIST & EXTRACTORS ==============================

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
                body.contains("session blocked", ignoreCase = true)
                    || body.contains("bot session", ignoreCase = true) -> "sesión bloqueada por el sitio"
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
    /** Reconstruye un Video con el constructor público de 4 argumentos. Evita
     *  `copy`/`copy$default`: esa función sintética no existe en todas las
     *  versiones de la librería de extensiones y provoca un NoSuchMethodError
     *  en tiempo de ejecución antes de mostrar las calidades. */
    private fun rebuildVideo(video: Video, quality: String, headers: Headers?): Video {
        return Video(video.url, quality, video.videoUrl, headers)
    }

    /** Etiqueta con idiomas REALES del HLS. Si no hay pistas declaradas, no añade nada. */
    private fun labelWithTracks(video: Video, probe: StreamProbe): Video {
        val parts = mutableListOf<String>()
        if (probe.audioLangs.isNotEmpty()) parts.add("audio " + probe.audioLangs.joinToString("/"))
        if (probe.subLangs.isNotEmpty()) parts.add("subs " + probe.subLangs.joinToString("/"))
        if (probe.inconclusive) parts.add("sin verificar")
        if (parts.isEmpty()) return video
        val labeled = video.quality.orEmpty() + " · " + parts.joinToString(" · ")
        return rebuildVideo(video, labeled, video.headers)
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
                val labeled = labelWithTracks(video, browser)
                val chosen = if (browser.ok) {
                    rebuildVideo(labeled, labeled.quality.orEmpty(), browserHeaders)
                } else {
                    labeled
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
    // Un sitio puede exponer el stream detrás de una API privada o como
    // embebido en HTML. AEXT no asume una sola ruta: prueba varias en orden y
    // aprende cuál funciona: guarda aciertos/fallos por estrategia y ordena
    // las siguientes reproducciones por probabilidad de éxito (suavizado de
    // Laplace). Así, si el sitio cambia de API, la estrategia que empieza a
    // fallar baja de puesto automáticamente sin regenerar la extensión.
    private data class VideoContext(val url: String, val html: String)

"## };
}
pub(crate) use video_engine_part1;
