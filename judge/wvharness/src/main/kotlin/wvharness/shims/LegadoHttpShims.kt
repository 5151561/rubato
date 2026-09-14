// 差分垫片:help/http 下 BackstageWebView 引到的两样。
//
// `CookieStore` **在观察面上**,但这里只**录调用**、不复刻语义:二级域名归一、
// 库与 session 合并那一套已经由 fetch 套逐字节钉过(`net::cookie`),
// 在这儿再挂一遍真身要把 Room/appDb/NetworkUtils 一整串拖进来,
// 换回来的只是同一件事被判两次。本套要判的是**「页面加载完有没有把
// CookieManager 的 cookie 抄过来、抄的是哪个 key」**。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help.http

object CookieStore {
    /** (url/tag → cookie 串),按调用序 */
    val saved = linkedMapOf<String, String?>()

    fun setCookie(url: String, cookie: String?) {
        saved[url] = cookie
    }

    fun clear() = saved.clear()
}

object CookieManager {
    /** 真身:内部网络选项,**不许发给网站**(toWebViewRequestConfig 靠它过滤) */
    const val cookieJarHeader = "CookieJar"
}
