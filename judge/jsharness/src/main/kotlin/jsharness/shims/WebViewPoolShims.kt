// 差分垫片:`io.legado.app.help.webView` 下 BackstageWebView 引到的三样。
//
// 同一个包里的 `WebViewRequestConfig.kt` 是**真身挂载**(UA 与请求头的取法
// 本身就是判据面,见 build.gradle.kts);这里的三样都不在观察面上:
// 池子、注入用的三个名字、以及池子里那个 WebView 的壳。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help.webView

import android.content.Context
import android.webkit.WebView
import io.legado.app.data.entities.BaseSource

class PooledWebView(val realWebView: WebView, val id: String) {
    var isInUse: Boolean = false
}

/**
 * 池子一律**新建**:复用与超时回收在这几套里没有投影,而复用会把上一个 case 的
 * WebView 状态带进下一个 —— 差分要的是每例独立。
 *
 * `acquire` / `release` 同时是**一次加载的起止**:剧本的时钟与游标在 acquire
 * 归零(真身那边 `withTimeout` 也是每次重新计时),release 之后剧本不再送事件
 * (真身 release 会 `stopLoading()` 并换掉 webViewClient)。
 */
object WebViewPool {
    var releaseCount = 0
        private set

    fun resetCounters() {
        releaseCount = 0
    }

    fun acquire(context: Context): PooledWebView {
        jsharness.WvStage.beginLoad()
        return PooledWebView(WebView(), "wv").apply { isInUse = true }
    }

    fun release(pooledWebView: PooledWebView) {
        if (!pooledWebView.isInUse) return
        pooledWebView.isInUse = false
        releaseCount++
        jsharness.WvStage.endLoad()
    }
}

/** 只有 `isRule = true` 那条路用得到(`@webjs:`);名字两侧钉成同一串 */
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
