// 差分垫片:AnalyzeUrl.getGlideUrl() 的类型面(封面加载,差分不经过)。
@file:Suppress("unused", "UNUSED_PARAMETER", "PackageDirectoryMismatch")

package com.bumptech.glide.load.model

interface Headers {
    fun getHeaders(): Map<String, String>
}

class GlideUrl(val url: String, val headers: Headers)
