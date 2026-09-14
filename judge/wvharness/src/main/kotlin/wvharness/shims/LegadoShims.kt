// 差分垫片:BackstageWebView 真身引到的 legado 内部符号。
//
// 判据面上的两样**没有**做成垫片,而是挂了真身(见 build.gradle.kts):
// `WebViewRequestConfig`(UA 与请求头的取法)与 `StrResponse`(`url()` 走
// networkResponse / priorResponse 的取法)。这里剩下的都是**不在观察面上**的:
// 池子、UI 线程、协程包装、日志、缓存。
//
// `CookieStore` 是例外:它在观察面上,但**只观察「被调了什么」** ——
// 二级域名归一、库与 session 合并那些语义已经由 fetch 套逐字节钉过,
// 这里再挂一遍真身就要把 Room/appDb/NetworkUtils 一整串拖进来,
// 换回来的是同一件事被判两次。所以这里录调用,不复刻语义。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help.webView

import android.content.Context
import android.webkit.WebView
import io.legado.app.data.entities.BaseSource

class PooledWebView(val realWebView: WebView, val id: String) {
    var isInUse: Boolean = false
}

/**
 * 池子一律**新建**:复用与超时回收对本套的观察面没有投影,而复用会把
 * 「上一个 case 的 WebView 状态」带进下一个 case —— 差分要的是每例独立。
 * `release` 只记一笔,给输出里的 `released` 用(真身在拿到结果 / 报错 /
 * 超时三条路上都要还,漏还在真机上是泄漏一个 WebView)。
 */
object WebViewPool {
    var releaseCount = 0
        private set

    /** 手上这个 WebView 还没还回来吗 —— 剧本据此停止送事件(见 WebView.startLoad) */
    var inUse = false
        private set

    fun resetCounters() {
        releaseCount = 0
        inUse = false
    }

    fun acquire(context: Context): PooledWebView {
        inUse = true
        return PooledWebView(WebView(), "wv").apply { isInUse = true }
    }

    fun release(pooledWebView: PooledWebView) {
        if (!pooledWebView.isInUse) return
        pooledWebView.isInUse = false
        releaseCount++
        inUse = false
    }
}

/** 只有 `isRule = true` 那条路用得到;本套不覆盖(理由见 fixtures/cases/webview/README.md) */
class WebJsExtensions(source: BaseSource?, book: Any?, webView: WebView?) {
    companion object {
        const val nameJava = "__wvJava"
        const val nameCache = "__wvCache"
        const val nameSource = "__wvSource"
        const val nameBasic = "__wvBasic"
        const val getInjectionString =
            "try{var cache=$nameCache,source=$nameSource,java=$nameJava;}catch(e){}"
    }
}
