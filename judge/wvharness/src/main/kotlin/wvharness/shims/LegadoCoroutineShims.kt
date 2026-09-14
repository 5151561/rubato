// 差分垫片:help/coroutine/Coroutine —— **同步跑**。
//
// 真身用它把 `handleResult` 与 `setCookie` 甩到别的线程。本套的时间全是虚拟的,
// 线程切换只会带来不确定的交错;同步跑保住的是**顺序**:结果回调、cookie 回抄
// 都发生在触发它们的那一拍里,与真身在单核上的观察顺序一致。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help.coroutine

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.sync.Semaphore
import kotlin.coroutines.CoroutineContext

class Coroutine<T> {
    companion object {
        fun <T> async(
            scope: CoroutineScope = kotlinx.coroutines.GlobalScope,
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
