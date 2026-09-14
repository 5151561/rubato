package android.text

// 差分垫片:AnalyzeRule / AnalyzeByXPath 只用到 isEmpty
object TextUtils {
    @JvmStatic
    fun isEmpty(str: CharSequence?): Boolean = str == null || str.isEmpty()

    @JvmStatic
    fun join(delimiter: CharSequence, tokens: Iterable<*>): String =
        tokens.joinToString(delimiter)
}
