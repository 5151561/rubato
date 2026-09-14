// 差分垫片:judge/rhino 真身触到的 android/androidx 面(仅两处)。
@file:Suppress("unused", "UNUSED_PARAMETER", "PackageDirectoryMismatch")

package android.os

// RhinoClassShutter 只看 SDK_INT 是否 >= O 来决定 java.nio.file 是否入白名单。
// 差分固定跑「>= O」路径(桌面 JVM 上 java.nio.file 本来就在)。
object Build {
    object VERSION {
        const val SDK_INT: Int = 34
    }

    object VERSION_CODES {
        const val N: Int = 24
        const val O: Int = 26
    }
}

// data/entities/rule/* 的 @Parcelize 标记接口
interface Parcelable

// icu4j CharsetDetector 的 setText(ParcelFileDescriptor) 重载(差分永不调用)
class ParcelFileDescriptor private constructor() {
    fun getFileDescriptor(): java.io.FileDescriptor = throw UnsupportedOperationException("差分桩")
}

// Looper 是个空壳;Handler 全转给剧本 WebView 的内联泵(jsharness.WvStage),
// 不真的睡 —— `postDelayed(runnable, 100 + delayTime)` 与重试梯子按**虚拟时间**
// 排序执行。`removeCallbacks` 按 runnable 身份摘,与真身一致。
class Looper private constructor() {
    companion object {
        private val main = Looper()

        @JvmStatic
        fun getMainLooper(): Looper = main
    }
}

class Handler(private val looper: Looper) {
    fun post(r: Runnable) = jsharness.WvStage.post(0L, r) { r.run() }
    fun postDelayed(r: Runnable, delayMillis: Long) =
        jsharness.WvStage.post(delayMillis, r) { r.run() }

    fun removeCallbacks(r: Runnable) = jsharness.WvStage.remove(r)
}
