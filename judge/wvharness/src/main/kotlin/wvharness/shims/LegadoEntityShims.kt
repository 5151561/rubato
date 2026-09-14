// 差分垫片:BaseSource —— 只作为 `addJavascriptInterface` 的形参类型出现。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.data.entities

interface BaseSource {
    fun getKey(): String
}
