// 差分垫片:AnalyzeUrl 里仅剩的两处 android.* 引用
@file:Suppress("unused")

package android.annotation

@Retention(AnnotationRetention.SOURCE)
annotation class SuppressLint(vararg val value: String)
