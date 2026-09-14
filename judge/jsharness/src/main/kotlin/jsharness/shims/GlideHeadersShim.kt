// 差分垫片:help/glide/GlideHeaders.kt(真身实现 Glide 的 Headers 接口)
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help.glide

import com.bumptech.glide.load.model.Headers

class GlideHeaders(private val headers: MutableMap<String, String>) : Headers {
    override fun getHeaders(): Map<String, String> = headers
}
