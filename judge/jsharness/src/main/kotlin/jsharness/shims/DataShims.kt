// 差分垫片:appDb 的 cookie 面(真身是 Room AppDatabase)
@file:Suppress("unused", "UNUSED_PARAMETER", "ClassName")

package io.legado.app.data

import io.legado.app.data.entities.Cookie

class CookieDao {
    private val map = LinkedHashMap<String, String>()
    fun insert(cookie: Cookie) {
        map[cookie.url] = cookie.cookie
    }
    fun get(url: String): Cookie? = map[url]?.let { Cookie(url, it) }
    fun delete(url: String) {
        map.remove(url)
    }
    fun deleteOkHttp() = Unit

    // harness 的差分观察面
    fun all(): Map<String, String> = map.toSortedMap()
    fun clear() = map.clear()
}

class BookChapterDao {
    fun getChapterList(bookUrl: String): List<io.legado.app.data.entities.BookChapter> = emptyList()
    fun getChapter(bookUrl: String, index: Int): io.legado.app.data.entities.BookChapter? = null
    fun update(vararg chapter: io.legado.app.data.entities.BookChapter) = Unit
    fun updateContentMetadata(bookUrl: String, index: Int, title: String, imgUrl: String?) = Unit
}

class BookSourceDao {
    fun getBookSource(key: String): io.legado.app.data.entities.BookSource? = null
    fun getBookSources(keys: Collection<String>): List<io.legado.app.data.entities.BookSource> = emptyList()
}

class BookDao {
    fun has(bookUrl: String): Boolean = false
    // 差分不带书架库,ContentProcessor 的「去重复标题缓存」恒空
    fun getBookByOrigin(name: String, origin: String): io.legado.app.data.entities.Book? = null
    fun insert(vararg book: io.legado.app.data.entities.Book) = Unit
    fun update(vararg book: io.legado.app.data.entities.Book) = Unit
    fun delete(vararg book: io.legado.app.data.entities.Book) = Unit
}

class ReplaceRuleDao {
    fun update(vararg rule: io.legado.app.data.entities.ReplaceRule) = Unit
    // 净化替换规则被砍(docs/plan.md §6),库里恒空
    fun findEnabledByTitleScope(name: String, origin: String): List<io.legado.app.data.entities.ReplaceRule> = emptyList()
    fun findEnabledByContentScope(name: String, origin: String): List<io.legado.app.data.entities.ReplaceRule> = emptyList()
}

object appDb {
    val cookieDao = CookieDao()
    val bookChapterDao = BookChapterDao()
    val bookSourceDao = BookSourceDao()
    val bookDao = BookDao()
    val replaceRuleDao = ReplaceRuleDao()
}
