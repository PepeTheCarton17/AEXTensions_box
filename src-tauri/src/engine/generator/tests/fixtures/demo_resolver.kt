package aext.demo

import eu.kanade.tachiyomi.aext.AextStreamResolver
import eu.kanade.tachiyomi.aext.AextVideoContext
import eu.kanade.tachiyomi.animesource.model.Video

/**
 * Motor de prueba neutro: demuestra que el bundle compila un resolutor
 * externo sin WebView y sin ninguna API o dominio concreto.
 */
class DemoResolver : AextStreamResolver {
    override val id: String = "demo"

    override fun canResolve(ctx: AextVideoContext): Boolean =
        ctx.html.contains("aext-demo-marker", ignoreCase = true)

    override fun resolve(ctx: AextVideoContext): List<Video> = emptyList()
}
