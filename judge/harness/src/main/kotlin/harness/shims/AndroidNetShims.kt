// 差分垫片:android.net(Uri / SslError)—— 剧本 WebView 那条路要的两位。
@file:Suppress("unused", "UNUSED_PARAMETER")

package android.net

/** 真身只用到 `request.url.toString()` */
class Uri(private val raw: String) {
    override fun toString(): String = raw
}
