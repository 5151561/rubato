// 差分垫片:StringExtensions.cnCompare 用 android.icu 的 Collator。
// 差分不比较排序面,退化为 String.compareTo。
@file:Suppress("unused", "UNUSED_PARAMETER")

package android.icu.text

abstract class Collator {
    abstract fun compare(a: String?, b: String?): Int

    companion object {
        @JvmStatic
        fun getInstance(locale: android.icu.util.ULocale?): Collator = object : Collator() {
            override fun compare(a: String?, b: String?): Int = (a ?: "").compareTo(b ?: "")
        }
    }
}
