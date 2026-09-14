// 差分垫片:icu4j CharsetDetector 的 setText(ParcelFileDescriptor) 重载
// 触到的 android.os / android.system 面。该重载 harness 永不调用,全部抛。
@file:Suppress("unused", "UNUSED_PARAMETER", "PackageDirectoryMismatch")

package android.os

import java.io.FileDescriptor

class ParcelFileDescriptor private constructor() {
    fun getFileDescriptor(): FileDescriptor = throw UnsupportedOperationException("差分桩")
}

// android.os.Parcelable:标记接口即可(Parcelize 插件不参与差分构建)
interface Parcelable

// android.os.Build:ContentProcessor 只看 SDK_INT 是否落在 26..27(Android 8 的
// \u00A0 修补分支)。差分固定跑「非 Android 8」路径,被测侧对齐。
object Build {
    object VERSION {
        const val SDK_INT: Int = 34
    }

    // BackstageWebView 只读 `SDK_INT >= N`(判 `request.isRedirect` 能不能用)
    object VERSION_CODES {
        const val N = 24
    }
}

// Looper 是个空壳;Handler 全转给剧本 WebView 的内联泵(harness.WvStage),
// 不真的睡 —— `postDelayed(runnable, 100 + delayTime)` 与重试梯子按**虚拟时间**
// 排序执行。`removeCallbacks` 按 runnable 身份摘,与真身一致(真身靠它把上一轮
// 排队的 EvalJsRunnable 撤掉:onPageFinished 可能来好几次)。
class Looper private constructor() {
    companion object {
        private val main = Looper()

        @JvmStatic
        fun getMainLooper(): Looper = main
    }
}

class Handler(private val looper: Looper) {
    fun post(r: Runnable) = harness.WvStage.post(0L, r) { r.run() }
    fun postDelayed(r: Runnable, delayMillis: Long) =
        harness.WvStage.post(delayMillis, r) { r.run() }

    fun removeCallbacks(r: Runnable) = harness.WvStage.remove(r)
}
