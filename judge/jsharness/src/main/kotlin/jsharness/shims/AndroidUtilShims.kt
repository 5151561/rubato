// 差分垫片:android.util.Base64 / Log。
//
// **保真说明**:Base64 是 js-host 差分的**判据面**(书源里 base64Encode/Decode
// 带 flags 的调用不少),不能像 :harness 那样只做 DEFAULT 等价。这里按 AOSP
// android.util.Base64 的 flag 语义复刻:
//   DEFAULT=0 NO_PADDING=1 NO_WRAP=2 CRLF=4 URL_SAFE=8 NO_CLOSE=16
// 解码永远宽松(忽略换行与 padding 缺失);URL_SAFE 用 -_ 字母表。
// 已知未复刻:AOSP 解码器对非法字符抛 IllegalArgumentException 的**时机**
// (它是流式的,可能在读到非法字符前就返回)——落差分豁免,见
// fixtures/cases/js-host/README.md。
@file:Suppress("unused", "UNUSED_PARAMETER")

package android.util

object Base64 {
    const val DEFAULT = 0
    const val NO_PADDING = 1
    const val NO_WRAP = 2
    const val CRLF = 4
    const val URL_SAFE = 8
    const val NO_CLOSE = 16

    @JvmStatic
    fun decode(str: String, flags: Int): ByteArray = decode(str.toByteArray(Charsets.UTF_8), flags)

    @JvmStatic
    fun decode(input: ByteArray, flags: Int): ByteArray {
        // AOSP 的解码器不分 URL_SAFE:两套字母表都认(见 Base64.decode 的 DECODE 表)
        val cleaned = String(input, Charsets.ISO_8859_1)
            .replace("\r", "").replace("\n", "")
            .replace('-', '+').replace('_', '/')
        val padded = when (cleaned.length % 4) {
            2 -> "$cleaned=="
            3 -> "$cleaned="
            else -> cleaned
        }
        return java.util.Base64.getMimeDecoder().decode(padded)
    }

    @JvmStatic
    fun encodeToString(input: ByteArray, flags: Int): String =
        String(encode(input, flags), Charsets.ISO_8859_1)

    @JvmStatic
    fun encode(input: ByteArray, flags: Int): ByteArray {
        var enc = if (flags and URL_SAFE != 0) {
            java.util.Base64.getUrlEncoder()
        } else {
            java.util.Base64.getEncoder()
        }
        if (flags and NO_PADDING != 0) enc = enc.withoutPadding()
        val body = enc.encodeToString(input)
        if (flags and NO_WRAP != 0) return body.toByteArray(Charsets.ISO_8859_1)
        // AOSP 默认每 76 字符断行,行尾 \n(带 CRLF 则 \r\n),**末尾也有换行**
        val nl = if (flags and CRLF != 0) "\r\n" else "\n"
        val sb = StringBuilder()
        var i = 0
        while (i < body.length) {
            val end = minOf(i + 76, body.length)
            sb.append(body, i, end).append(nl)
            i = end
        }
        return sb.toString().toByteArray(Charsets.ISO_8859_1)
    }
}

// BuildConfig.DEBUG=false 下真身不会调,空实现
object Log {
    @JvmStatic fun d(tag: String?, msg: String?): Int = 0
    @JvmStatic fun e(tag: String?, msg: String?): Int = 0
    @JvmStatic fun e(tag: String?, msg: String?, tr: Throwable?): Int = 0
    @JvmStatic fun i(tag: String?, msg: String?): Int = 0
}

// android.util.AndroidRuntimeException:BackstageWebView 的 `load()` 用
// @Throws 声明它(真机上 WebView 不在主线程建会抛)
@Suppress("unused")
class AndroidRuntimeException(message: String? = null) : RuntimeException(message)
