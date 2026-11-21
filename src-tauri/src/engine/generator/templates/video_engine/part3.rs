/// Parte 3 de la plantilla del motor de vídeo Kotlin (emisión de código).

macro_rules! video_engine_part3 {
    () => { r##"            if (isBlacklisted(blacklist, server, embedUrl)) continue

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
        return listOf(Video(ctx.url, label, master, browserHeaders))
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
"## };
}
pub(crate) use video_engine_part3;
