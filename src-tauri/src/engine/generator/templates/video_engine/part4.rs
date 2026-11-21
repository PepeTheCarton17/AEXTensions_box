/// Parte 4 de la plantilla del motor de vídeo Kotlin (emisión de código).

macro_rules! video_engine_part4 {
    () => { r##"            val h = headers.newBuilder().set("Cache-Control", "no-cache").build()
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
        return listOf(Video(ctx.url, "Pluto TV", masterVideoUrl, headers))
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
"## };
}
pub(crate) use video_engine_part4;
