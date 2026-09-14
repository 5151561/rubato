// 差分垫片:help/config —— 真身绑死 SharedPreferences。取真身默认值。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help.config

object AppConfig {
    val userAgent: String = "rubato-judge"
    val isCronet: Boolean = false
    val threadCount: Int = 1

    // 以下取真身 PreferKey 默认值(与 :harness 的 HelpMiscShims 同口径)
    val replaceEnableDefault: Boolean = true
    val chineseConverterType: Int = 0
    val adaptSpecialStyle: Boolean = false
    val tocCountWords: Boolean = true
    val audioSkipOpenCredits: Int = 0
    val audioSkipCloseCredits: Int = 0

    // JsExtensions.getThemeMode:UI 面,砍单
    val themeMode: String? = "0"
}

object ReadBookConfig {
    val pageAnim: Int = 0
    val paragraphIndent: String = "　　"

    // JsExtensions.getReadBookConfig:阅读配置面属产品侧(真身是 Config 数据类),
    // 差分给空配置
    val durConfig: DurConfigStub = DurConfigStub()

    class DurConfigStub {
        fun toMap(): Map<String, Any> = emptyMap()
    }
}

object ThemeConfig {
    // JsExtensions.getThemeConfig/getThemeMode:UI 面,砍单(见 plan §6)
    val configList: List<Any> = emptyList()

    fun getDurConfig(context: Any?): Map<String, Any?> = emptyMap()
}
