// 差分垫片:runOnUI —— 直接跑。虚拟时钟已经把「什么时候跑」这件事接管了,
// 再套一层线程只会引入不确定的交错。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.utils

inline fun runOnUI(crossinline block: () -> Unit) = block()
