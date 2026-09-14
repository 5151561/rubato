// 差分垫片:Debug.log —— 本套把它录下来(webChromeClient 的 console 转发走它)。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.model

object Debug {
    val logs = mutableListOf<String>()

    fun log(sourceUrl: String?, msg: String? = null, print: Boolean = true) {
        msg?.let { logs.add(it) }
    }

    fun clear() = logs.clear()
}
