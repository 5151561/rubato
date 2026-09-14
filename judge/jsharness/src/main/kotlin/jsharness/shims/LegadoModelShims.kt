// 差分垫片:analyzeRule 包里**不挂真身**的那几个类型。
//
// **AnalyzeUrl 已不在此处** —— M2c 把真身挂了进来(build.gradle.kts 的 include),
// 网络面走 jsharness/ReplayHttp(HTTP 录放,契约 docs/http-snapshot.md)。
// 此前这里有一份 AnalyzeUrl 垫片:JS 宿主面(evalJS/put/get)逐字复制自真身,
// 网络面抛 `«net-unsupported»`。真身进来后那份复制作废 —— 真身的 evalJS 就是它。
//
// 剩下的仍是垫片:QueryTTF(字体反混淆属 Phase 3)。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.model.analyzeRule

/**
 * 差分标记:凡是走到**仍未接**的能力(webView / 压缩包 / 字体),两侧都收敛到这个串。
 * 网络本身已不在此列 —— 它走录放。
 */
const val NET_UNSUPPORTED = "«net-unsupported»"

/** QueryTTF 字体反混淆属 Phase 3(plan §4),类型面留桩 */
class QueryTTF(buffer: ByteArray) {
    fun getGlyfByUnicode(unicode: Int): String? = null
    fun getGlyfIdByUnicode(unicode: Int): Int = 0
    fun getUnicodeByGlyf(glyf: String?): Int = 0
    fun isBlankUnicode(unicode: Int): Boolean = false
}
