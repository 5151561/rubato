// pipeline-corpus-b 的裁判侧:**WebBook 四步真身 × 真 Rhino**。
//
// 与 :harness 的同名 op(fixtures/cases/pipeline{,-corpus})逐字同一张投影 ——
// 差别只有一处,而那一处正是这套存在的理由:那边的 com.script 是确定性桩,
// 这边是真 Rhino。于是 `<js>` / `@js:` 的 B 层书源才第一次真的跑起来。
//
// 契约 fixtures/cases/pipeline-corpus-b/README.md;
// 输出格式与 fixtures/cases/pipeline/README.md 完全一致(多一个 `fallback` 输入字段)。
package jsharness

import com.google.gson.JsonArray
import com.google.gson.JsonObject
import io.legado.app.utils.GSON
import io.legado.app.utils.fromJsonObject

private fun varsJson(variable: String?): com.google.gson.JsonElement {
    variable ?: return com.google.gson.JsonNull.INSTANCE
    val m: HashMap<String, String> = GSON
        .fromJsonObject<HashMap<String, String>>(variable).getOrNull()
        ?: return com.google.gson.JsonPrimitive(variable)
    val out = JsonObject()
    java.util.TreeMap(m).forEach { (k, v) -> out.addProperty(k, v) }
    return out
}

private fun searchBookJson(b: io.legado.app.data.entities.SearchBook): JsonObject =
    JsonObject().apply {
        addProperty("name", b.name)
        addProperty("author", b.author)
        addProperty("kind", b.kind)
        addProperty("coverUrl", b.coverUrl)
        addProperty("intro", b.intro)
        addProperty("wordCount", b.wordCount)
        addProperty("latestChapterTitle", b.latestChapterTitle)
        addProperty("bookUrl", b.bookUrl)
        addProperty("origin", b.origin)
        addProperty("type", b.type)
        add("vars", varsJson(b.variable))
        addProperty("infoHtml", b.infoHtml)
    }

private fun bookJson(b: io.legado.app.data.entities.Book): JsonObject =
    JsonObject().apply {
        addProperty("name", b.name)
        addProperty("author", b.author)
        addProperty("kind", b.kind)
        addProperty("wordCount", b.wordCount)
        addProperty("latestChapterTitle", b.latestChapterTitle)
        addProperty("intro", b.intro)
        addProperty("coverUrl", b.coverUrl)
        addProperty("tocUrl", b.tocUrl)
        addProperty("bookUrl", b.bookUrl)
        addProperty("type", b.type)
        addProperty("durChapterTitle", b.durChapterTitle)
        addProperty("totalChapterNum", b.totalChapterNum)
        add("vars", varsJson(b.variable))
        addProperty("tocHtml", b.tocHtml)
        addProperty("infoHtml", b.infoHtml)
    }

private fun chapterJson(ch: io.legado.app.data.entities.BookChapter): JsonObject =
    JsonObject().apply {
        addProperty("title", ch.title)
        addProperty("url", ch.url)
        addProperty("tag", ch.tag)
        addProperty("wordCount", ch.wordCount)
        addProperty("isVolume", ch.isVolume)
        addProperty("isVip", ch.isVip)
        addProperty("isPay", ch.isPay)
        addProperty("index", ch.index)
        add("vars", varsJson(ch.variable))
    }

private fun bookFromJson(j: JsonObject?): io.legado.app.data.entities.Book {
    val b = io.legado.app.data.entities.Book()
    j ?: return b
    j["bookUrl"]?.takeIf { !it.isJsonNull }?.let { b.bookUrl = it.asString }
    j["name"]?.takeIf { !it.isJsonNull }?.let { b.name = it.asString }
    j["author"]?.takeIf { !it.isJsonNull }?.let { b.author = it.asString }
    j["tocUrl"]?.takeIf { !it.isJsonNull }?.let { b.tocUrl = it.asString }
    j["infoHtml"]?.takeIf { !it.isJsonNull }?.let { b.infoHtml = it.asString }
    j["tocHtml"]?.takeIf { !it.isJsonNull }?.let { b.tocHtml = it.asString }
    j["variable"]?.takeIf { !it.isJsonNull }?.let { b.variable = it.asString }
    j["type"]?.takeIf { !it.isJsonNull }?.let { b.type = it.asInt }
    j["durChapterIndex"]?.takeIf { !it.isJsonNull }?.let { b.durChapterIndex = it.asInt }
    j["totalChapterNum"]?.takeIf { !it.isJsonNull }?.let { b.totalChapterNum = it.asInt }
    return b
}

/** 四步。`o` 已带 id;抛出去的异常由调用方 [runCase] 归一。 */
fun runPipeline(c: JsonObject, o: JsonObject): JsonObject {
    val source = try {
        GSON.fromJson(
            c["source"].asString,
            io.legado.app.data.entities.BookSource::class.java
        ) ?: throw NullPointerException()
    } catch (e: Throwable) {
        o.addProperty("error", "source_error")
        return o
    }

    try {
        val web = io.legado.app.model.webBook.WebBook
        when (c["step"].asString) {
            "search" -> {
                val books = kotlinx.coroutines.runBlocking {
                    web.searchBookAwait(source, c["key"]?.asString ?: "", c["page"]?.asInt)
                }
                val arr = JsonArray()
                books.forEach { arr.add(searchBookJson(it)) }
                o.add("books", arr)
            }
            "explore" -> {
                val books = kotlinx.coroutines.runBlocking {
                    web.exploreBookAwait(source, c["url"].asString, c["page"]?.asInt)
                }
                val arr = JsonArray()
                books.forEach { arr.add(searchBookJson(it)) }
                o.add("books", arr)
            }
            "info" -> {
                val book = bookFromJson(c["book"]?.asJsonObject)
                kotlinx.coroutines.runBlocking {
                    web.getBookInfoAwait(source, book, c["canReName"]?.asBoolean ?: true)
                }
                o.add("book", bookJson(book))
            }
            "toc" -> {
                val book = bookFromJson(c["book"]?.asJsonObject)
                val chapters = kotlinx.coroutines.runBlocking {
                    web.getChapterListAwait(source, book).getOrThrow()
                }
                val arr = JsonArray()
                chapters.forEach { arr.add(chapterJson(it)) }
                o.add("chapters", arr)
                o.add("book", bookJson(book))
            }
            "content" -> {
                val book = bookFromJson(c["book"]?.asJsonObject)
                val ch = io.legado.app.data.entities.BookChapter(bookUrl = book.bookUrl)
                c["chapter"]?.asJsonObject?.let { j ->
                    j["url"]?.takeIf { !it.isJsonNull }?.let { ch.url = it.asString }
                    j["title"]?.takeIf { !it.isJsonNull }?.let { ch.title = it.asString }
                    j["baseUrl"]?.takeIf { !it.isJsonNull }?.let { ch.baseUrl = it.asString }
                    j["index"]?.takeIf { !it.isJsonNull }?.let { ch.index = it.asInt }
                    j["isVolume"]?.takeIf { !it.isJsonNull }?.let { ch.isVolume = it.asBoolean }
                    j["tag"]?.takeIf { !it.isJsonNull }?.let { ch.tag = it.asString }
                }
                val content = kotlinx.coroutines.runBlocking {
                    web.getContentAwait(
                        source, book, ch, c["nextChapterUrl"]?.asString, needSave = false,
                    )
                }
                o.addProperty("content", content)
                o.add("chapter", chapterJson(ch))
            }
            else -> {
                o.addProperty("error", "unknown_step")
                return o
            }
        }
        val reqs = JsonArray()
        ReplayHttp.hops.forEach { (m, u) ->
            val e = JsonArray()
            e.add(m)
            e.add(u)
            reqs.add(e)
        }
        o.add("requests", reqs)
        val db = JsonObject()
        io.legado.app.data.appDb.cookieDao.all().forEach { (k, v) -> db.addProperty(k, v) }
        o.add("cookiesDb", db)
    } catch (e: Throwable) {
        // 归一与 :harness 的同名 op 逐字一致:snapshot_miss / too_many_redirects /
        // toc_empty / content_empty 单列,其余一律 `pipeline_error`(粗粒度 ——
        // 两侧只要求「抛了」,这一档的一致不等于同因,见 README)。
        // **真因打 stderr**(被测侧 PIPELINE_DEBUG=1 的对侧):归一之后
        // `pipeline_error` 一个标签盖住几十种因,不打就没法定位。
        if (System.getenv("JSHARNESS_DEBUG") == "1") {
            System.err.println("[pipeline] ${o["id"]?.asString}: ${e.javaClass.name}: ${e.message}")
        }
        var cause: Throwable? = e
        var msg = "pipeline_error"
        while (cause != null) {
            val m = cause.message ?: ""
            when {
                m.startsWith("snapshot_miss:") || m == "too_many_redirects" -> {
                    msg = m
                    break
                }
                cause is io.legado.app.exception.TocEmptyException -> {
                    msg = "toc_empty"
                    break
                }
                cause is io.legado.app.exception.ContentEmptyException -> {
                    msg = "content_empty"
                    break
                }
            }
            cause = cause.cause
        }
        val keys = o.keySet().toList()
        keys.forEach { if (it != "id") o.remove(it) }
        o.addProperty("error", msg)
    }
    return o
}
