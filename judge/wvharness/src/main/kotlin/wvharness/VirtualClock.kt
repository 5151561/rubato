package wvharness

import java.util.PriorityQueue

/**
 * 虚拟时钟:`Handler.postDelayed` 与「加载完成」全排进这里,**不真的睡**。
 *
 * 为什么非它不可:本套要钉的策略**全是时间** —— `javaScript == null &&
 * delayTime == 0` 补 900 ms、`onPageFinished` 之后 `100 + delayTime` 起跑、
 * 取不到结果按 `200/400/600/800/1000` 重试、`retry > 30` 报「js执行超时」、
 * `withTimeout(timeout ?: 60000)`。真睡一遍要几十秒且不稳;虚拟时钟让这些
 * **逐毫秒可比**,整套跑完不到一秒。被测侧 `webview-compat` 有同名同语义的一份。
 *
 * 排序:先按到点时间,同刻按**入队序**(真身的 `Handler` 就是这个语义 ——
 * 同一时刻先 post 的先跑)。
 */
object VirtualClock {
    private class Entry(val at: Long, val seq: Long, val tag: Any?, val run: () -> Unit)

    private var seq = 0L
    private var q = PriorityQueue<Entry>(compareBy({ it.at }, { it.seq }))

    /** 当前虚拟时刻(毫秒) */
    var now = 0L
        private set

    fun reset() {
        now = 0
        seq = 0
        q = PriorityQueue(compareBy({ it.at }, { it.seq }))
    }

    fun post(delayMillis: Long, tag: Any?, run: () -> Unit) {
        q.add(Entry(now + maxOf(0L, delayMillis), seq++, tag, run))
    }

    /** `Handler.removeCallbacks`:按 runnable 身份摘掉还没到点的那些 */
    fun remove(tag: Any?) {
        q.removeIf { it.tag === tag }
    }

    /**
     * 推进时钟直到 [stop] 说停、队列空、或越过 [deadline]。
     *
     * 返回 true = **越过了 deadline**(对应真身 `withTimeout` 到点抛)。
     * 注意 deadline 的判定在**执行之前**:到点还没出结果就是超时,
     * 与 `withTimeout` 一致。
     */
    fun drain(deadline: Long, stop: () -> Boolean): Boolean {
        while (!stop()) {
            val e = q.poll() ?: return false
            if (e.at > deadline) {
                now = deadline
                return true
            }
            now = e.at
            e.run()
        }
        return false
    }
}
