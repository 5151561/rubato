@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help.config

object AppConfig {
    val userAgent: String = "rubato-judge"
    val isCronet: Boolean = false
    // 净化/简繁替换被砍:默认关
    val replaceEnableDefault: Boolean = true
    val chineseConverterType: Int = 0
    val adaptSpecialStyle: Boolean = false
    // 真身默认 32;差分要请求序列确定,mapAsync(1) 退化为顺序 map
    val threadCount: Int = 1
    // 真身 PreferKey.tocCountWords 默认 true
    val tocCountWords: Boolean = true
    // 真身 PreferKey.audioSkip* 默认 0
    val audioSkipOpenCredits: Int = 0
    val audioSkipCloseCredits: Int = 0
}

// help/config/ReadBookConfig.kt:真身绑死 SharedPreferences + 主题配置文件,
// 只取 Book/ContentProcessor 触到的两项(值 = 真身 Config 的默认值)
object ReadBookConfig {
    // Config.pageAnim 默认 0(仿真翻页)
    val pageAnim: Int = 0
    // Config.paragraphIndent 默认两个全角空格
    val paragraphIndent: String = "\u3000\u3000"
}
