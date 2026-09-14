@file:Suppress("unused", "UNUSED_PARAMETER")

package android.icu.util

class ULocale(name: String) {
    companion object {
        @JvmField val SIMPLIFIED_CHINESE = ULocale("zh_CN")
        @JvmStatic fun forLanguageTag(t: String): ULocale = ULocale(t)
    }
}
