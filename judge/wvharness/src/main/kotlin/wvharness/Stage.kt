package wvharness

/**
 * 一个 case 的**剧本**与它跑出来的痕迹。契约:fixtures/cases/webview/README.md。
 *
 * 真 WebView 换成剧本 —— case 说清「加载这一页会依次发生什么」,
 * 垫片照着回调 `WebViewClient`。两侧(这里与 `webview-compat`)逐字实现同一份。
 */
class Script(
    val overrideUrls: List<Override>,
    val loadResources: List<String>,
    /** 这一次加载**会不会**完成(缺省会)。false = 页面一直加载不完,只能等超时 */
    val pageFinished: Boolean,
    val pageFinishedUrl: String?,
    val evals: List<String>,
    val cookie: String?,
    /** **后来的**加载完成:`(第几毫秒, 那张页的地址)`。跳转站(`Redirecting…`)
     * 就是这个形状 —— 真身每来一次 `onPageFinished` 都 `removeCallbacks` 掉还没跑的
     * 求值、重排 `100 + delayTime`,所以取到的是**最后一次**之后的页面 */
    val laterPageFinished: List<Later> = emptyList(),
) {
    class Override(val url: String, val isRedirect: Boolean)

    class Later(val at: Long, val url: String?)
}

/** 一次求值:第几毫秒、跑的是哪段 JS、回了什么 */
class Eval(val at: Long, val js: String, val reply: String)

class Trace {
    /** `WebSettings` 上真身写进去的那几位(UA 来自 toWebViewRequestConfig,是判据面) */
    var ua: String? = null
    var cacheMode: Int = 0
    var blockNetworkImage: Boolean = false
    var loadedUrl: String? = null
    var loadedHtml: String? = null
    var loadedEncoding: String? = null
    var requestHeaders: Map<String, String>? = null
    val evals = mutableListOf<Eval>()
}

object Stage {
    var current: Script? = null
        private set
    private var trace = Trace()
    private var evalIndex = 0

    fun reset(script: Script?) {
        current = script
        trace = Trace()
        evalIndex = 0
        VirtualClock.reset()
    }

    fun record(): Trace = trace

    /**
     * `evaluateJavascript` 的回值:剧本里的第 N 条。
     * **用完之后一直重复最后一条** —— 「一直取不到结果」的重试梯子只要写一个
     * `"null"` 就能钉,不必把 31 条都写出来。剧本为空时给 `"null"`
     * (= JS 返回 null,真身据此重试)。
     */
    fun nextEval(js: String): String {
        val evals = current?.evals.orEmpty()
        val reply = when {
            evals.isEmpty() -> "null"
            evalIndex < evals.size -> evals[evalIndex]
            else -> evals.last()
        }
        evalIndex++
        trace.evals.add(Eval(VirtualClock.now, js, reply))
        return reply
    }
}
