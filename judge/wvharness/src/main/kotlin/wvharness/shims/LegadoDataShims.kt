// 差分垫片:appDb / BaseSource —— 只有 `isRule = true` 那条路会碰它们。
// `getBookSource` 一律给 null:本套不覆盖注入 WebJsExtensions 的那一档
// (理由见 fixtures/cases/webview/README.md)。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.data

object BookSourceDao {
    fun getBookSource(key: String): io.legado.app.data.entities.BaseSource? = null
}

object appDb {
    val bookSourceDao = BookSourceDao
}
