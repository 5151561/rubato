// 差分垫片(与 judge/harness/shims/JsSourceShims.kt 同一份):mainJs 纯 JS 书源(LegadoTeam 新增)在 Rubato 属砍单项
// (docs/plan.md §6:显式报「不支持」,Phase 3 再议)。四步差分用例不含
// isJsSource() 为真的源;真进到这里两侧都以「不支持」出错,契约一致。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.model.jsSource

import io.legado.app.data.entities.Book
import io.legado.app.data.entities.BookChapter
import io.legado.app.data.entities.BookSource
import io.legado.app.data.entities.SearchBook
import io.legado.app.exception.NoStackTraceException

object JsSourceBook {

    private fun unsupported(): Nothing =
        throw NoStackTraceException("不支持JS单文件书源")

    suspend fun searchAwait(
        source: BookSource,
        key: String,
        page: Int? = 1,
        filter: ((name: String, author: String, kind: String?) -> Boolean)? = null,
    ): ArrayList<SearchBook> = unsupported()

    suspend fun exploreAwait(
        source: BookSource,
        url: String,
        page: Int? = 1,
    ): ArrayList<SearchBook> = unsupported()

    suspend fun getBookInfoAwait(
        source: BookSource,
        book: Book,
        canReName: Boolean,
    ): Book = unsupported()

    suspend fun getChapterListAwait(
        source: BookSource,
        book: Book,
    ): Result<List<BookChapter>> = unsupported()

    suspend fun getContentAwait(
        source: BookSource,
        book: Book,
        chapter: BookChapter,
        nextChapterUrl: String? = null,
        needSave: Boolean = true,
    ): String = unsupported()
}
