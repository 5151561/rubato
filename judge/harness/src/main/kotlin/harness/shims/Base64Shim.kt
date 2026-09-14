@file:Suppress("unused", "UNUSED_PARAMETER")

package android.util

// 差分垫片:android.util.Base64 → java.util.Base64(DEFAULT 等价于标准解码)
object Base64 {
    const val DEFAULT = 0
    const val NO_WRAP = 2

    @JvmStatic
    fun decode(str: String, flags: Int): ByteArray =
        java.util.Base64.getMimeDecoder().decode(str)

    @JvmStatic
    fun encodeToString(input: ByteArray, flags: Int): String =
        java.util.Base64.getEncoder().encodeToString(input)
}

// android.util.Log:真身 Debug.kt 在 BuildConfig.DEBUG=false 下不会调,空实现
@Suppress("unused", "UNUSED_PARAMETER")
object Log {
    @JvmStatic
    fun d(tag: String?, msg: String?): Int = 0
    @JvmStatic
    fun e(tag: String?, msg: String?): Int = 0
    @JvmStatic
    fun e(tag: String?, msg: String?, tr: Throwable?): Int = 0
}

// android.util.AndroidRuntimeException:BackstageWebView 的 `load()` 用
// @Throws 声明它(真机上 WebView 不在主线程建会抛)
@Suppress("unused")
class AndroidRuntimeException(message: String? = null) : RuntimeException(message)
