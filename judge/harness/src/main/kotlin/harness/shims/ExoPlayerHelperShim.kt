// 差分垫片:help/exoplayer/ExoPlayerHelper.kt(真身绑死 ExoPlayer/Context),
// 只复刻 AnalyzeUrl 触到的 createMediaItem。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help.exoplayer

import androidx.media3.common.MediaItem
import com.google.gson.reflect.TypeToken
import io.legado.app.utils.GSON

object ExoPlayerHelper {

    // 真身 ExoPlayerHelper.kt L41 —— 逐字复刻(U+1F6A7 施工标志)
    private const val SPLIT_TAG = "\uD83D\uDEA7"

    private val mapType by lazy {
        object : TypeToken<Map<String, String>>() {}.type
    }

    fun createMediaItem(url: String, headers: Map<String, String>): MediaItem {
        val formatUrl = url + SPLIT_TAG + GSON.toJson(headers, mapType)
        val mediaItemBuilder = MediaItem.Builder().setUri(formatUrl)
        return mediaItemBuilder.build()
    }
}
