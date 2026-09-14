@file:Suppress("unused")

package androidx.annotation

@Retention(AnnotationRetention.SOURCE)
annotation class IntDef(vararg val value: Int = [], val flag: Boolean = false, val open: Boolean = false)

@Retention(AnnotationRetention.SOURCE)
annotation class StringDef(vararg val value: String = [], val open: Boolean = false)
