package harness

import com.google.gson.GsonBuilder
import com.google.gson.JsonArray
import com.google.gson.JsonObject
import io.legado.app.model.analyzeRule.RuleAnalyzer
import io.legado.app.utils.fromJsonObject
import java.io.File

// fr 回调契约见 fixtures/cases/rule-syntax/README.md,两侧实现必须一致
private fun fr(s: String): String? = when {
    s.contains("NULL") -> null
    s.contains("EMPTY") -> ""
    else -> "«$s»"
}

// 逐行复制自 AnalyzeRule.replaceRegex(judge/engine 冻结源 L482-505,去掉缓存)。
// 引擎本体是 Android library 无法直接挂进纯 JVM 模块,故此处复制并以差分保真。
private fun replaceRegexJudge(
    result: String,
    replaceRegex: String,
    replacement: String,
    replaceFirst: Boolean,
): String {
    if (replaceRegex.isEmpty()) return result
    val regex = try {
        replaceRegex.toRegex()
    } catch (e: Exception) {
        null
    }
    if (replaceFirst) {
        /* ##match##replace### 获取第一个匹配到的结果并进行替换 */
        if (regex != null) kotlin.runCatching {
            val match = regex.find(result)
            return if (match != null) {
                match.value.replaceFirst(regex, replacement)
            } else {
                ""
            }
        }
        return replacement
    } else {
        /* ##match##replace 替换*/
        if (regex != null) kotlin.runCatching {
            return result.replace(regex, replacement)
        }
        return result.replace(replaceRegex, replacement)
    }
}

// defineDoc 注册的页面(供 cssSelect 复用,避免逐 case 内联大 HTML)
private val docs = HashMap<String, org.jsoup.nodes.Document>()
// 原始 HTML 串(jsoupDsl 的 html 动作会变异文档,须每 case 重新解析)
private val docSrcs = HashMap<String, String>()

/** case 的 `webview` 字段 → 剧本(字段缺席 = 缺省剧本) */
private fun parseWvScript(c: JsonObject): WvScript {
    val o = c["webview"]?.takeIf { it.isJsonObject }?.asJsonObject ?: return WvScript()
    fun arr(name: String) = o[name]?.takeIf { it.isJsonArray }?.asJsonArray
    return WvScript(
        overrideUrls = arr("overrideUrls")?.map {
            val e = it.asJsonObject
            WvScript.Override(
                e["url"].asString,
                e["isRedirect"]?.takeIf { v -> !v.isJsonNull }?.asBoolean ?: false,
            )
        } ?: emptyList(),
        loadResources = arr("loadResources")?.map { it.asString } ?: emptyList(),
        pageFinished = o["pageFinished"]?.takeIf { !it.isJsonNull }?.asBoolean ?: true,
        pageFinishedUrl = o["pageFinishedUrl"]?.takeIf { !it.isJsonNull }?.asString,
        evals = arr("evals")?.map { it.asString } ?: emptyList(),
        cookie = o["cookie"]?.takeIf { !it.isJsonNull }?.asString,
        laterPageFinished = arr("laterPageFinished")?.map {
            val e = it.asJsonObject
            WvScript.Later(
                e["at"].asLong,
                e["url"]?.takeIf { v -> !v.isJsonNull }?.asString,
            )
        } ?: emptyList(),
    )
}

private fun runCase(c: JsonObject): JsonObject {
    val o = JsonObject()
    o.addProperty("id", c["id"].asString)
    // §6 回落页:逐 case 设置(不带该字段就是老语义 —— 未命中报 snapshot_miss)
    ReplayHttp.fallback = c["fallback"]?.takeIf { !it.isJsonNull }?.asString
    // webView 的剧本:`{"webView": true}` 的书源与 `@webjs:` 都走它。
    // 不带 `webview` 字段就是缺省剧本(页面加载得完、每次求值都回 "null"
    // —— 于是重试梯子跑满、报「js执行超时」)。契约见
    // fixtures/cases/webview/README.md,被测侧同一份在 difftest::wv_script。
    WvStage.resetCase(parseWvScript(c))
    try {
            when (val op = c["op"].asString) {
                "split" -> {
                    val ra = RuleAnalyzer(c["data"].asString, c["code"]?.asBoolean ?: false)
                    if (c["trim"]?.asBoolean == true) ra.trim()
                    val sep = c["sep"].asJsonArray.map { it.asString }.toTypedArray()
                    val r = ra.splitRule(*sep)
                    val arr = JsonArray()
                    r.forEach { arr.add(it) }
                    o.add("result", arr)
                    o.addProperty("elementsType", ra.elementsType)
                }
                "innerRule" -> {
                    val ra = RuleAnalyzer(c["data"].asString)
                    val r = ra.innerRule(
                        c["inner"].asString,
                        c["startStep"]?.asInt ?: 1,
                        c["endStep"]?.asInt ?: 1,
                        ::fr,
                    )
                    o.addProperty("result", r)
                }
                "innerRuleStr" -> {
                    val ra = RuleAnalyzer(c["data"].asString)
                    val r = ra.innerRule(c["startStr"].asString, c["endStr"].asString, ::fr)
                    o.addProperty("result", r)
                }
                "replaceRegex" -> {
                    val r = replaceRegexJudge(
                        c["data"].asString,
                        c["pattern"].asString,
                        c["replacement"]?.asString ?: "",
                        c["first"]?.asBoolean ?: false,
                    )
                    o.addProperty("result", r)
                }
                "defineDoc" -> {
                    docs[c["name"].asString] = org.jsoup.Jsoup.parse(c["html"].asString)
                    docSrcs[c["name"].asString] = c["html"].asString
                    o.addProperty("result", "ok")
                }
                "jsoupDsl" -> {
                    val html = c["doc"]?.asString?.let { docSrcs[it]!! } ?: c["html"].asString
                    val rule = c["rule"].asString
                    try {
                        val analyzer = io.legado.app.model.analyzeRule.AnalyzeByJSoup(
                            org.jsoup.Jsoup.parse(html)
                        )
                        when (c["mode"].asString) {
                            "string" -> {
                                val r = analyzer.getString(rule)
                                if (r == null) o.add("result", com.google.gson.JsonNull.INSTANCE)
                                else o.addProperty("result", r)
                            }
                            "string0" -> o.addProperty("result", analyzer.getString0(rule))
                            "stringList" -> {
                                val arr = JsonArray()
                                analyzer.getStringList(rule).forEach { arr.add(it) }
                                o.add("result", arr)
                            }
                            "elements" -> {
                                val arr = JsonArray()
                                analyzer.getElements(rule).forEach { arr.add(it.outerHtml()) }
                                o.add("result", arr)
                            }
                            else -> o.addProperty("error", "unknown_mode")
                        }
                    } catch (e: Throwable) {
                        o.remove("result")
                        o.addProperty("error", "dsl_error")
                    }
                }
                "cssSelect" -> {
                    val doc = c["doc"]?.asString?.let { docs[it] }
                        ?: org.jsoup.Jsoup.parse(c["html"].asString)
                    try {
                        val els = doc.select(c["selector"].asString)
                        val action = c["action"].asString
                        val arr = JsonArray()
                        for (el in els) {
                            arr.add(
                                when {
                                    action == "text" -> el.text()
                                    action == "ownText" -> el.ownText()
                                    action == "wholeText" -> el.wholeText()
                                    action == "data" -> el.data()
                                    action == "html" -> el.html()
                                    action == "outerHtml" -> el.outerHtml()
                                    action.startsWith("attr:") -> el.attr(action.substring(5))
                                    else -> ""
                                }
                            )
                        }
                        o.add("result", arr)
                    } catch (e: org.jsoup.select.Selector.SelectorParseException) {
                        o.addProperty("error", "selector_error")
                    } catch (e: IllegalStateException) {
                        o.addProperty("error", "selector_error")
                    }
                }
                "xpathDsl" -> {
                    // AnalyzeByXPath 真身(JsoupXpath 2.5.5)。契约见
                    // fixtures/cases/xpath/README.md。
                    // contentType:
                    //   "string"(默认)—— 走 strToJXDocument(带 </td> </tr> 包裹
                    //                     与 <?xml 走 xmlParser 两个分支);
                    //   "document"    —— 先 Jsoup.parse 再进 Document 分支
                    //                     (JXDocument.create(doc) = doc.children());
                    //   "element"     —— 取该文档 select 到的第一个元素进 Element 分支。
                    val html = c["doc"]?.asString?.let { docSrcs[it]!! } ?: c["html"].asString
                    val rule = c["rule"].asString
                    try {
                        val content: Any = when (c["contentType"]?.asString ?: "string") {
                            "document" -> org.jsoup.Jsoup.parse(html)
                            "element" -> org.jsoup.Jsoup.parse(html)
                                .select(c["at"].asString).first()!!
                            else -> html
                        }
                        val analyzer =
                            io.legado.app.model.analyzeRule.AnalyzeByXPath(content)
                        when (c["mode"].asString) {
                            "string" -> {
                                val r = analyzer.getString(rule)
                                if (r == null) o.add("result", com.google.gson.JsonNull.INSTANCE)
                                else o.addProperty("result", r)
                            }
                            "stringList" -> {
                                val arr = JsonArray()
                                analyzer.getStringList(rule).forEach { arr.add(it) }
                                o.add("result", arr)
                            }
                            "elements" -> {
                                val els = analyzer.getElements(rule)
                                if (els == null) {
                                    o.add("result", com.google.gson.JsonNull.INSTANCE)
                                } else {
                                    val arr = JsonArray()
                                    val kinds = JsonArray()
                                    els.forEach {
                                        arr.add(it.asString())
                                        kinds.add(if (it.isElement) "el" else "str")
                                    }
                                    o.add("result", arr)
                                    o.add("kinds", kinds)
                                }
                            }
                            else -> o.addProperty("error", "unknown_mode")
                        }
                    } catch (e: Throwable) {
                        o.remove("result")
                        o.remove("kinds")
                        o.addProperty("error", "xpath_error")
                        // 归一成一个标签(ANTLR 的报错文案不该逐字比),
                        // 但把真因打到 stderr —— 定位 FAIL 时要看它
                        System.err.println("xpath_error ${c["id"].asString} $rule :: $e")
                    }
                }
                "analyzeUrl" -> {
                    // AnalyzeUrl 的离线面:url 规则 → 一次请求的全部要素。
                    // 私有字段用反射读(engine 不改),JS 走同一套确定性桩。
                    val data: io.legado.app.model.analyzeRule.RuleDataInterface? =
                        when (c["ruleData"]?.asString ?: "plain") {
                            "none" -> null
                            "book" -> io.legado.app.data.entities.Book().apply {
                                name = c["bookName"]?.asString ?: ""
                                author = c["bookAuthor"]?.asString ?: ""
                            }
                            else -> io.legado.app.model.analyzeRule.RuleData()
                        }
                    c["vars"]?.asJsonObject?.entrySet()?.forEach { (k, v) ->
                        data?.putVariable(k, v.asString)
                    }
                    var chapter: io.legado.app.data.entities.BookChapter? = null
                    if (c["chapter"]?.asBoolean == true) {
                        chapter = io.legado.app.data.entities.BookChapter().apply {
                            title = c["title"]?.asString ?: ""
                        }
                        c["chapterVars"]?.asJsonObject?.entrySet()?.forEach { (k, v) ->
                            chapter.putVariable(k, v.asString)
                        }
                    }
                    val headerMapF = c["headers"]?.asJsonObject?.let { obj ->
                        LinkedHashMap<String, String>().apply {
                            obj.entrySet().forEach { (k, v) -> put(k, v.asString) }
                        }
                    }
                    try {
                        val au = io.legado.app.model.analyzeRule.AnalyzeUrl(
                            mUrl = c["mUrl"].asString,
                            key = c["key"]?.asString,
                            page = c["page"]?.asInt,
                            speakText = c["speakText"]?.asString,
                            speakSpeed = c["speakSpeed"]?.asInt,
                            baseUrl = c["baseUrl"]?.asString ?: "",
                            ruleData = data,
                            chapter = chapter,
                            headerMapF = headerMapF,
                        )
                        fun priv(name: String): Any? =
                            io.legado.app.model.analyzeRule.AnalyzeUrl::class.java
                                .getDeclaredField(name)
                                .apply { isAccessible = true }
                                .get(au)
                        o.addProperty("ruleUrl", au.ruleUrl)
                        o.addProperty("url", au.url)
                        o.addProperty("urlNoQuery", au.urlNoQuery)
                        o.addProperty("type", au.type)
                        o.addProperty("method", priv("method").toString())
                        o.addProperty("body", priv("body") as String?)
                        o.addProperty("encodedForm", priv("encodedForm") as String?)
                        o.addProperty("encodedQuery", priv("encodedQuery") as String?)
                        o.addProperty("charset", priv("charset") as String?)
                        o.addProperty("proxy", priv("proxy") as String?)
                        o.addProperty("retry", priv("retry") as Int)
                        o.addProperty("useWebView", priv("useWebView") as Boolean)
                        o.addProperty("webJs", priv("webJs") as String?)
                        o.addProperty("bodyJs", priv("bodyJs") as String?)
                        o.addProperty("dnsIp", priv("dnsIp") as String?)
                        // LegadoTeam 新增的 urlOption 字段(readTimeout/followRedirects)
                        o.addProperty("readTimeoutMs", priv("readTimeoutMs") as Long?)
                        o.addProperty("urlTimeoutConfigured", priv("urlTimeoutConfigured") as Boolean)
                        o.addProperty("followRedirects", priv("followRedirects") as Boolean?)
                        o.addProperty("webViewDelayTime", priv("webViewDelayTime") as Long)
                        o.addProperty("serverID", au.serverID)
                        o.addProperty("userAgent", au.getUserAgent())
                        o.addProperty("isPost", au.isPost())
                        val hm = JsonObject()
                        au.headerMap.forEach { (k, v) -> hm.addProperty(k, v) }
                        o.add("headers", hm)
                        val vars = JsonObject()
                        if (data != null) {
                            val m = JsonObject()
                            java.util.TreeMap(data.variableMap).forEach { (k, v) -> m.addProperty(k, v) }
                            vars.add("data", m)
                        }
                        if (chapter != null) {
                            val m = JsonObject()
                            java.util.TreeMap(chapter.variableMap).forEach { (k, v) -> m.addProperty(k, v) }
                            vars.add("chapter", m)
                        }
                        o.add("vars", vars)
                    } catch (e: Throwable) {
                        val keys = o.keySet().toList()
                        keys.forEach { if (it != "id") o.remove(it) }
                        o.addProperty("error", "url_error")
                    }
                }
                "pipeline" -> {
                    // 流水线级差分:WebBook 四步真身 + HTTP 录放。
                    // 契约见 fixtures/cases/pipeline/README.md
                    io.legado.app.help.CacheManager.clearMemory()
                    io.legado.app.data.appDb.cookieDao.clear()
                    ReplayHttp.newCase()
                    val source = try {
                        io.legado.app.utils.GSON.fromJson(
                            c["source"].asString,
                            io.legado.app.data.entities.BookSource::class.java
                        ) ?: throw NullPointerException()
                    } catch (e: Throwable) {
                        o.addProperty("error", "source_error")
                        return o
                    }

                    fun varsJson(variable: String?): com.google.gson.JsonElement {
                        variable ?: return com.google.gson.JsonNull.INSTANCE
                        val m: HashMap<String, String> = io.legado.app.utils.GSON
                            .fromJsonObject<HashMap<String, String>>(variable).getOrNull()
                            ?: return com.google.gson.JsonPrimitive(variable)
                        val out = JsonObject()
                        java.util.TreeMap(m).forEach { (k, v) -> out.addProperty(k, v) }
                        return out
                    }

                    fun searchBookJson(b: io.legado.app.data.entities.SearchBook): JsonObject =
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

                    fun bookJson(b: io.legado.app.data.entities.Book): JsonObject =
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

                    fun chapterJson(ch: io.legado.app.data.entities.BookChapter): JsonObject =
                        JsonObject().apply {
                            addProperty("title", ch.title)
                            addProperty("url", ch.url)
                            addProperty("tag", ch.tag)
                            // tocCountWords 从章节信息里抽出的字数
                            addProperty("wordCount", ch.wordCount)
                            addProperty("isVolume", ch.isVolume)
                            addProperty("isVip", ch.isVip)
                            addProperty("isPay", ch.isPay)
                            addProperty("index", ch.index)
                            add("vars", varsJson(ch.variable))
                        }

                    fun bookFromJson(j: JsonObject?): io.legado.app.data.entities.Book {
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
                        j["durChapterIndex"]?.takeIf { !it.isJsonNull }
                            ?.let { b.durChapterIndex = it.asInt }
                        j["totalChapterNum"]?.takeIf { !it.isJsonNull }
                            ?.let { b.totalChapterNum = it.asInt }
                        return b
                    }

                    try {
                        val web = io.legado.app.model.webBook.WebBook
                        when (c["step"].asString) {
                            "search" -> {
                                val books = kotlinx.coroutines.runBlocking {
                                    web.searchBookAwait(
                                        source,
                                        c["key"]?.asString ?: "",
                                        c["page"]?.asInt,
                                    )
                                }
                                val arr = JsonArray()
                                books.forEach { arr.add(searchBookJson(it)) }
                                o.add("books", arr)
                            }
                            "explore" -> {
                                val books = kotlinx.coroutines.runBlocking {
                                    web.exploreBookAwait(
                                        source,
                                        c["url"].asString,
                                        c["page"]?.asInt,
                                    )
                                }
                                val arr = JsonArray()
                                books.forEach { arr.add(searchBookJson(it)) }
                                o.add("books", arr)
                            }
                            "info" -> {
                                val book = bookFromJson(c["book"]?.asJsonObject)
                                kotlinx.coroutines.runBlocking {
                                    web.getBookInfoAwait(
                                        source, book, c["canReName"]?.asBoolean ?: true
                                    )
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
                                val ch = io.legado.app.data.entities.BookChapter(
                                    bookUrl = book.bookUrl
                                )
                                c["chapter"]?.asJsonObject?.let { j ->
                                    j["url"]?.takeIf { !it.isJsonNull }?.let { ch.url = it.asString }
                                    j["title"]?.takeIf { !it.isJsonNull }
                                        ?.let { ch.title = it.asString }
                                    j["baseUrl"]?.takeIf { !it.isJsonNull }
                                        ?.let { ch.baseUrl = it.asString }
                                    j["index"]?.takeIf { !it.isJsonNull }
                                        ?.let { ch.index = it.asInt }
                                    j["isVolume"]?.takeIf { !it.isJsonNull }
                                        ?.let { ch.isVolume = it.asBoolean }
                                    j["tag"]?.takeIf { !it.isJsonNull }?.let { ch.tag = it.asString }
                                }
                                val content = kotlinx.coroutines.runBlocking {
                                    web.getContentAwait(
                                        source, book, ch,
                                        c["nextChapterUrl"]?.asString,
                                        needSave = false,
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
                        io.legado.app.data.appDb.cookieDao.all()
                            .forEach { (k, v) -> db.addProperty(k, v) }
                        o.add("cookiesDb", db)
                    } catch (e: Throwable) {
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
                }
                "parseSource" -> {
                    // 书源 JSON → BookSource(真身 GSON 语义:Int 宽容、String 强转、
                    // rule 对象接受「字符串包 JSON」),投影出流水线用到的字段
                    val src = try {
                        io.legado.app.utils.GSON.fromJson(
                            c["json"].asString,
                            io.legado.app.data.entities.BookSource::class.java
                        ) ?: throw NullPointerException("null source")
                    } catch (e: Throwable) {
                        o.addProperty("error", "source_error")
                        return o
                    }
                    fun ruleObj(r: Any?): com.google.gson.JsonElement {
                        if (r == null) return com.google.gson.JsonNull.INSTANCE
                        val m = JsonObject()
                        when (r) {
                            is io.legado.app.data.entities.rule.SearchRule -> {
                                m.addProperty("checkKeyWord", r.checkKeyWord)
                                m.addProperty("bookList", r.bookList)
                                m.addProperty("name", r.name)
                                m.addProperty("author", r.author)
                                m.addProperty("intro", r.intro)
                                m.addProperty("kind", r.kind)
                                m.addProperty("lastChapter", r.lastChapter)
                                m.addProperty("updateTime", r.updateTime)
                                m.addProperty("bookUrl", r.bookUrl)
                                m.addProperty("coverUrl", r.coverUrl)
                                m.addProperty("wordCount", r.wordCount)
                            }
                            is io.legado.app.data.entities.rule.ExploreRule -> {
                                m.addProperty("bookList", r.bookList)
                                m.addProperty("name", r.name)
                                m.addProperty("author", r.author)
                                m.addProperty("intro", r.intro)
                                m.addProperty("kind", r.kind)
                                m.addProperty("lastChapter", r.lastChapter)
                                m.addProperty("updateTime", r.updateTime)
                                m.addProperty("bookUrl", r.bookUrl)
                                m.addProperty("coverUrl", r.coverUrl)
                                m.addProperty("wordCount", r.wordCount)
                            }
                            is io.legado.app.data.entities.rule.BookInfoRule -> {
                                m.addProperty("init", r.init)
                                m.addProperty("name", r.name)
                                m.addProperty("author", r.author)
                                m.addProperty("intro", r.intro)
                                m.addProperty("kind", r.kind)
                                m.addProperty("lastChapter", r.lastChapter)
                                m.addProperty("updateTime", r.updateTime)
                                m.addProperty("coverUrl", r.coverUrl)
                                m.addProperty("tocUrl", r.tocUrl)
                                m.addProperty("wordCount", r.wordCount)
                                m.addProperty("canReName", r.canReName)
                                m.addProperty("downloadUrls", r.downloadUrls)
                            }
                            is io.legado.app.data.entities.rule.TocRule -> {
                                m.addProperty("preUpdateJs", r.preUpdateJs)
                                m.addProperty("chapterList", r.chapterList)
                                m.addProperty("chapterName", r.chapterName)
                                m.addProperty("chapterUrl", r.chapterUrl)
                                m.addProperty("formatJs", r.formatJs)
                                m.addProperty("isVolume", r.isVolume)
                                m.addProperty("isVip", r.isVip)
                                m.addProperty("isPay", r.isPay)
                                m.addProperty("updateTime", r.updateTime)
                                m.addProperty("nextTocUrl", r.nextTocUrl)
                            }
                            is io.legado.app.data.entities.rule.ContentRule -> {
                                m.addProperty("content", r.content)
                                m.addProperty("subContent", r.subContent)
                                m.addProperty("title", r.title)
                                m.addProperty("nextContentUrl", r.nextContentUrl)
                                m.addProperty("webJs", r.webJs)
                                m.addProperty("sourceRegex", r.sourceRegex)
                                m.addProperty("replaceRegex", r.replaceRegex)
                                m.addProperty("imageStyle", r.imageStyle)
                                m.addProperty("imageDecode", r.imageDecode)
                                m.addProperty("payAction", r.payAction)
                                m.addProperty("callBackJs", r.callBackJs)
                            }
                        }
                        return m
                    }
                    o.addProperty("bookSourceUrl", src.bookSourceUrl)
                    o.addProperty("bookSourceName", src.bookSourceName)
                    o.addProperty("bookSourceGroup", src.bookSourceGroup)
                    o.addProperty("bookSourceType", src.bookSourceType)
                    o.addProperty("bookUrlPattern", src.bookUrlPattern)
                    o.addProperty("customOrder", src.customOrder)
                    o.addProperty("enabled", src.enabled)
                    o.addProperty("enabledExplore", src.enabledExplore)
                    o.addProperty("jsLib", src.jsLib)
                    o.addProperty("enabledCookieJar", src.enabledCookieJar)
                    o.addProperty("concurrentRate", src.concurrentRate)
                    o.addProperty("header", src.header)
                    o.addProperty("loginUrl", src.loginUrl)
                    o.addProperty("loginCheckJs", src.loginCheckJs)
                    o.addProperty("coverDecodeJs", src.coverDecodeJs)
                    // LegadoTeam 新增:mainJs(JS 单文件书源)——Rubato 砍单,
                    // 但 isJsSource() 决定四步入口走向,必须两侧一致解析出来
                    o.addProperty("mainJs", src.mainJs)
                    o.addProperty("lastUpdateTime", src.lastUpdateTime)
                    o.addProperty("respondTime", src.respondTime)
                    o.addProperty("weight", src.weight)
                    o.addProperty("exploreUrl", src.exploreUrl)
                    o.addProperty("searchUrl", src.searchUrl)
                    o.add("ruleSearch", ruleObj(src.ruleSearch))
                    o.add("ruleExplore", ruleObj(src.ruleExplore))
                    o.add("ruleBookInfo", ruleObj(src.ruleBookInfo))
                    o.add("ruleToc", ruleObj(src.ruleToc))
                    o.add("ruleContent", ruleObj(src.ruleContent))
                }
                "detectCharset" -> {
                    // EncodingDetect.getHtmlEncode(真身:meta 判定 + icu4j 兜底)
                    val bytes = java.util.Base64.getDecoder().decode(c["bytesBase64"].asString)
                    o.addProperty("result", io.legado.app.utils.EncodingDetect.getHtmlEncode(bytes))
                }
                "fetch" -> {
                    // HTTP 录放全链路:AnalyzeUrl 构造 + getStrResponse
                    // (契约 docs/http-snapshot.md;回放客户端 harness/ReplayHttp)
                    io.legado.app.help.CacheManager.clearMemory()
                    io.legado.app.data.appDb.cookieDao.clear()
                    ReplayHttp.newCase()
                    c["cookies"]?.asJsonObject?.entrySet()?.forEach { (domain, v) ->
                        io.legado.app.help.http.CookieStore.setCookie(domain, v.asString)
                    }
                    c["sessionCookies"]?.asJsonObject?.entrySet()?.forEach { (domain, v) ->
                        io.legado.app.help.CacheManager.putMemory(
                            "${domain}_session_cookie", v.asString
                        )
                    }
                    val source = c["source"]?.asJsonObject?.let { s ->
                        io.legado.app.data.entities.BookSource().apply {
                            bookSourceUrl = s["url"]?.asString ?: ""
                            enabledCookieJar = s["enabledCookieJar"]?.asBoolean ?: false
                        }
                    }
                    val data: io.legado.app.model.analyzeRule.RuleDataInterface? =
                        when (c["ruleData"]?.asString ?: "plain") {
                            "none" -> null
                            else -> io.legado.app.model.analyzeRule.RuleData()
                        }
                    c["vars"]?.asJsonObject?.entrySet()?.forEach { (k, v) ->
                        data?.putVariable(k, v.asString)
                    }
                    val headerMapF = c["headers"]?.asJsonObject?.let { obj ->
                        LinkedHashMap<String, String>().apply {
                            obj.entrySet().forEach { (k, v) -> put(k, v.asString) }
                        }
                    }
                    val au = try {
                        io.legado.app.model.analyzeRule.AnalyzeUrl(
                            mUrl = c["mUrl"].asString,
                            key = c["key"]?.asString,
                            page = c["page"]?.asInt,
                            baseUrl = c["baseUrl"]?.asString ?: "",
                            source = source,
                            ruleData = data,
                            headerMapF = headerMapF,
                        )
                    } catch (e: Throwable) {
                        o.addProperty("error", "url_error")
                        return o
                    }
                    try {
                        val resp = au.getStrResponse()
                        o.addProperty("url", resp.url)
                        o.addProperty("status", resp.code())
                        o.addProperty("body", resp.body)
                        val reqs = JsonArray()
                        ReplayHttp.hops.forEach { (m, u) ->
                            val e = JsonArray()
                            e.add(m)
                            e.add(u)
                            reqs.add(e)
                        }
                        o.add("requests", reqs)
                        val hm = JsonObject()
                        au.headerMap.forEach { (k, v) -> hm.addProperty(k, v) }
                        o.add("headers", hm)
                        val db = JsonObject()
                        io.legado.app.data.appDb.cookieDao.all()
                            .forEach { (k, v) -> db.addProperty(k, v) }
                        o.add("cookiesDb", db)
                        val sess = JsonObject()
                        io.legado.app.help.CacheManager.memorySnapshot().toSortedMap()
                            .forEach { (k, v) ->
                                if (k.endsWith("_session_cookie")) {
                                    sess.addProperty(
                                        k.removeSuffix("_session_cookie"), v.toString()
                                    )
                                }
                            }
                        o.add("cookiesSession", sess)
                    } catch (e: Throwable) {
                        var cause: Throwable? = e
                        var msg = "fetch_error"
                        while (cause != null) {
                            val m = cause.message ?: ""
                            if (m.startsWith("snapshot_miss:") || m == "too_many_redirects") {
                                msg = m
                                break
                            }
                            cause = cause.cause
                        }
                        val keys = o.keySet().toList()
                        keys.forEach { if (it != "id") o.remove(it) }
                        o.addProperty("error", msg)
                    }
                }
                "analyzeRule" -> {
                    // AnalyzeRule 全链路:content + 规则 → 结果 + 变量终态。
                    // JS/WebView/网络走确定性桩,契约见
                    // fixtures/cases/rule-engine/README.md
                    val data: io.legado.app.model.analyzeRule.RuleDataInterface? =
                        when (c["ruleData"]?.asString ?: "plain") {
                            "none" -> null
                            "book" -> io.legado.app.data.entities.Book().apply {
                                name = c["bookName"]?.asString ?: ""
                                author = c["bookAuthor"]?.asString ?: ""
                            }
                            else -> io.legado.app.model.analyzeRule.RuleData()
                        }
                    c["vars"]?.asJsonObject?.entrySet()?.forEach { (k, v) ->
                        data?.putVariable(k, v.asString)
                    }
                    val ar = io.legado.app.model.analyzeRule.AnalyzeRule(data, null)
                    var chapter: io.legado.app.data.entities.BookChapter? = null
                    if (c["chapter"]?.asBoolean == true) {
                        chapter = io.legado.app.data.entities.BookChapter().apply {
                            title = c["title"]?.asString ?: ""
                        }
                        c["chapterVars"]?.asJsonObject?.entrySet()?.forEach { (k, v) ->
                            chapter.putVariable(k, v.asString)
                        }
                        with(io.legado.app.model.analyzeRule.AnalyzeRule) { ar.setChapter(chapter) }
                    }
                    c["nextChapterUrl"]?.asString?.let {
                        with(io.legado.app.model.analyzeRule.AnalyzeRule) { ar.setNextChapterUrl(it) }
                    }
                    // contentType=map:内容是 gson LinkedTreeMap(JS 宿主把
                    // Map<String, Any?> 交给 AnalyzeRule 的那条路)。用
                    // GSON.fromJsonObject<Map<String, Any?>> 构造 —— 这条路
                    // 正好吃 INITIAL_GSON 的 MapDeserializerDoubleAsIntFix
                    val contentAny: Any = when (c["contentType"]?.asString ?: "string") {
                        "map" -> io.legado.app.utils.GSON
                            .fromJsonObject<Map<String, Any?>>(c["content"].asString)
                            .getOrNull() ?: run {
                                o.addProperty("error", "content_error")
                                return o
                            }
                        else -> c["content"].asString
                    }
                    ar.setContent(contentAny, c["baseUrl"]?.asString)
                    c["redirectUrl"]?.asString?.let { ar.setRedirectUrl(it) }
                    val rule = c["rule"].asString
                    val isUrl = c["isUrl"]?.asBoolean ?: false
                    val mode = c["mode"].asString
                    try {
                        when (mode) {
                            "string" -> o.addProperty("result", ar.getString(rule, null, isUrl))
                            "stringList" -> {
                                val l: List<*>? = ar.getStringList(rule, null, isUrl)
                                if (l == null) o.add("result", com.google.gson.JsonNull.INSTANCE)
                                else {
                                    val arr = JsonArray()
                                    l.forEach { arr.add(it?.toString()) }
                                    o.add("result", arr)
                                }
                            }
                            "element" -> {
                                val e = ar.getElement(rule)
                                if (e == null) o.add("result", com.google.gson.JsonNull.INSTANCE)
                                else o.addProperty("result", e.toString())
                            }
                            "elements" -> {
                                val arr = JsonArray()
                                ar.getElements(rule).forEach { arr.add(it.toString()) }
                                o.add("result", arr)
                            }
                            else -> o.addProperty("error", "unknown_mode")
                        }
                        val vars = JsonObject()
                        if (data != null) {
                            val m = JsonObject()
                            java.util.TreeMap(data.variableMap).forEach { (k, v) -> m.addProperty(k, v) }
                            vars.add("data", m)
                        }
                        if (chapter != null) {
                            val m = JsonObject()
                            java.util.TreeMap(chapter.variableMap).forEach { (k, v) -> m.addProperty(k, v) }
                            vars.add("chapter", m)
                        }
                        o.add("vars", vars)
                    } catch (e: Throwable) {
                        o.remove("result")
                        o.remove("vars")
                        o.addProperty("error", "rule_error")
                    }
                }
                "jsonDsl" -> {
                    // AnalyzeByJSonPath 的 DSL 面(&&/||/%% 与 {$.} 内嵌规则)
                    // 构造即解析,非法 JSON 抛 InvalidJsonException(用例不生成这类)
                    val a = io.legado.app.model.analyzeRule.AnalyzeByJSonPath(c["json"].asString)
                    val rule = c["rule"].asString
                    when (c["mode"].asString) {
                        "string" -> {
                            val r = a.getString(rule)
                            if (r == null) o.add("result", com.google.gson.JsonNull.INSTANCE)
                            else o.addProperty("result", r)
                        }
                        "stringList" -> {
                            val arr = JsonArray()
                            a.getStringList(rule).forEach { arr.add(it) }
                            o.add("result", arr)
                        }
                        "list" -> {
                            val arr = JsonArray()
                            a.getList(rule)?.forEach { arr.add(it.toString()) }
                            o.add("result", arr)
                        }
                        "object" -> try {
                            o.addProperty("result", a.getObject(rule).toString())
                        } catch (e: Exception) {
                            // PathNotFound / InvalidPath / 对 null 调 toString 的 NPE
                            o.addProperty("error", "read_error")
                        }
                        else -> o.addProperty("error", "unknown_mode")
                    }
                }
                "absUrl" -> {
                    // NetworkUtils.getAbsoluteURL(URL?, String):base 先按 java.net.URL 解析
                    val b = c["base"].asString
                    val u = try {
                        java.net.URL(b)
                    } catch (e: Exception) {
                        null
                    }
                    if (u == null) {
                        o.addProperty("error", "base_malformed")
                    } else {
                        o.addProperty(
                            "result",
                            io.legado.app.utils.NetworkUtils.getAbsoluteURL(u, c["path"].asString)
                        )
                    }
                }
                "absUrlStr" -> {
                    // NetworkUtils.getAbsoluteURL(String?, String)
                    val b = c["base"].takeIf { !it.isJsonNull }?.asString
                    o.addProperty(
                        "result",
                        io.legado.app.utils.NetworkUtils.getAbsoluteURL(b, c["path"].asString)
                    )
                }
                "jsonRead" -> {
                    // 观察面与 AnalyzeByJSonPath 一致:read<Any> 后按值类型渲染;
                    // 一切异常(InvalidPath/PathNotFound/对 null 调 toString 的 NPE)
                    // 在引擎里都被 catch 吞成空结果,这里统一规范化为 read_error
                    try {
                        val ctx = com.jayway.jsonpath.JsonPath.parse(c["json"].asString)
                        val ob = ctx.read<Any>(c["path"].asString)
                        if (ob is List<*>) {
                            val arr = JsonArray()
                            ob.forEach { arr.add(it.toString()) }
                            o.add("list", arr)
                        } else {
                            o.addProperty("scalar", ob.toString())
                        }
                    } catch (e: Exception) {
                        o.remove("list")
                        o.remove("scalar")
                        o.addProperty("error", "read_error")
                    }
                }
                "regexFind" -> {
                    val regex = try {
                        c["pattern"].asString.toRegex()
                    } catch (e: Exception) {
                        null
                    }
                    if (regex == null) {
                        o.addProperty("error", "compile_error")
                    } else {
                        val arr = JsonArray()
                        var count = 0
                        for (m in regex.findAll(c["data"].asString)) {
                            if (count++ >= 50) break
                            val g = JsonArray()
                            for (i in m.groups.indices) {
                                val gv = m.groups[i]
                                if (gv == null) g.add(com.google.gson.JsonNull.INSTANCE)
                                else g.add(gv.value)
                            }
                            arr.add(g)
                        }
                        o.add("result", arr)
                    }
                }
                else -> o.addProperty("error", "exception:UnknownOp:$op")
            }
    } catch (e: StringIndexOutOfBoundsException) {
        o.remove("result")
        o.remove("elementsType")
        o.remove("kinds")
        o.addProperty("error", "index_out_of_bounds")
    } catch (e: Error) {
        o.remove("result")
        o.remove("elementsType")
        o.remove("kinds")
        o.addProperty("error", e.message ?: "error:null")
    } catch (e: Exception) {
        o.remove("result")
        o.remove("elementsType")
        o.remove("kinds")
        o.addProperty("error", "exception:${e.javaClass.simpleName}:${e.message}")
    }
    return o
}

fun main(args: Array<String>) {
    require(args.size in 2..3) { "用法: harness <cases.json> <out.jsonl> [http快照根目录]" }
    ReplayHttp.reset(args.getOrNull(2)?.let(::File))
    val gson = GsonBuilder().disableHtmlEscaping().serializeNulls().create()
    val cases = gson.fromJson(File(args[0]).readText(), JsonArray::class.java)
    val out = StringBuilder()

    // 单 case 跑在可弃线程上:野生正则可能灾难性回溯,超时记 "timeout" 并换线程
    var executor = java.util.concurrent.Executors.newSingleThreadExecutor { r ->
        Thread(r).apply { isDaemon = true }
    }
    // 逐例计时(默认关,见 timingRounds)。关的时候这段与老写法逐字节等价 ——
    // 计时不能改判据面。多轮取**每例最小值**:第一轮跑在解释器里,不预热出来的
    // 数字是「JIT 没热」而不是「Kotlin 慢」;输出取**最后一轮**,前几轮只为计时。
    val timing = timingRounds() > 0
    val rounds = if (timing) timingRounds() else 1
    val best = LongArray(cases.size()) { Long.MAX_VALUE }
    for (round in 0 until rounds) {
        val last = round == rounds - 1
        for ((idx, el) in cases.withIndex()) {
        val c = el.asJsonObject
        // 计时**在工作线程里头**取:submit/get 那一次交接是几十微秒,
        // 而最快的那几套单例只有一两微秒 —— 把交接算进去等于给裁判侧灌水。
        val ns = java.util.concurrent.atomic.AtomicLong(-1)
        val future = executor.submit(java.util.concurrent.Callable {
            val t0 = System.nanoTime()
            try {
                runCase(c)
            } finally {
                ns.set(System.nanoTime() - t0)
            }
        })
        val o = try {
            future.get(5, java.util.concurrent.TimeUnit.SECONDS)
        } catch (e: java.util.concurrent.TimeoutException) {
            future.cancel(true)
            executor.shutdownNow()
            executor = java.util.concurrent.Executors.newSingleThreadExecutor { r ->
                Thread(r).apply { isDaemon = true }
            }
            JsonObject().apply {
                addProperty("id", c["id"].asString)
                addProperty("error", "timeout")
            }
        } catch (e: java.util.concurrent.ExecutionException) {
            JsonObject().apply {
                addProperty("id", c["id"].asString)
                addProperty("error", "exception:${e.cause?.javaClass?.simpleName}:${e.cause?.message}")
            }
        }
        val took = ns.get()
        if (took in 0 until best[idx]) best[idx] = took
        if (last) {
            // 超时那一条 callback 可能没跑到 finally —— 没测到就不写 __ns,
            // 让报表把它算进「没计到时的例」而不是当成 0。
            if (timing && best[idx] != Long.MAX_VALUE) o.addProperty("__ns", best[idx])
            out.append(gson.toJson(o)).append('\n')
        }
        }
    }
    executor.shutdownNow()

    File(args[1]).writeText(out.toString())
}

/// 逐例计时的轮数:`RUBATO_DIFF_TIME=1` 打开(否则 0 = 关),轮数由
/// `RUBATO_DIFF_ROUNDS` 给,缺省 3。与 difftest::timing_rounds 同一份口径。
private fun timingRounds(): Int {
    if (System.getenv("RUBATO_DIFF_TIME") != "1") return 0
    return System.getenv("RUBATO_DIFF_ROUNDS")?.toIntOrNull()?.takeIf { it > 0 } ?: 3
}
