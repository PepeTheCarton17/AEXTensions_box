/// Parte 2 de la plantilla del motor de vídeo Kotlin (emisión de código).

macro_rules! video_engine_part2 {
    () => { r##"    private class VideoStrategy(
        val id: String,
        val priority: Int,
        val resolve: (AextVideoContext) -> List<Video>?,
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

    /**
     * Orden inicial de los resolutores. AEXT no decide por dominio: el
     * catálogo real que devuelva cada motor decide después, con aciertos y
     * fallos contados en preferencias.
     */
    private fun strategyPriority(id: String): Int = when {
        id.startsWith("engine:") -> 0
        id == "html-embeds" -> 1
        else -> 1
    }

    private fun orderedStrategies(): List<VideoStrategy> {
        val all = mutableListOf(
            VideoStrategy("html-embeds", strategyPriority("html-embeds")) { ctx -> htmlEmbedVideos(ctx) },
        )
        // Los motores instalados se añaden por su id declarado. Si ningún
        // resolver aplica, devuelve null y el pipeline sigue con el HTML.
        for (resolver in AextEngineRegistry().streamResolvers) {
            val id = "engine:" + resolver.id
            all.add(VideoStrategy(id, strategyPriority(id)) { ctx ->
                if (resolver.canResolve(ctx)) resolver.resolve(ctx) else null
            })
        }
        return all.sortedWith(
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
        val ctx = AextVideoContext(finalUrl, html, emptyList(), emptyList(), client, browserHeaders)
        val errors = mutableListOf<String>()
        val strategies = orderedStrategies()
        Log.d("AEXT", "videoListParse $finalUrl · métodos ${strategies.map { it.id }} · HTML ${html.length} bytes")

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
        val detail = if (errors.isEmpty()) "ningún método aplicable a esta página" else errors.take(4).joinToString(" | ")
        val message = "AEXT no pudo resolver ningún stream ($detail)"
        Log.e("AEXT", message)
        throw Exception(message)
    }

    private fun htmlEmbedVideos(ctx: AextVideoContext): List<Video> {
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
"## };
}
pub(crate) use video_engine_part2;
