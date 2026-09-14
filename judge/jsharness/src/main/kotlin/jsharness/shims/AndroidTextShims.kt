@file:Suppress("unused")

package android.text

// 差分垫片:只用到 isEmpty / join(与 :harness 同实现)
object TextUtils {
    @JvmStatic
    fun isEmpty(str: CharSequence?): Boolean = str == null || str.isEmpty()

    @JvmStatic
    fun join(delimiter: CharSequence, tokens: Iterable<*>): String =
        tokens.joinToString(delimiter)
}
