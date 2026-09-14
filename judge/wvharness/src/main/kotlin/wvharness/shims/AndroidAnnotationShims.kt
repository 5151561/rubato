// 差分垫片:android.annotation.SuppressLint(纯注解,运行期无行为)。
@file:Suppress("unused")

package android.annotation

@Retention(AnnotationRetention.SOURCE)
annotation class SuppressLint(vararg val value: String)
