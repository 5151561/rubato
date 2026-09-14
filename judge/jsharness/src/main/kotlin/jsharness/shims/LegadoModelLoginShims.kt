@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.model.login

// 登录 UI(loginUi 规则的渲染面)属砍单;BaseSource 只用它做类型转换
object LoginUiV2 {
    fun isV2(loginUi: String?): Boolean = false
    fun parse(json: String?): Any? = null
}
