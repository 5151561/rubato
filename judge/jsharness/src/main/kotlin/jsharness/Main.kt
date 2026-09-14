// js-host 差分执行器(裁判侧)。
//
// 被测对象:**一段书源 JS 在 AnalyzeRule 上求值**的完整语义 —— 引擎方言
// (真 Rhino / htmlunit-core-js)+ `java` 宿主 API(JsExtensions 真身)+
// 绑定面(result/baseUrl/src/page/…)+ 变量层(java.put/get)。
//
// 与 :harness 的分工:那边把 JS 换成确定性桩,钉 AnalyzeRule 的调度与拼接;
// 这边把 JS 打开,钉 JS 侧本身。两侧用例目录、契约互不相干。
// 契约见 fixtures/cases/js-host/README.md,被测侧是 difftest 的 js_case_runner。
package jsharness

import com.google.gson.GsonBuilder
import com.google.gson.JsonArray
import com.google.gson.JsonObject
import io.legado.app.data.entities.Book
import io.legado.app.data.entities.BookSource
import io.legado.app.help.source.clearExploreKindsCache
import io.legado.app.help.source.exploreKinds
import io.legado.app.model.analyzeRule.AnalyzeRule
import io.legado.app.model.analyzeRule.AnalyzeRule.Companion.setChapter
import io.legado.app.model.analyzeRule.AnalyzeUrl
import io.legado.app.model.analyzeRule.NET_UNSUPPORTED
import io.legado.app.utils.FS_UNSUPPORTED
import io.legado.app.utils.GSON
import org.htmlunit.corejs.javascript.NativeArray
import org.htmlunit.corejs.javascript.Scriptable
import org.htmlunit.corejs.javascript.Undefined
import org.htmlunit.corejs.javascript.Wrapper
import java.io.File
import java.util.Locale

private const val DEFAULT_SOURCE = """{"bookSourceUrl":"https://difftest.example.com",
"bookSourceName":"difftest","bookSourceType":0,"enabled":true}"""

/** 完成值 → 粗类型标签(两侧同一套) */
private fun typeOf(v: Any?): String = when {
    v == null || v is Undefined -> "null"
    v is Boolean -> "bool"
    v is Number -> "number"
    v is CharSequence -> "string"
    v is NativeArray -> "list"
    v is Wrapper -> "wrapped"
    v is Scriptable -> "object"
    else -> "other"
}

/**
 * AnalyzeRule 的消费口径(AnalyzeRule.kt L798-806):JS 完成值怎么变成拼进规则的串。
 * 这是 `<js>` 结果真正落到规则里的形态,必须逐字对齐。
 */
private fun consumeAsAnalyzeRule(v: Any?): String? = when {
    v == null || v is Undefined -> null
    v is String -> v
    v is Double && v % 1.0 == 0.0 -> String.format(Locale.ROOT, "%.0f", v)
    else -> v.toString()
}

private fun unsupportedTag(msg: String?): String? = when {
    msg == null -> null
    msg.contains(NET_UNSUPPORTED) -> "net"
    msg.contains(FS_UNSUPPORTED) -> "fs"
    else -> null
}

/** 异常 → 归一标签。两个引擎的消息文本必然不同,只比「哪一类错」。 */
private fun errorTag(e: Throwable): String {
    var cur: Throwable? = e
    val seen = HashSet<Throwable>()
    while (cur != null && seen.add(cur)) {
        unsupportedTag(cur.message)?.let { return "unsupported:$it" }
        if (System.getenv("JSHARNESS_DEBUG") == "1") {
            System.err.println("[jsharness] ${cur.javaClass.name}: ${cur.message}")
        }
        when (cur) {
            // 次序要紧:WrappedException 继承自 EvaluatorException,但它只是
            // 「JS 里调 Java 抛了」的包装,真正的错在 wrappedException 上
            is org.htmlunit.corejs.javascript.WrappedException ->
                return hostTag(cur.wrappedException)
            is org.htmlunit.corejs.javascript.EcmaError -> return "js:${cur.name}"
            is org.htmlunit.corejs.javascript.EvaluatorException -> return "js:SyntaxError"
            is org.htmlunit.corejs.javascript.JavaScriptException -> return "js:throw"
            is StackOverflowError -> return "js:StackOverflow"
        }
        cur = cur.cause
    }
    // JSHARNESS_DEBUG=1 时把原始异常打到 stderr(排查用,不进 jsonl)
    if (System.getenv("JSHARNESS_DEBUG") == "1") e.printStackTrace()
    return "host:${e.javaClass.simpleName}"
}


/**
 * `exploreKinds()` 的观察面。**错误那一格单独出**:真身出错时交回的是
 * `ExploreKind("ERROR:${localizedMessage}", stackTraceToString())` —— 消息与栈
 * 两边不可能逐字一致,只比**哪一类错**(与 `errorTag` 同一条口径,取值
 * `js:<ECMA 错名>` / `js:throw` / `json` / `host:<类名>`)。
 */
private fun writeExploreKinds(
    kinds: List<io.legado.app.data.entities.rule.ExploreKind>,
    o: JsonObject
) {
    // **`title` 可能是 null**:Kotlin 那边是非空 `String`,而 gson 的反射适配器
    // 对非基元字段照样写得进 null(`{"title":null}`)—— 直接 `.startsWith` 会
    // 在这里 NPE,把「真身解出来的一格」误报成「裁判自己炸了」。
    val err = kinds.singleOrNull()?.takeIf { (it.title as String?).orEmpty().startsWith("ERROR:") }
    if (err != null) {
        o.addProperty("error", exploreErrorTag(err.url, (err.title as String?).orEmpty()))
        return
    }
    val arr = JsonArray()
    kinds.forEach { k ->
        val e = JsonObject()
        e.addProperty("title", k.title)
        e.addProperty("url", k.url)
        e.addProperty("type", k.type)
        e.addProperty("action", k.action)
        e.addProperty("default", k.default)
        e.addProperty("viewName", k.viewName)
        k.chars?.let { cs ->
            val a = JsonArray()
            cs.forEach { a.add(it) }
            e.add("chars", a)
        }
        // `style` 分两位出:给没给(`style` 键在不在)与**取值**(`style()` 的兜底)。
        // 合成一位会把「没给」和「给了一份与默认相同的」照成一致。
        e.addProperty("hasStyle", k.style != null)
        val st = JsonObject()
        k.style().let { s ->
            // **浮点出成串**:两侧的 JSON 序列化器对 float 的字面形态口径不同
            // (gson 走 `Float.toString`,serde_json 会先加宽到 f64)——
            // 那是**报表**的差,不是语义的差。都取 `Float.toString` 的形态。
            st.addProperty("layout_flexGrow", s.layout_flexGrow.toString())
            st.addProperty("layout_flexShrink", s.layout_flexShrink.toString())
            st.addProperty("layout_alignSelf", s.layout_alignSelf)
            st.addProperty("layout_flexBasisPercent", s.layout_flexBasisPercent.toString())
            st.addProperty("layout_wrapBefore", s.layout_wrapBefore)
            st.addProperty("layout_justifySelf", s.layout_justifySelf)
            st.addProperty("alignSelf()", s.alignSelf())
        }
        e.add("style", st)
        arr.add(e)
    }
    o.add("kinds", arr)
}

/** 出错那一格的栈首行 → 归一标签 */
private fun exploreErrorTag(stack: String?, title: String): String {
    val s = stack ?: ""
    return when {
        s.contains("EcmaError") ->
            "js:" + (Regex("(SyntaxError|ReferenceError|TypeError|RangeError|InternalError)")
                .find(title)?.value ?: "throw")
        s.contains("EvaluatorException") -> "js:SyntaxError"
        s.contains("JavaScriptException") -> "js:throw"
        s.contains("Json") -> "json"
        else -> "host:" + (s.substringBefore(':').substringAfterLast('.').ifEmpty { "Unknown" })
    }
}

/** Java 侧异常 → 标签(宿主 API 抛的错,两侧按类别比) */
private fun hostTag(e: Throwable): String {
    unsupportedTag(e.message)?.let { return "unsupported:$it" }
    e.cause?.let { c -> unsupportedTag(c.message)?.let { return "unsupported:$it" } }
    return "host:${e.javaClass.simpleName}"
}

// Rhino 对象的默认 toString 带身份哈希(NativeArray@2f23b431),逐次运行都不同,
// 也不可能与被测侧一致 —— 归一成标记
private val IDENTITY = Regex("""^\[?[\w.$;\[]*@[0-9a-f]+$""")

// **异常的栈字符串**。`JsExtensions.ajax/connect` 失败时吞掉异常、返回
// `it.stackTraceStr`(`java.lang.IllegalArgumentException: …\n\tat okhttp3.…`)。
// 那串里全是 Java 的类名与行号,被测侧不可能、也不该逐字复现 —— 两侧归一成标记
// (被测侧直接给这个串)。**比的是「失败了」,不是「怎么描述失败」**;
// 至于「是不是在同一处失败」,由 `hops` 那一面钉(发没发出请求、发了几跳)。
private val JAVA_STACK = Regex("""^[\w.$]+(Exception|Error)\b""")

private fun normalizeStr(s: String?): String? = when {
    s == null -> null
    IDENTITY.matches(s) -> "«identity»"
    JAVA_STACK.containsMatchIn(s) -> "«java-stack»"
    else -> s
}

// 未接能力的错误会被 JsExtensions 吞成「返回栈字符串」,两侧无法逐字对齐 → 归一
private fun normalizeLog(s: String): String = when {
    s.contains(NET_UNSUPPORTED) -> "«unsupported:net»"
    s.contains(FS_UNSUPPORTED) -> "«unsupported:fs»"
    // `AnalyzeRule.ajax` 失败时记的是 `ajax(<url>) error\n<整个栈>`
    // (AnalyzeRule.kt L967)。**URL 那半要比**(它是 url 拼装的观察面),
    // 栈那半不比 —— 只把换行之后的部分归一。
    else -> {
        val i = s.indexOf('\n')
        if (i >= 0 && JAVA_STACK.containsMatchIn(s.substring(i + 1))) {
            s.substring(0, i + 1) + "«java-stack»"
        } else if (JAVA_STACK.containsMatchIn(s)) {
            "«java-stack»"
        } else {
            s
        }
    }
}

/**
 * 两侧共享的确定性垫片(冻 `Date` 与 `Math.random`)。由 tools/js_diff.sh 复制到
 * cases.json 的同级目录,被测侧读的是同一份文本。
 *
 * 裁判侧没有「先在作用域里跑一段再跑用例」的入口(AnalyzeRule.evalJS 每次自建
 * 作用域),故**前置拼接**到用例代码前。垫片整段是一条 `var` 语句,完成值为空,
 * 拼接后脚本的完成值仍旧来自用例自己 —— 详见 determinism.js 顶部的说明。
 */
private var determinismPrelude: String = ""

private fun withDeterminism(code: String): String =
    if (determinismPrelude.isEmpty()) code else determinismPrelude + "\n" + code

/** case 的 `verify` 字段 → 剧本用户(字段缺席 = 没有人答) */
private fun parseVerifyScript(c: JsonObject): VerifyScript? {
    val o = c["verify"]?.takeIf { it.isJsonObject }?.asJsonObject ?: return null
    fun str(k: String) = o[k]?.takeIf { !it.isJsonNull }?.asString
    return VerifyScript(
        result = str("result"),
        url = str("url"),
        close = o["close"]?.takeIf { !it.isJsonNull }?.asBoolean ?: false,
    )
}

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
    // webView 的剧本:`java.ajax` 里 `{"webView": true}` 的链接与 `@webjs:`
    // 都走它。不带 `webview` 字段就是缺省剧本(页面加载得完、每次求值都回
    // "null" —— 于是重试梯子跑满、报「js执行超时」)。契约见
    // fixtures/cases/webview/README.md,被测侧同一份在 difftest::wv_script。
    WvStage.resetCase(parseWvScript(c))
    // 「请用户出手」那三件的剧本用户(case 的 `verify` 字段);
    // 不带就是没有人答 —— 调那三件的 case 必须带它
    VerifyStage.resetCase(parseVerifyScript(c))
    try {
        when (val op = c["op"].asString) {
            "js" -> {
                io.legado.app.help.CacheManager.clearAll()
                io.legado.app.model.Debug.clear()
                // 每 case 清空 cookie 库(真身 CookieStore 落 appDb.cookieDao +
                // CacheManager 内存表):用例之间不许串味
                io.legado.app.data.appDb.cookieDao.clear()
                // HTTP 录放:清空本 case 的 hop 记录;回落页按 case 指定
                // (契约 docs/http-snapshot.md §6,缺省 json —— B 层 ajax 绝大多数是 API)
                ReplayHttp.newCase()
                // 随机流回到 block 0(见 DeterministicRandom:计数器是全进程一份的,
                // 不复位就会让「第 N 个 case」取决于前面消耗了多少)
                DeterministicRandom.reset()
                ReplayHttp.fallback =
                    c["fallback"]?.takeIf { !it.isJsonNull }?.asString ?: "json"
                val source = GSON.fromJson(
                    c["source"]?.takeIf { !it.isJsonNull }?.asString ?: DEFAULT_SOURCE,
                    BookSource::class.java
                )
                // 真身 `WebViewModel.initData` 从库里取回同一个源(算加载计划要用它)
                VerifyStage.source = source
                val book = Book().apply {
                    bookUrl = c["bookUrl"]?.asString ?: "https://difftest.example.com/book/1"
                    name = c["bookName"]?.asString ?: "测试书名"
                    author = c["bookAuthor"]?.asString ?: "测试作者"
                    origin = source.bookSourceUrl
                }
                c["vars"]?.asJsonObject?.entrySet()?.forEach { (k, v) ->
                    book.putVariable(k, v.asString)
                }
                // 预置 CacheManager(`loginHeader_<key>` / `userInfo_<key>` /
                // `sourceVariable_<key>` 这几条路要先有值才测得到读取那一半)
                c["cache"]?.asJsonObject?.entrySet()?.forEach { (k, v) ->
                    io.legado.app.help.CacheManager.put(k, v.asString)
                }
                val result: Any? = c["result"]?.takeIf { !it.isJsonNull }?.asString
                val baseUrl = c["baseUrl"]?.takeIf { !it.isJsonNull }?.asString

                // **两个宿主**(见 fixtures/cases/js-host/README.md「两个宿主」):
                // searchUrl/exploreUrl 的 JS 在真身里跑 AnalyzeUrl.evalJS —— 它绑
                // key/page,但 `java` 是 AnalyzeUrl,**没有** getString/getElement/
                // setContent,也不绑 src/title/nextChapterUrl/fromBookInfo。
                // 其余规则位跑 AnalyzeRule.evalJS。挑错宿主会把语义钉歪。
                val value = when (val host = c["host"]?.takeIf { !it.isJsonNull }?.asString ?: "rule") {
                    "url" -> {
                        // mUrl 给个良性值:AnalyzeUrl 的 init 会跑 initUrl()(内含
                        // analyzeJs/replaceKeyPageJs),不能让它先把用例的代码跑了
                        AnalyzeUrl(
                            mUrl = baseUrl ?: "https://difftest.example.com",
                            key = c["key"]?.takeIf { !it.isJsonNull }?.asString,
                            page = c["page"]?.takeIf { !it.isJsonNull }?.asString?.toIntOrNull(),
                            baseUrl = baseUrl ?: "",
                            source = source,
                            ruleData = book,
                            headerMapF = emptyMap<String, String>()
                        ).evalJS(withDeterminism(c["code"].asString), result)
                    }

                    "rule" -> {
                        val rule = AnalyzeRule(book, source)
                        // 章节实体:用例带 `chapter` 才 setChapter —— 不带就是 null
                        // (真身 `AnalyzeRule.chapter` 的缺省)。`chapter.vars` 打进
                        // **章节那一层**,与 `vars` 打进书那一层分开。
                        c["chapter"]?.takeIf { !it.isJsonNull }?.asJsonObject?.let { j ->
                            val ch = io.legado.app.data.entities.BookChapter(
                                url = j["url"]?.asString
                                    ?: "https://difftest.example.com/book/1/2.html",
                                title = j["title"]?.asString ?: "第 12 章 混沌雷池",
                                isVolume = j["isVolume"]?.asBoolean ?: false,
                                baseUrl = j["baseUrl"]?.asString
                                    ?: "https://difftest.example.com/book/1",
                                bookUrl = j["bookUrl"]?.asString
                                    ?: "https://difftest.example.com/book/1",
                                index = j["index"]?.asInt ?: 0,
                                isVip = j["isVip"]?.asBoolean ?: false,
                                isPay = j["isPay"]?.asBoolean ?: false,
                                tag = j["tag"]?.takeIf { t -> !t.isJsonNull }?.asString,
                            )
                            j["vars"]?.asJsonObject?.entrySet()?.forEach { (k, v) ->
                                ch.putVariable(k, v.asString)
                            }
                            rule.setChapter(ch)
                        }
                        c["content"]?.takeIf { !it.isJsonNull }?.let {
                            rule.setContent(it.asString, baseUrl)
                        } ?: baseUrl?.let { rule.setBaseUrl(it) }
                        c["page"]?.takeIf { !it.isJsonNull }?.let { rule.setLocal("page", it.asString) }
                        // **`result` 绑的不一定是串**:产品里 `init: $.xxx` 之后
                        // 那一步的 `result` 是 jayway 从 JSON 页读出来的 **Java 对象**
                        // (Map / List),Rhino 包成 NativeJavaMap / NativeJavaList。
                        // `resultRule` 就给一条规则,走**真身 getElement** 拿到那个对象
                        // 本身再绑 —— 与产品路径逐字同一条(getElement → evalJS)。
                        // 契约见 fixtures/cases/js-host/README.md「result 的形态」。
                        val bound = c["resultRule"]?.takeIf { !it.isJsonNull }
                            ?.let { rule.getElement(it.asString) } ?: result
                        rule.evalJS(withDeterminism(c["code"].asString), bound)
                    }

                    // **第三个宿主**:BaseSource.evalJS(BaseSource.kt L395)——
                    // source 的 header / loginUrl / loginCheckJs 那条路径。
                    // `java` 就是书源实体本身(`source`/`sourceApi` 同一个对象),
                    // 绑定面最窄:没有 result/book/chapter/page/key,
                    // 而 `baseUrl` 是 getKey()(bookSourceUrl)不是页面地址。
                    // `java.put/get` 打的是 **CacheManager**,不是变量层。
                    "source" -> source.evalJS(withDeterminism(c["code"].asString))

                    else -> throw IllegalArgumentException("未知 host: $host")
                }

                val tag = unsupportedTag(value as? String)
                if (tag != null) {
                    o.addProperty("error", "unsupported:$tag")
                } else {
                    o.addProperty("type", typeOf(value))
                    normalizeStr(consumeAsAnalyzeRule(value))?.let { o.addProperty("str", it) }
                    // normalizeJsResult 自己也会抛(Infinity/NaN 过 GSON、循环引用),
                    // 那是 header JS 那条路径的真实行为 → 记成字段级错误,不吞
                    runCatching {
                        // 数字**归一**:Rhino 什么时候给 Integer、什么时候给 Double
                        // 不可预测('abc'.length → Integer 而 1+1 → Double),
                        // 而 gson 按运行时类型输出("3" vs "2.0")。这层表示差
                        // 不属于书源可观察的语义,统一按 Java Double 形态比较。
                        // 见 fixtures/cases/js-host/README.md「已归一的差异」。
                        if (value is Number) {
                            val d = value.toDouble()
                            if (!d.isFinite()) {
                                throw IllegalArgumentException("$d is not a valid double value as per JSON specification")
                            }
                            d.toString()
                        } else {
                            io.legado.app.model.jsSource.JsSourceEngine.normalizeJsResult(value)
                        }
                    }.onSuccess { n -> normalizeStr(n)?.let { o.addProperty("norm", it) } }
                        .onFailure { o.addProperty("normError", hostTag(it)) }
                }
                val vars = JsonObject()
                java.util.TreeMap(book.variableMap).forEach { (k, v) -> vars.addProperty(k, v) }
                o.add("vars", vars)
                val logs = JsonArray()
                io.legado.app.model.Debug.logs.forEach { logs.add(normalizeLog(it)) }
                o.add("logs", logs)
            }

            // **`@js:` url 面**:一条真实的 url 规则(searchUrl)整条走 AnalyzeUrl
            // 真身 —— `@js:` / `<js>` / `{{js}}` / `<a,b,c>` 页码 + option JSON +
            // 绝对化 + query/form 编码,**JS 是真 Rhino**。
            //
            // 与 `analyze-url` 那套(judge/harness,JS 是确定性桩)的分工:那边钉
            // 拼装与编码,这边把 JS 打开钉「书源真的写的那些 JS 在 url 上跑出什么」。
            // headerMapF **不传** —— 于是 `source.getHeaderMap()` 真的跑,
            // header 规则(387 源,14 条是 JS)与默认 UA 注入一并进套。
            "analyzeUrl" -> {
                io.legado.app.help.CacheManager.clearAll()
                io.legado.app.model.Debug.clear()
                io.legado.app.data.appDb.cookieDao.clear()
                ReplayHttp.newCase()
                DeterministicRandom.reset()
                ReplayHttp.fallback = c["fallback"]?.takeIf { !it.isJsonNull }?.asString ?: "json"
                val source = GSON.fromJson(
                    c["source"]?.takeIf { !it.isJsonNull }?.asString ?: DEFAULT_SOURCE,
                    BookSource::class.java
                )
                val book = Book().apply {
                    bookUrl = c["bookUrl"]?.asString ?: "https://difftest.example.com/book/1"
                    name = c["bookName"]?.asString ?: "测试书名"
                    author = c["bookAuthor"]?.asString ?: "测试作者"
                    origin = source.bookSourceUrl
                }
                c["vars"]?.asJsonObject?.entrySet()?.forEach { (k, v) ->
                    book.putVariable(k, v.asString)
                }
                c["cache"]?.asJsonObject?.entrySet()?.forEach { (k, v) ->
                    io.legado.app.help.CacheManager.put(k, v.asString)
                }
                val au = AnalyzeUrl(
                    mUrl = c["mUrl"].asString,
                    key = c["key"]?.takeIf { !it.isJsonNull }?.asString,
                    page = c["page"]?.takeIf { !it.isJsonNull }?.asInt,
                    baseUrl = c["baseUrl"]?.takeIf { !it.isJsonNull }?.asString ?: "",
                    source = source,
                    ruleData = book,
                )
                // 私有字段用反射读(engine 一行未改),与 :harness 的同名 op 同一张投影
                fun priv(name: String): Any? = AnalyzeUrl::class.java
                    .getDeclaredField(name).apply { isAccessible = true }.get(au)
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
                o.addProperty("readTimeoutMs", priv("readTimeoutMs") as Long?)
                o.addProperty("urlTimeoutConfigured", priv("urlTimeoutConfigured") as Boolean)
                o.addProperty("followRedirects", priv("followRedirects") as Boolean?)
                o.addProperty("webViewDelayTime", priv("webViewDelayTime") as Long)
                o.addProperty("serverID", au.serverID)
                o.addProperty("userAgent", au.getUserAgent())
                o.addProperty("isPost", au.isPost())
                val hm = JsonObject()
                au.headerMap.forEach { (k, v) -> hm.addProperty(k, normalizeStr(v)) }
                o.add("headers", hm)
                val vars = JsonObject()
                java.util.TreeMap(book.variableMap).forEach { (k, v) -> vars.addProperty(k, v) }
                o.add("vars", vars)
                val logs = JsonArray()
                io.legado.app.model.Debug.logs.forEach { logs.add(normalizeLog(it)) }
                o.add("logs", logs)
            }

            // **四步流水线 × 真 Rhino**(pipeline-corpus-b)。与 :harness 的同名 op
            // 同一张投影,区别只在 com.script:那边是确定性桩,这边是真引擎。
            "pipeline" -> {
                io.legado.app.help.CacheManager.clearAll()
                io.legado.app.model.Debug.clear()
                io.legado.app.data.appDb.cookieDao.clear()
                io.legado.app.help.RuleBigDataHelp.clear()
                ReplayHttp.newCase()
                DeterministicRandom.reset()
                ReplayHttp.fallback = c["fallback"]?.takeIf { !it.isJsonNull }?.asString ?: "json"
                runPipeline(c, o)
            }

            // **发现页分类列表**(M3s):`BookSourceExtensions.exploreKinds()` 真身。
            // 用例只给整份书源 —— 三条路(JSON 数组 / `title::url` 行 / `@js:`+`<js>`)
            // 由 `exploreUrl` 自己决定走哪条,这正是要比的东西。
            "exploreKinds" -> {
                io.legado.app.help.CacheManager.clearAll()
                io.legado.app.utils.ACache.clearAll()
                io.legado.app.ui.main.explore.ExploreAdapter.clearCache()
                ReplayHttp.newCase()
                DeterministicRandom.reset()
                ReplayHttp.fallback = c["fallback"]?.takeIf { !it.isJsonNull }?.asString ?: "json"
                val source = GSON.fromJson(
                    c["source"]?.takeIf { !it.isJsonNull }?.asString ?: DEFAULT_SOURCE,
                    BookSource::class.java
                )
                VerifyStage.source = source
                // **进程内那张表也要清**:`exploreKindsMap` 的键是
                // md5(bookSourceUrl + exploreUrl),同一份语料里键撞不上,
                // 但让每例冷算是判据面的要求,不是巧合。
                kotlinx.coroutines.runBlocking { source.clearExploreKindsCache() }
                val kinds = kotlinx.coroutines.runBlocking { source.exploreKinds() }
                writeExploreKinds(kinds, o)
            }

            else -> o.addProperty("error", "exception:UnknownOp:$op")
        }
    } catch (e: Throwable) {
        val keys = o.keySet().toList()
        keys.forEach { if (it != "id") o.remove(it) }
        o.addProperty("error", errorTag(e))
    }
    // **发出的请求序列**:URL 拼装/方法/跳数是 ajax 类用例最要紧的观察面,
    // 出错路径上也要记(异常往往正是「多发/少发了一跳」造成的)。
    // 只记 (method, 规范化 URL) —— 回放的响应两侧同一份字节,不必比。
    // **CacheManager 终态**:`source` 宿主的 `java.put/get` 打的就是这张表
    // (`v_<sourceKey>_<key>`),登录头/登录信息/源变量也都落它。空则不出字段。
    val cacheSnap = io.legado.app.help.CacheManager.snapshot()
    if (cacheSnap.isNotEmpty()) {
        val m = JsonObject()
        cacheSnap.forEach { (k, v) -> m.addProperty(k, v) }
        o.add("cache", m)
    }
    // **ACache 终态**(`cache.putFile/getFile` 那张)。与上面那张是两处存储,
    // 分开出字段 —— 合一张会把「写内存、读磁盘」这类分歧照成一致。
    val fileSnap = io.legado.app.help.CacheManager.fileSnapshot()
    if (fileSnap.isNotEmpty()) {
        val m = JsonObject()
        fileSnap.forEach { (k, v) -> m.addProperty(k, v) }
        o.add("cacheFile", m)
    }
    if (ReplayHttp.hops.isNotEmpty() && c["op"].asString != "pipeline") {
        val hops = JsonArray()
        ReplayHttp.hops.forEach { (m, u) -> hops.add("$m $u") }
        o.add("hops", hops)
    }
    // **界面被要求弹什么**(`java.startBrowser*` / `getVerificationCode`):
    // 返回值只说了一半,另一半是用户看见了什么。归一形态与被测侧
    // (`difftest::verify_script::opens_json`)逐字同一份 —— 挂号 key 本身不比
    // (真身是 UUID),只比「要不要等结果」这一位
    if (VerifyStage.opens.isNotEmpty()) {
        val opens = JsonArray()
        VerifyStage.opens.forEach { e ->
            val one = JsonObject()
            val browser = e["activity"] == "WebViewActivity"
            one.addProperty("kind", if (browser) "browser" else "code")
            one.addProperty("url", (e["url"] ?: e["imageUrl"]) as String?)
            if (browser) {
                one.addProperty("title", e["title"] as String?)
                one.addProperty("saveResult", e["sourceVerificationEnable"] as Boolean?)
                one.addProperty("refetchAfterSuccess", e["refetchAfterSuccess"] as Boolean?)
                one.addProperty("html", e["html"] as String?)
                // **可见浏览器到底怎么加载这一页**(真身 `WebViewModel.initData`;
                // 被测侧同一份算在 js-host 的网络面。见 VerifyStage.browserLoad)
                @Suppress("UNCHECKED_CAST")
                val load = e["browserLoad"] as Map<String, Any?>?
                if (load == null) {
                    one.add("browserLoad", null)
                } else {
                    val l = JsonObject()
                    l.addProperty("url", load["url"] as String?)
                    l.addProperty("userAgent", load["userAgent"] as String?)
                    val hm = JsonObject()
                    @Suppress("UNCHECKED_CAST")
                    (load["headers"] as Map<String, String>).forEach { (k, v) ->
                        hm.addProperty(k, normalizeStr(v))
                    }
                    l.add("headers", hm)
                    l.addProperty("html", load["html"] as String?)
                    one.add("browserLoad", l)
                }
            }
            one.addProperty("sourceKey", e["sourceOrigin"] as String?)
            one.addProperty("sourceName", e["sourceName"] as String?)
            one.addProperty("sourceType", e["sourceType"] as Int?)
            one.addProperty("waits", e["verificationResultKey"] != null)
            opens.add(one)
        }
        o.add("verifyOpens", opens)
    }
    return o
}

fun main(args: Array<String>) {
    require(args.size in 2..3) { "用法: jsharness <cases.json> <out.jsonl> [http快照根目录]" }
    // HTTP 录放的快照根(tools/js_diff.sh 传 fixtures/http-js-host)。
    // 不传就没有网络面 —— 任何请求会报 snapshot_miss。
    ReplayHttp.reset(args.getOrNull(2)?.let(::File))
    // 把 JVM 的 http/https URL 流也接到录放上。`java.get/head/post` 走 jsoup,
    // 而 jsoup 用的是 HttpURLConnection —— 不拦这条路,裁判会去连真外网
    // (实测有用例把线上 302 当成了判据)。见 ReplayUrlStream 顶部。
    ReplayUrlStream.install()
    // 冻住宿主侧随机源(java.randomUUID)。必须在任何用例跑之前 —— 见
    // DeterministicRandom 顶部:此前有两个语料用例每次跑结果都不同。
    DeterministicRandom.install()
    val gson = GsonBuilder().disableHtmlEscaping().serializeNulls().create()
    val cases = gson.fromJson(File(args[0]).readText(), JsonArray::class.java)
    File(args[0]).absoluteFile.parentFile?.resolve("determinism.js")?.takeIf { it.isFile }?.let {
        determinismPrelude = it.readText()
    }
    val out = StringBuilder()

    // 单 case 跑在可弃线程上:野生 JS 可能死循环
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
                addProperty("error", errorTag(e.cause ?: e))
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
