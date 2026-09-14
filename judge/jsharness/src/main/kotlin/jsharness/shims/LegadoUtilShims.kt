// 差分垫片:io.legado.app.utils 下 JsExtensions 触到、但绑死 Android/文件系统的面。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.utils

// LogUtils.printOnDebug:BuildConfig.DEBUG=false 下真身是空操作
fun Throwable.printOnDebug() = Unit

// utils/HandlerUtils.kt 的 runOnUI:直接跑。剧本 WebView 的内联泵已经把
// 「什么时候跑」接管了,再套一层线程只会引入不确定的交错。
inline fun runOnUI(crossinline block: () -> Unit) = block()
