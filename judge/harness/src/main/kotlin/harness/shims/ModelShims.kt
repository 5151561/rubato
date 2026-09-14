// 差分垫片:model/ 下被挂载文件触到的对象。
// Debug 真身绑 UI 会话/时间戳,此处保 log/callback 面;其余为身份桩。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.model

import com.script.ScriptBindings
import io.legado.app.data.entities.BaseBook
import io.legado.app.data.entities.Book
import io.legado.app.data.entities.BookChapter
import io.legado.app.data.entities.BookSource
import kotlin.coroutines.CoroutineContext

object Debug {
    var callback: Callback? = null

    fun log(
        sourceUrl: String?,
        msg: String = "",
        print: Boolean = true,
        isHtml: Boolean = false,
        showTime: Boolean = true,
        state: Int = 1
    ) = Unit

    fun log(msg: String?) = Unit

    interface Callback {
        fun printLog(state: Int, msg: String)
    }
}

// model/ReadBook.kt:实体 delete() 触到;差分无阅读会话
object ReadBook {
    var book: Book? = null
}

// model/SharedJsScope.kt:桩不共享作用域
object SharedJsScope {
    fun getScope(jsLib: String?, coroutineContext: CoroutineContext? = null): ScriptBindings? = null
    fun getCryptoScope(owner: Any, coroutineContext: CoroutineContext?): ScriptBindings? = null
    fun remove(jsLib: String?) = Unit
}

// model/AudioPlay.kt:BookSource.kt 的 import 触到(死引用);身份桩
object AudioPlay

// model/jsSource/JsSourceBook.kt:mainJs 书源流程,差分不覆盖 → 显式报错,
// 保证 case 不会静默走进未对齐的路径
object JsSourceBook {
    private fun unsupported(): Nothing =
        throw io.legado.app.exception.NoStackTraceException("差分不支持 mainJs 书源")

    suspend fun searchAwait(
        bookSource: BookSource,
        key: String,
        page: Int?,
        filter: ((name: String, author: String) -> Boolean)? = null,
    ): ArrayList<io.legado.app.data.entities.SearchBook> = unsupported()

    suspend fun exploreAwait(
        bookSource: BookSource,
        url: String,
        page: Int?,
    ): List<io.legado.app.data.entities.SearchBook> = unsupported()

    suspend fun getBookInfoAwait(
        bookSource: BookSource,
        book: Book,
        canReName: Boolean = true,
    ): Book = unsupported()

    suspend fun getChapterListAwait(
        bookSource: BookSource,
        book: Book,
    ): List<BookChapter> = unsupported()

    suspend fun getContentAwait(
        bookSource: BookSource,
        book: Book,
        bookChapter: BookChapter,
        nextChapterUrl: String? = null,
    ): String = unsupported()
}
