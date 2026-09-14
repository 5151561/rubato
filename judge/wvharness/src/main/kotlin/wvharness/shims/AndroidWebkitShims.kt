// 差分垫片:`android.*` —— **确定性 WebView** 与虚拟时钟就在这里。
//
// 真 WebView 换成一份**剧本**([wvharness.Stage]):case 说清「加载这一页会依次
// 发生什么」,垫片照着回调 `WebViewClient`。所有回调与延时都走
// [wvharness.VirtualClock],不真的睡 —— 于是 `900 + 100`、重试梯子
// `200/400/600/800/1000`、`retry > 30` 与 60 s 超时**逐毫秒可比**,
// 而整套跑完不到一秒。契约:fixtures/cases/webview/README.md。
@file:Suppress("unused", "UNUSED_PARAMETER")

package android.webkit

import wvharness.Stage
import wvharness.VirtualClock

/**
 * **`fun interface` 不是随手写的**:真身 `evaluateJavascript(js) { ... }` 传的是
 * lambda,那在真机上靠 Java 接口的 SAM 转换成立。Kotlin 的普通 interface 没有
 * SAM 转换,写成普通 interface 会让真身那一行编不过 —— 也就等于**改了真身**。
 */
fun interface ValueCallback<T> {
    fun onReceiveValue(value: T)
}

/**
 * 写进来的**当场记进痕迹**:`createWebView()` 里那三行(blockNetworkImage / UA /
 * cacheMode)是真身的策略,而且**在 `load()` 之前就跑完了** —— 记在加载那一步会
 * 漏掉「url 为 null、根本没加载」那一档(真身在那一档照样设过 settings)。
 */
class WebSettings {
    var blockNetworkImage: Boolean = false
        set(v) { field = v; Stage.record().blockNetworkImage = v }
    var userAgentString: String? = null
        set(v) { field = v; Stage.record().ua = v }
    var cacheMode: Int = LOAD_DEFAULT
        set(v) { field = v; Stage.record().cacheMode = v }
    var javaScriptEnabled: Boolean = false

    companion object {
        const val LOAD_DEFAULT = -1
        const val LOAD_CACHE_ELSE_NETWORK = 1
    }
}

/** 页面加载时喂给 `shouldOverrideUrlLoading` 的那个请求 */
class WebResourceRequest(private val u: String, val isRedirect: Boolean) {
    val url: android.net.Uri = android.net.Uri(u)
}

class SslErrorHandler {
    var proceeded = false
        private set

    fun proceed() {
        proceeded = true
    }
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

class CookieManager private constructor() {
    /** 剧本给的那一串(`webview.cookie`);没给就是 null,和真机上「这个域没 cookie」一致 */
    fun getCookie(url: String?): String? = Stage.current?.cookie

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
 * 确定性 WebView。
 *
 * `loadUrl` / `loadDataWithBaseURL` **不同步回调** —— 真身那边它们只是把加载排进
 * 主线程队列,回调稍后才来。这里把整串事件按 delay 0 排进虚拟时钟队列,
 * 于是「加载完成」与 `postDelayed(runnable, 100 + delayTime)` 的先后关系
 * 与真机一致(前者先,后者后)。
 */
open class WebView {
    private val JS_SCHEME = "javascript:"
    val settings = WebSettings()
    var webViewClient: WebViewClient? = null
    var webChromeClient: WebChromeClient? = null

    /** 真身 `shouldOverrideUrlLoading` 的旧签名分支要读它(`request.url != view.url`) */
    var url: String? = null

    var resumed = false
        private set
    val jsInterfaces = linkedMapOf<String, Any>()

    fun onResume() {
        resumed = true
    }

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
        if (url.startsWith(JS_SCHEME)) {
            Stage.nextEval(url)
            return
        }
        startLoad(url)
    }

    fun loadUrl(url: String, headers: Map<String, String>) {
        Stage.record().requestHeaders = headers
        startLoad(url)
    }

    fun loadDataWithBaseURL(
        baseUrl: String?, data: String?, mimeType: String?, encoding: String?, historyUrl: String?
    ) {
        Stage.record().loadedHtml = data
        Stage.record().loadedEncoding = encoding
        startLoad(baseUrl ?: "about:blank")
    }

    fun evaluateJavascript(script: String, callback: ValueCallback<String>?) {
        val reply = Stage.nextEval(script)
        callback?.onReceiveValue(reply)
    }

    /**
     * 剧本回放:① shouldOverrideUrlLoading ② onLoadResource ③ onPageFinished。
     *
     * **每一步之前看一眼 WebView 还回去没有**:真身 `WebViewPool.release` 会
     * `stopLoading()` 并把 `webViewClient` 换成池子自己的 —— 还回去之后
     * BackstageWebView 的客户端再也收不到回调。嗅探命中(`onLoadResource`)
     * 之后就是这一档:后面的 `onPageFinished` **不该**再送进来
     * (漏了这一位,命中之后还会多抄一次 cookie —— 被测侧那边命中即返回)。
     */
    private fun startLoad(requestUrl: String) {
        this.url = requestUrl
        Stage.record().loadedUrl = requestUrl
        VirtualClock.post(0L, this) {
            val script = Stage.current ?: return@post
            val client = webViewClient ?: return@post
            for (o in script.overrideUrls) {
                if (!io.legado.app.help.webView.WebViewPool.inUse) return@post
                if (client.shouldOverrideUrlLoading(this, WebResourceRequest(o.url, o.isRedirect))) {
                    return@post
                }
            }
            for (r in script.loadResources) {
                if (!io.legado.app.help.webView.WebViewPool.inUse) return@post
                client.onLoadResource(this, r)
            }
            if (!script.pageFinished) return@post
            if (!io.legado.app.help.webView.WebViewPool.inUse) return@post
            val finished = script.pageFinishedUrl ?: requestUrl
            this.url = finished
            client.onPageFinished(this, finished)
            // **后来的**加载完成:跳转站会有第二次(甚至更多)。真身每来一次都
            // `removeCallbacks` + 重排 —— 这一位就是为了钉住那个
            for (p in script.laterPageFinished) {
                VirtualClock.post(p.at, this) {
                    if (!io.legado.app.help.webView.WebViewPool.inUse) return@post
                    val u = p.url ?: requestUrl
                    this.url = u
                    webViewClient?.onPageFinished(this, u)
                }
            }
        }
    }
}
