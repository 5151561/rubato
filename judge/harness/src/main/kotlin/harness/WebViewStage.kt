// 剧本 WebView 的**本体**:剧本 + 虚拟时钟(内联泵)。
//
// 为什么 :harness 也要有这一套:`{"webView": true}` 的书源经 `AnalyzeUrl`
// 走 `BackstageWebView` 那 394 行(此前这里是「回传 javaScript 原文」的
// Phase 1 桩)。真 WebView 换成一份**剧本** —— case 里写清「加载这一页会依次
// 发生什么」,契约与 `fixtures/cases/webview/README.md` 逐字同一份
// (被测侧是 `difftest::wv_script`)。
//
// **与 :wvharness 的那份不同在哪**:那边把整个 `getStrResponse()` 挂在
// `Dispatchers.Unconfined` 上跑,时钟从**外面**推。这里推不了 ——
// `AnalyzeUrl.getStrResponse()` 与 `AnalyzeRule.getWebJsResult()` 都是
// `runBlocking { … }`,一旦挂起就没人再喂那个事件循环,直接死锁。
// 所以这里的时钟是**内联泵**:入队即就地按虚拟时间跑完,`BackstageWebView`
// 的回调因此在 `suspendCancellableCoroutine` 那个块**返回之前**就 resume 了
// —— 协程根本不挂起。语义与真身一致(同一时刻先 post 的先跑),而
// 「等 900 ms」这种事一秒都不用真等。
package harness

import java.util.PriorityQueue

/** 一次加载的剧本(case 的 `webview` 字段) */
class WvScript(
    val overrideUrls: List<Override> = emptyList(),
    val loadResources: List<String> = emptyList(),
    /** 这一次加载**会不会**完成(缺省会)。false = 一直加载不完,只能等超时 */
    val pageFinished: Boolean = true,
    val pageFinishedUrl: String? = null,
    val evals: List<String> = emptyList(),
    val cookie: String? = null,
    /** **后来的**加载完成:`(第几毫秒, 那张页的地址)`。跳转站(`Redirecting…`)
     * 就是这个形状 —— 真身每来一次 `onPageFinished` 都 `removeCallbacks` 掉还没跑的
     * 求值、重排 `100 + delayTime`,所以取到的是**最后一次**之后的页面 */
    val laterPageFinished: List<Later> = emptyList(),
) {
    class Override(val url: String, val isRedirect: Boolean)

    class Later(val at: Long, val url: String?)
}

/** 虚拟时钟到点了还没出结果 —— 真身那边是 `withTimeout` 抛。 */
class WebViewTimeout : Exception("webview timeout")

object WvStage {
    /** 本例的剧本;每例由 Main 换一次 */
    var script: WvScript = WvScript()
        private set

    private class Entry(val at: Long, val seq: Long, val tag: Any?, val run: () -> Unit)

    private var q = PriorityQueue<Entry>(compareBy({ it.at }, { it.seq }))
    private var seq = 0L
    private var evalIndex = 0
    private var pumping = false
    private var loading = false
    private var deadline = DEFAULT_TIMEOUT

    /** 当前虚拟时刻(毫秒,**每次加载从 0 起**) */
    var now = 0L
        private set

    const val DEFAULT_TIMEOUT = 60000L

    fun resetCase(s: WvScript) {
        script = s
        endLoad()
    }

    /**
     * `WebViewPool.acquire` 那一刻:清队列、时钟归零、剧本游标归零。
     * 一个 case 里可以有**好几次** `BackstageWebView`(四步各一次),
     * 每次都是独立的一条时间轴 —— 真身那边 `withTimeout` 也是每次重新计时。
     */
    fun beginLoad() {
        q = PriorityQueue(compareBy({ it.at }, { it.seq }))
        seq = 0
        now = 0
        evalIndex = 0
        loading = true
        deadline = DEFAULT_TIMEOUT
    }

    /** `withTimeout(timeout ?: 60000)` 的那个数(由剧本 WebView 反射真身读出) */
    fun setDeadline(ms: Long) {
        deadline = ms
    }

    fun endLoad() {
        loading = false
        q.clear()
    }

    /** 这一次加载还在进行吗(= WebView 还没还回池子)。剧本据此停止送事件 */
    fun isLoading(): Boolean = loading

    fun post(delayMillis: Long, tag: Any?, run: () -> Unit) {
        q.add(Entry(now + maxOf(0L, delayMillis), seq++, tag, run))
        pump()
    }

    /** `Handler.removeCallbacks`:按 runnable 身份摘掉还没到点的那些 */
    fun remove(tag: Any?) {
        q.removeIf { it.tag === tag }
    }

    /**
     * 内联泵:把队列按虚拟时间跑干净。
     *
     * - **可重入保护**:回调里再 post 的只入队,由最外层这一圈接着跑 ——
     *   否则栈会随重试梯子一路加深。
     * - **到点**:下一件事排在 deadline 之后,或者队列空了而这次加载还没收尾
     *   (`pageFinished: false`),都是 `withTimeout` 到点。抛出去 ——
     *   真身 `load()` 的 `catch (e: Exception)` 会收成 onError + destroy,
     *   与「被 withTimeout 取消」那条路的观察面一致(异常类别两侧都归一)。
     */
    private fun pump() {
        if (pumping) return
        pumping = true
        try {
            while (true) {
                val e = q.peek() ?: break
                if (e.at > deadline) {
                    endLoad()
                    throw WebViewTimeout()
                }
                q.poll()
                now = e.at
                e.run()
            }
            if (loading) {
                // 队列空了、加载还没收尾:真身那边时间还在走,withTimeout 照样到点
                endLoad()
                throw WebViewTimeout()
            }
        } finally {
            pumping = false
        }
    }

    /**
     * `evaluateJavascript` / `loadUrl("javascript:…")` 的回值:剧本里的第 N 条。
     * **用完之后一直重复最后一条**;剧本没给就一律 `"null"`(= JS 返回 null,
     * 真身据此重试)。
     */
    fun nextEval(): String {
        val evals = script.evals
        val reply = when {
            evals.isEmpty() -> "null"
            evalIndex < evals.size -> evals[evalIndex]
            else -> evals.last()
        }
        evalIndex++
        return reply
    }
}
