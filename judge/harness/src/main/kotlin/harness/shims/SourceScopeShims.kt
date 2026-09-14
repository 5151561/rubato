@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help.source

import com.script.ScriptBindings
import com.script.SharedGlobalStateHandle
import io.legado.app.constant.BookSourceType
import io.legado.app.constant.BookType
import io.legado.app.data.entities.BaseSource
import io.legado.app.data.entities.BookSource
import io.legado.app.data.entities.RssSource
import io.legado.app.utils.MD5Utils
import kotlin.coroutines.CoroutineContext

// 桩:不共享顶层作用域,evalJS 每次都走独立 scope 分支
fun BaseSource.getShareScope(coroutineContext: CoroutineContext? = null): ScriptBindings? = null

// 真身:jsLib 非空时给共享全局态一个稳定键;桩保键身份、不共享状态
fun BaseSource.getSharedGlobalStateKey(): SharedGlobalStateHandle? {
    val library = jsLib?.takeIf { it.isNotBlank() } ?: return null
    val key = "${MD5Utils.md5Encode(library)}:${javaClass.name}:${MD5Utils.md5Encode(getKey())}"
    return ScriptBindings.getSharedGlobalStateHandle(key)
}

fun BaseSource.clearSharedGlobalState() {
    getSharedGlobalStateKey()?.let(ScriptBindings::removeSharedGlobalState)
}

// help/source/BookSourceExtensions.kt 被 webBook 触到的两个函数
// (真身文件带 ACache/LruCache;getBookType 逐字复刻 LegadoTeam 版,
// exploreKindsJson 只在 Debug 会话时被走到,差分恒空)
fun BookSource.getBookType(): Int {
    return when (bookSourceType) {
        BookSourceType.file -> BookType.text or BookType.webFile
        BookSourceType.image -> BookType.image
        BookSourceType.audio -> BookType.audio
        BookSourceType.video -> BookType.video
        else -> BookType.text
    }
}

fun BookSource.exploreKindsJson(): String = ""

suspend fun BookSource.clearExploreKindsCache() = Unit

// help/source/SourceType 判定(真身 BaseSourceExtensions.kt)
fun BaseSource.getSourceType(): Int {
    return when (this) {
        is BookSource -> io.legado.app.constant.SourceType.book
        is RssSource -> io.legado.app.constant.SourceType.rss
        else -> error("unknown source type: ${this::class.simpleName}.")
    }
}
