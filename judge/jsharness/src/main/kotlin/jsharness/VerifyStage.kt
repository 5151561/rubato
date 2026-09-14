// 剧本「用户」:`java.startBrowserAwait` / `java.getVerificationCode` 那三件
// 要人出手,而差分里没有人。
//
// 真身那两个界面(`VerificationCodeActivity` / `WebViewActivity`)由
// `SourceVerificationHelp` 用 `appCtx.startActivity<…> { putExtra … }` 拉起来,
// 做完之后回头调 `SourceVerificationHelp.setResult(key, result, url)`。
// **这里顶掉的就是那一下**:`startActivity` 的垫片把 extras 交到这里,
// 这里照 case 的 `verify` 字段当场答一句 —— 于是等待那一侧第一圈就看见结果,
// 两侧都不必真等(与 webView 剧本「不真的睡」同一个道理)。
//
// 契约与被测侧那一份(`difftest::verify_script`)逐字同一份:
//
//     "verify": { "result": "<html>过了</html>", "url": "https://a.example/ok" }
//     "verify": { "close": true }      // 用户把界面关掉 → 「验证结果为空」
//
// **不带 `verify` 字段** = 这一侧没接界面:被测侧报「未接」,而裁判这边**什么
// 都不答** —— 那种 case 会把真身挂在轮询里,所以调这三件的 case 必须带剧本。
package jsharness

import io.legado.app.data.entities.BaseSource
import io.legado.app.help.config.AppConfig
import io.legado.app.help.source.SourceVerificationHelp
import io.legado.app.help.webView.toWebViewRequestConfig
import io.legado.app.model.analyzeRule.AnalyzeUrl
import kotlinx.coroutines.runBlocking

/** 一次 case 里「用户」的答复 */
class VerifyScript(
    val result: String? = null,
    val url: String? = null,
    /** 用户把界面关掉(真身 `checkResult`:塞空结果 → 「验证结果为空」) */
    val close: Boolean = false,
)

object VerifyStage {
    var script: VerifyScript? = null
        private set

    /** 观察面:界面弹了几次、每次带的是什么(与被测侧的 `opens` 对拍) */
    val opens = mutableListOf<Map<String, Any?>>()

    /**
     * 本 case 的书源。真身 `WebViewModel.initData` 是
     * `SourceHelp.getSource(sourceOrigin, sourceType)` 从库里取回来的同一个源
     * —— 差分里没有库,由 `Main` 每 case 交进来。
     */
    var source: BaseSource? = null

    fun resetCase(s: VerifyScript?) {
        script = s
        source = null
        opens.clear()
    }

    /**
     * **可见浏览器到底怎么加载这一页**:真身 `WebViewActivity` 被拉起来之后,
     * `WebViewModel.initData` 自己拆 `url,{…}`(`AnalyzeUrl`)、把 UA 从头里
     * 单拎出来(`toWebViewRequestConfig`,`CookieJar`/`proxy` 不许发给网站)、
     * POST 还先用 okhttp 那条路抓一份 HTML **覆盖**入参那份。
     *
     * 被测侧的界面在 Dart(算不了这些),同一份算在宿主网络面
     * (`js-host::net_face::browser_load`),随「弹界面」一起交给平台 ——
     * 于是这一面两侧比得了。
     *
     * 两处按差分口径归一(都写在 fixtures/cases/js-host/README.md):
     * 入参 html 不注入 `JS_INJECTION2`(页面里的 `java.*` 在砍单里);
     * `initData` 出错 = 交回 `null`(真身走 `onError` 弹 toast,界面照开)。
     */
    private fun browserLoad(extras: Map<String, Any?>): Map<String, Any?>? = try {
        val url = extras["url"] as String
        val analyzeUrl = AnalyzeUrl(mUrl = url, source = source)
        val config = analyzeUrl.headerMap.toWebViewRequestConfig(AppConfig.userAgent)
        var html = extras["html"] as? String
        if (analyzeUrl.isPost()) {
            html = runBlocking { analyzeUrl.getStrResponseAwait(useWebView = false).body }
        }
        mapOf(
            "url" to analyzeUrl.url,
            "userAgent" to config.userAgent,
            "headers" to config.additionalHeaders,
            "html" to html?.takeIf { it.isNotEmpty() },
        )
    } catch (_: Throwable) {
        null
    }

    /** `appCtx.startActivity<T> { putExtra … }` 落到这里 */
    fun opened(activity: String, extras: Map<String, Any?>) {
        val load = if (activity == "WebViewActivity") browserLoad(extras) else null
        opens.add(extras + mapOf("activity" to activity, "browserLoad" to load))
        // `startBrowser` 那条不等结果 —— 没有挂号 key,答无可答
        val key = extras["verificationResultKey"] as? String ?: return
        val s = script ?: return
        when {
            s.close -> SourceVerificationHelp.checkResult(key)
            s.result != null -> SourceVerificationHelp.setResult(key, s.result, s.url ?: "")
        }
    }
}
