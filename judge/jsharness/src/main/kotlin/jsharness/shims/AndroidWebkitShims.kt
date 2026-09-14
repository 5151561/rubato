// 差分垫片:`android.webkit` —— **剧本 WebView** 就在这里。
//
// 三件事:
// 0. `@JavascriptInterface` 标记 + `WebSettings.getDefaultUserAgent`
//    (JsExtensions 的 `getWebViewUA()` 走它,差分固定 UA);
// 1. cookie 面(真身 CookieStore / CookieManagerExtensions 会触到)—— 空操作;
// 2. `BackstageWebView` 用的那套 WebView API —— 真 WebView 换成一份**剧本**
//    ([jsharness.WvStage]):case 说清「加载这一页会依次发生什么」,垫片照着
//    回调 `WebViewClient`。所有回调与延时都走 [jsharness.WvStage] 的内联泵,
//    不真的睡。契约:fixtures/cases/webview/README.md(被测侧 difftest::wv_script)。
@file:Suppress("unused", "UNUSED_PARAMETER")

package android.webkit

import jsharness.WvStage

/**
 * **`fun interface` 不是随手写的**:真身 `evaluateJavascript(js) { … }` 传的是
 * lambda,那在真机上靠 Java 接口的 SAM 转换成立。Kotlin 的普通 interface 没有
 * SAM 转换,写成普通 interface 会让真身那一行编不过 —— 也就等于**改了真身**。
 */
// JsExtensions 用 @JavascriptInterface 标注每个暴露给 JS 的方法(仅标记)
@Retention(AnnotationRetention.RUNTIME)
@Target(AnnotationTarget.FUNCTION, AnnotationTarget.PROPERTY_GETTER)
annotation class JavascriptInterface

fun interface ValueCallback<T> {
    fun onReceiveValue(value: T)
}

class CookieManager private constructor() {
    /** 剧本给的那一串(`webview.cookie`);没给就是 null,与真机上「这个域没 cookie」一致 */
    fun getCookie(url: String?): String? = WvStage.script.cookie

    fun setCookie(url: String?, value: String?) = Unit
    fun removeSessionCookies(callback: ValueCallback<Boolean>?) = Unit
    fun flush() = Unit

    companion object {
        private val instance = CookieManager()

        @JvmStatic
        fun getInstance(): CookieManager = instance
    }
}

/**
 * 写进来的**当场记进痕迹**:`createWebView()` 里那三行(blockNetworkImage / UA /
 * cacheMode)是真身的策略,而且在 `load()` 之前就跑完了。
 */
class WebSettings {
    var blockNetworkImage: Boolean = false
    var userAgentString: String? = null
    var cacheMode: Int = LOAD_DEFAULT
    var javaScriptEnabled: Boolean = false

    companion object {
        const val LOAD_DEFAULT = -1
        const val LOAD_CACHE_ELSE_NETWORK = 1

        // getWebViewUA() 走 WebSettings.getDefaultUserAgent(appCtx) —— 差分固定 UA,
        // 见 fixtures/cases/js-host/README.md「不可差分面」
        @JvmStatic
        fun getDefaultUserAgent(context: Any?): String = "rubato-difftest-ua"
    }
}

/** 页面加载时喂给 `shouldOverrideUrlLoading` 的那个请求 */
class WebResourceRequest(private val u: String, val isRedirect: Boolean) {
    val url: android.net.Uri = android.net.Uri.parse(u)
}

class SslErrorHandler {
    fun proceed() = Unit
}

open class WebViewClient {
    open fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean = false
    open fun shouldOverrideUrlLoading(view: WebView, url: String): Boolean = false
    open fun onPageFinished(view: WebView, url: String) = Unit
    open fun onLoadResource(view: WebView, url: String) = Unit
    open fun onReceivedSslError(
        view: WebView?,
        handler: SslErrorHandler?,
        error: android.net.http.SslError?
    ) = Unit
}

open class WebChromeClient {
    open fun onConsoleMessage(consoleMessage: ConsoleMessage): Boolean = false
}

class ConsoleMessage(private val msg: String, private val level: MessageLevel) {
    enum class MessageLevel { TIP, LOG, WARNING, ERROR, DEBUG }

    fun message(): String = msg
    fun messageLevel(): MessageLevel = level
}

/**
 * 剧本 WebView。
 *
 * `loadUrl` / `loadDataWithBaseURL` **不同步回调** —— 真身那边它们只是把加载排进
 * 主线程队列。这里把整串事件按 delay 0 排进 [WvStage] 的队列,于是「加载完成」
 * 与 `postDelayed(runnable, 100 + delayTime)` 的先后关系与真机一致。
 */
open class WebView {
    private val jsScheme = "javascript:"
    val settings = WebSettings()
    var webChromeClient: WebChromeClient? = null

    /** 真身 `shouldOverrideUrlLoading` 的旧签名分支要读它(`request.url != view.url`) */
    var url: String? = null

    var webViewClient: WebViewClient? = null
        set(v) {
            field = v
            // `withTimeout(timeout ?: 60000)` 的那个数:它是 BackstageWebView 的
            // 私有字段,而客户端是它的**内部类** —— 顺着 `this$0` 读回来。
            // (`AnalyzeUrl` 那条路是 60000,`AnalyzeRule.getWebJsResult` 是 10000,
            // 差着 3 倍;拿不到就按缺省。)
            WvStage.setDeadline(outerTimeout(v))
        }

    val jsInterfaces = linkedMapOf<String, Any>()

    fun onResume() = Unit
    fun onPause() = Unit
    fun stopLoading() = Unit
    fun destroy() = Unit
    fun clearHistory() = Unit
    fun removeJavascriptInterface(name: String) {
        jsInterfaces.remove(name)
    }

    fun addJavascriptInterface(obj: Any, name: String) {
        jsInterfaces[name] = obj
    }

    fun loadUrl(url: String) {
        // `javascript:` **不是一次页面加载**:真身的嗅探客户端就是拿它跑 JS 的
        // (`LoadJsRunnable`)。当成加载会把整段剧本重放一遍 —— 死循环。
        if (url.startsWith(jsScheme)) {
            WvStage.nextEval()
            return
        }
        startLoad(url)
    }

    fun loadUrl(url: String, headers: Map<String, String>) = startLoad(url)

    fun loadDataWithBaseURL(
        baseUrl: String?, data: String?, mimeType: String?, encoding: String?, historyUrl: String?
    ) = startLoad(baseUrl ?: "about:blank")

    fun evaluateJavascript(script: String, callback: ValueCallback<String>?) {
        val reply = WvStage.nextEval()
        callback?.onReceiveValue(reply)
    }

    /**
     * 剧本回放:① shouldOverrideUrlLoading ② onLoadResource ③ onPageFinished。
     *
     * **每一步之前看一眼 WebView 还回去没有**:真身 `WebViewPool.release` 会
     * `stopLoading()` 并把 `webViewClient` 换成池子自己的 —— 还回去之后
     * BackstageWebView 的客户端再也收不到回调。嗅探命中(`onLoadResource`)
     * 之后就是这一档:后面的 `onPageFinished` **不该**再送进来。
     */
    private fun startLoad(requestUrl: String) {
        this.url = requestUrl
        WvStage.post(0L, this) {
            val script = WvStage.script
            val client = webViewClient ?: return@post
            for (o in script.overrideUrls) {
                if (!WvStage.isLoading()) return@post
                if (client.shouldOverrideUrlLoading(
                        this, WebResourceRequest(o.url, o.isRedirect)
                    )
                ) {
                    return@post
                }
            }
            for (r in script.loadResources) {
                if (!WvStage.isLoading()) return@post
                client.onLoadResource(this, r)
            }
            if (!script.pageFinished || !WvStage.isLoading()) return@post
            val finished = script.pageFinishedUrl ?: requestUrl
            this.url = finished
            client.onPageFinished(this, finished)
            // **后来的**加载完成:跳转站会有第二次(甚至更多)。真身每来一次都
            // `removeCallbacks` + 重排 —— 这一位就是为了钉住那个
            for (p in script.laterPageFinished) {
                WvStage.post(p.at, this) {
                    if (!WvStage.isLoading()) return@post
                    val u = p.url ?: requestUrl
                    this.url = u
                    webViewClient?.onPageFinished(this, u)
                }
            }
        }
    }

    private fun outerTimeout(client: Any?): Long {
        val outer = client?.javaClass?.declaredFields
            ?.firstOrNull { it.name == "this\$0" }
            ?.also { it.isAccessible = true }
            ?.get(client) ?: return WvStage.DEFAULT_TIMEOUT
        return runCatching {
            outer.javaClass.getDeclaredField("timeout")
                .also { it.isAccessible = true }
                .get(outer) as? Long
        }.getOrNull() ?: WvStage.DEFAULT_TIMEOUT
    }
}
