// 差分垫片:`android.os` —— Looper 是个空壳,Handler 全转给虚拟时钟。
@file:Suppress("unused", "UNUSED_PARAMETER")

package android.os

/** 真身只读 `SDK_INT >= N`(判 `request.isRedirect` 能不能用)。钉在 N 之上 */
object Build {
    object VERSION {
        const val SDK_INT = 34
    }

    object VERSION_CODES {
        const val N = 24
    }
}

class Looper private constructor() {
    companion object {
        private val main = Looper()

        @JvmStatic
        fun getMainLooper(): Looper = main
    }
}

/**
 * 全部转给 [wvharness.VirtualClock] —— 不睡,按虚拟时间排序执行。
 * `removeCallbacks` 按 **runnable 身份**摘,与真身一致(真身靠它把上一轮排队的
 * `EvalJsRunnable` 撤掉:`onPageFinished` 可能来好几次)。
 */
class Handler(private val looper: Looper) {
    fun post(r: Runnable) = wvharness.VirtualClock.post(0L, r) { r.run() }
    fun postDelayed(r: Runnable, delayMillis: Long) =
        wvharness.VirtualClock.post(delayMillis, r) { r.run() }

    fun removeCallbacks(r: Runnable) = wvharness.VirtualClock.remove(r)
}
