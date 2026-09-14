// 差分垫片:android.net(Uri / SslError)。
@file:Suppress("unused", "UNUSED_PARAMETER")

package android.net

/** 真身只用到 `request.url.toString()` */
class Uri(private val raw: String) {
    override fun toString(): String = raw
}
