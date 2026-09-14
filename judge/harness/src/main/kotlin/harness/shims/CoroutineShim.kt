// 差分垫片:help/coroutine/Coroutine —— **同步跑**。
//
// WebBook 的非 suspend 包装用得到它(harness 走 *Await 入口,不调);
// `BackstageWebView` 用它把 `handleResult` 与 `setCookie` 甩到别的线程 ——
// 那两处这里必须**就地**跑完:本套的时间全是虚拟的(harness.WvStage 的内联泵),
// 线程切换只会带来不确定的交错。同步跑保住的是**顺序**:结果回调、cookie 回抄
// 都发生在触发它们的那一拍里,与真身在单核上的观察顺序一致。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help.coroutine

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.GlobalScope
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.sync.Semaphore
import kotlin.coroutines.CoroutineContext

class Coroutine<T> {
    companion object {
        @OptIn(kotlinx.coroutines.DelicateCoroutinesApi::class)
        fun <T> async(
            scope: CoroutineScope = GlobalScope,
            context: CoroutineContext = Dispatchers.IO,
            start: CoroutineStart = CoroutineStart.DEFAULT,
            executeContext: CoroutineContext = Dispatchers.Main,
            semaphore: Semaphore? = null,
            block: suspend CoroutineScope.() -> T,
        ): Coroutine<T> {
            runBlocking { block() }
            return Coroutine()
        }
    }
}
