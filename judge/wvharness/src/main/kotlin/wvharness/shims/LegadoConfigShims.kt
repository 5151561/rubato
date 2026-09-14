// 差分垫片:AppConfig —— 本套只读 userAgent(WebSettings 的 UA 兜底)。
// 与其余差分套同一约定:钉成裁判垫片的固定值,产品的真 UA 不从这里漏出去。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help.config

object AppConfig {
    var userAgent: String = "rubato-judge"
    var isNightTheme: Boolean = false
    var threadCount: Int = 16
}
