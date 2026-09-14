// 纯 JVM harness 的最小 stub:引擎源文件引用的 androidx 注解
package androidx.annotation

annotation class Keep
annotation class NonNull
annotation class Nullable
annotation class IntDef(vararg val value: Int, val flag: Boolean = false, val open: Boolean = false)
