// webView 策略差分执行器(裁判侧)。
//
// 读 cases.json,逐条把**真身** `BackstageWebView` 跑在「剧本 WebView + 虚拟时钟」
// 上,输出规范化 JSONL。契约:fixtures/cases/webview/README.md。
//
// 被测侧等价物:rust/crates/difftest/src/bin/webview_case_runner.rs
// (底下是 `webview-compat`)—— 输出的字段名、顺序、归一方式必须逐字一致。
package wvharness

import com.google.gson.GsonBuilder
import com.google.gson.JsonArray
import com.google.gson.JsonObject
import io.legado.app.help.http.BackstageWebView
import io.legado.app.help.http.CookieStore
import io.legado.app.help.http.StrResponse
import io.legado.app.model.Debug
import io.legado.app.help.webView.WebViewPool
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.launch
import java.io.File

private val GSON = GsonBuilder().disableHtmlEscaping().serializeNulls().create()

private fun JsonObject.str(name: String): String? =
    get(name)?.takeIf { !it.isJsonNull }?.asString

private fun JsonObject.long(name: String, def: Long): Long =
    get(name)?.takeIf { !it.isJsonNull }?.asLong ?: def

private fun JsonObject.bool(name: String, def: Boolean): Boolean =
    get(name)?.takeIf { !it.isJsonNull }?.asBoolean ?: def

private fun parseScript(c: JsonObject): Script {
    val o = c.getAsJsonObject("webview") ?: JsonObject()
    val overrides = o.getAsJsonArray("overrideUrls")?.map {
        val e = it.asJsonObject
        Script.Override(e.get("url").asString, e.bool("isRedirect", false))
    } ?: emptyList()
    val resources = o.getAsJsonArray("loadResources")?.map { it.asString } ?: emptyList()
    val evals = o.getAsJsonArray("evals")?.map { it.asString } ?: emptyList()
    val later = o.getAsJsonArray("laterPageFinished")?.map {
        val e = it.asJsonObject
        Script.Later(e.get("at").asLong, e.str("url"))
    } ?: emptyList()
    return Script(
        overrides, resources, o.bool("pageFinished", true), o.str("pageFinishedUrl"),
        evals, o.str("cookie"), later,
    )
}

private fun headerMapOf(c: JsonObject): HashMap<String, String>? {
    val o = c.getAsJsonObject("headerMap") ?: return null
    val m = HashMap<String, String>()
    o.entrySet().forEach { (k, v) -> m[k] = v.asString }
    return m
}

@OptIn(kotlinx.coroutines.DelicateCoroutinesApi::class)
private fun runCase(c: JsonObject): JsonObject {
    val out = JsonObject()
    out.addProperty("id", c.get("id").asString)

    Stage.reset(parseScript(c))
    CookieStore.clear()
    Debug.clear()
    WebViewPool.resetCounters()

    val timeout = c.get("timeout")?.takeIf { !it.isJsonNull }?.asLong
    val bwv = BackstageWebView(
        url = c.str("url"),
        html = c.str("html"),
        encode = c.str("encode"),
        tag = c.str("tag"),
        headerMap = headerMapOf(c),
        sourceRegex = c.str("sourceRegex"),
        overrideUrlRegex = c.str("overrideUrlRegex"),
        javaScript = c.str("javaScript"),
        delayTime = c.long("delayTime", 0L),
        cacheFirst = c.bool("cacheFirst", false),
        timeout = timeout,
        result = c.str("result"),
        isRule = c.bool("isRule", false),
    )

    var res: StrResponse? = null
    var err: Throwable? = null
    var done = false
    // Unconfined:`getStrResponse()` 一路**内联**跑到第一次挂起
    // (`suspendCancellableCoroutine`),回调由虚拟时钟那一拍就地恢复它。
    // 换任何别的调度器都会引入线程交错,而本套要的正是确定的顺序。
    val job = GlobalScope.launch(Dispatchers.Unconfined, CoroutineStart.DEFAULT) {
        try {
            res = bwv.getStrResponse()
        } catch (e: Throwable) {
            err = e
        } finally {
            done = true
        }
    }
    // 真身是 `withTimeout(timeout ?: 60000)`,那是**真时间**;这里按虚拟时间判 ——
    // 语义一样(到点还没出结果就是超时),但可比、且不用真等一分钟。
    val deadline = timeout ?: 60000L
    // 剧本回放里抛出来的(比如 sourceRegex 编译不了)要**当成本 case 的结果**记下,
    // 不能让它穿到 main 去 —— 那样输出就只剩 {id, error},evals/headers 全丢,
    // 被测侧照样会给全套,一比就成了假 FAIL
    @Suppress("UNUSED_VARIABLE") val timedOut = try {
        VirtualClock.drain(deadline) { done }
    } catch (e: Throwable) {
        err = e
        done = true
        false
    }
    // **先记下「取消之前完没完」**:`job.cancel()` 会让协程带着
    // CancellationException 恢复,而那个异常会被上面的 `catch (e: Throwable)`
    // 收进 `err` —— 判据要的是「超时」,不是「取消」。
    val finished = done
    if (finished) {
        // 出结果之后还得再把队列排空:真身收尾那一步是 `mHandler.post { destroy() }`
        // —— 它排在队列里,不跑就永远看不到「WebView 还了没有」。
        VirtualClock.drain(deadline) { false }
    } else {
        // 取消会触发 `invokeOnCancellation { runOnUI { destroy() } }` —— WebView 还回去
        job.cancel()
    }

    val trace = Stage.record()
    when {
        // 队列空了也算超时:真身那边时间还在走,`withTimeout` 照样会到点
        !finished -> out.addProperty("error", "timeout")
        err != null -> out.addProperty("error", errorTag(err!!))
        else -> {
            val r = res!!
            out.addProperty("url", r.url)
            out.addProperty("body", r.body)
            out.addProperty("isRedirect", r.raw.priorResponse != null)
        }
    }

    val evals = JsonArray()
    trace.evals.forEach {
        val e = JsonObject()
        e.addProperty("at", it.at)
        e.addProperty("js", it.js)
        evals.add(e)
    }
    out.add("evals", evals)
    out.addProperty("ua", trace.ua)
    out.addProperty("cacheMode", trace.cacheMode)
    out.addProperty("blockNetworkImage", trace.blockNetworkImage)
    out.addProperty("loadedUrl", trace.loadedUrl)
    out.addProperty("loadedHtml", trace.loadedHtml)
    out.addProperty("loadedEncoding", trace.loadedEncoding)
    // 请求头按**键名排序**输出:真身的 headerMap 是 `HashMap`,
    // `toWebViewRequestConfig` 顺着它的迭代序建 LinkedHashMap —— 那是 Java 的
    // 桶序,复刻它没有意义(产品行为也不依赖它)。用例里不放「多个大小写不同的
    // User-Agent」,那是唯一会让顺序变成语义的地方(见本套 README)。
    val headers = JsonObject()
    trace.requestHeaders?.toSortedMap()?.forEach { (k, v) -> headers.addProperty(k, v) }
    out.add("headers", headers)
    val cookies = JsonObject()
    CookieStore.saved.forEach { (k, v) -> cookies.addProperty(k, v) }
    out.add("cookies", cookies)
    out.addProperty("released", WebViewPool.releaseCount)
    return out
}

/** 异常 → 差分标签。「js执行超时」单列一档,别的按类名 */
private fun errorTag(e: Throwable): String = when {
    e.message == "js执行超时" -> "js_timeout"
    e is kotlinx.coroutines.TimeoutCancellationException -> "timeout"
    else -> e.javaClass.simpleName
}

fun main(args: Array<String>) {
    require(args.size == 2) { "用法: wvharness <cases.json> <out.jsonl>" }
    val cases = GSON.fromJson(File(args[0]).readText(), JsonArray::class.java)
    val out = StringBuilder()
    // 逐例计时(默认关,见 timingRounds)。这一套两侧的 WebView 都挂虚拟时钟,
    // 所以量到的是**策略层记账**的耗时,不含真的等 900ms —— 报表里要单独说明。
    val timing = timingRounds() > 0
    val rounds = if (timing) timingRounds() else 1
    val best = LongArray(cases.size()) { Long.MAX_VALUE }
    for (round in 0 until rounds) {
        val last = round == rounds - 1
        for ((idx, el) in cases.withIndex()) {
            val c = el.asJsonObject
            val t0 = System.nanoTime()
            val o = try {
                runCase(c)
            } catch (e: Throwable) {
                JsonObject().apply {
                    addProperty("id", c.get("id").asString)
                    addProperty("error", errorTag(e))
                }
            }
            val took = System.nanoTime() - t0
            if (took < best[idx]) best[idx] = took
            if (last) {
                if (timing) o.addProperty("__ns", best[idx])
                out.append(GSON.toJson(o)).append('\n')
            }
        }
    }
    File(args[1]).writeText(out.toString())
}

/// 逐例计时的轮数:`RUBATO_DIFF_TIME=1` 打开(否则 0 = 关),轮数由
/// `RUBATO_DIFF_ROUNDS` 给,缺省 3。与 difftest::timing_rounds 同一份口径。
private fun timingRounds(): Int {
    if (System.getenv("RUBATO_DIFF_TIME") != "1") return 0
    return System.getenv("RUBATO_DIFF_ROUNDS")?.toIntOrNull()?.takeIf { it > 0 } ?: 3
}
