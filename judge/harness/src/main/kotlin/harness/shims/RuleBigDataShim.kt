// 差分垫片:help/RuleBigDataHelp.kt 真身经 Room + 文件目录持久化;
// 差分里换成内存表,语义面(put/get/has 三类变量)一致。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help

object RuleBigDataHelp {
    private val bookVars = HashMap<Pair<String, String>, String>()
    private val chapterVars = HashMap<Triple<String, String, String>, String>()
    private val rssVars = HashMap<Triple<String, String, String>, String>()

    fun putBookVariable(bookUrl: String, key: String, value: String?) {
        if (value == null) bookVars.remove(bookUrl to key) else bookVars[bookUrl to key] = value
    }

    fun getBookVariable(bookUrl: String, key: String?): String? =
        key?.let { bookVars[bookUrl to it] }

    fun hasBookVariable(bookUrl: String, key: String): Boolean =
        bookVars.containsKey(bookUrl to key)

    fun putChapterVariable(bookUrl: String, chapterUrl: String, key: String, value: String?) {
        val k = Triple(bookUrl, chapterUrl, key)
        if (value == null) chapterVars.remove(k) else chapterVars[k] = value
    }

    fun getChapterVariable(bookUrl: String, chapterUrl: String, key: String): String? =
        chapterVars[Triple(bookUrl, chapterUrl, key)]

    fun putRssVariable(origin: String, link: String, key: String, value: String?) {
        val k = Triple(origin, link, key)
        if (value == null) rssVars.remove(k) else rssVars[k] = value
    }

    fun getRssVariable(origin: String, link: String, key: String): String? =
        rssVars[Triple(origin, link, key)]

    // harness 差分观察面
    fun clear() {
        bookVars.clear(); chapterVars.clear(); rssVars.clear()
    }
}
