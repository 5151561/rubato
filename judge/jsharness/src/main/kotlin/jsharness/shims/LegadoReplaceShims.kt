// 差分垫片:净化替换规则面(plan §6 砍单)+ DebugLog。
// 取自 :harness 的 UtilShims 同一份写法 —— 唯一差别是这里的 `eval` 是**真 Rhino**。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.utils

// utils/DebugLog.kt:空实现
object DebugLog {
    fun e(msg: String, e: Throwable? = null) = Unit
}

// (quoteReplacementJs 走真身 StringExtensions.kt L177,不在这里垫)

// utils/RegexExtensions.kt L28 的语义面:真身带超时看门狗 + 重启逻辑;
// 差分里同步执行
fun CharSequence.replace(
    name: String,
    regex: Regex,
    replacement: String,
    timeout: Long,
    chapter: io.legado.app.data.entities.BookChapter? = null,
    book: io.legado.app.data.entities.ReplaceBook? = null,
    includeContentInTimeoutMessage: Boolean = true,
): String {
    val isJs = replacement.startsWith("@js:")
    val replacement1 = if (isJs) replacement.substring(4) else replacement
    val pattern = regex.toPattern()
    val matcher = pattern.matcher(this)
    val stringBuffer = StringBuffer()
    while (matcher.find()) {
        if (isJs) {
            val jsResult = com.script.rhino.RhinoScriptEngine.run {
                val bindings = com.script.ScriptBindings()
                bindings["result"] = matcher.group()
                bindings["chapter"] = chapter
                bindings["book"] = book
                eval(replacement1, bindings)
            }.toString()
            matcher.appendReplacement(stringBuffer, jsResult.quoteReplacementJs())
        } else {
            matcher.appendReplacement(stringBuffer, replacement1)
        }
    }
    matcher.appendTail(stringBuffer)
    return stringBuffer.toString()
}
