// 差分垫片:引擎里这些工具函数所在的文件带 Android/Room 依赖,纯 JVM harness
// 挂不进来。此处按 judge/engine 冻结源逐字复刻**被 AnalyzeRule 用到的那部分**,
// 复刻处标出原文件与行为;engine 本体一行未改。
@file:Suppress("unused")

package io.legado.app.utils

// utils/LogUtils.kt L140:BuildConfig.DEBUG 在裁判构建里为 false → 空实现
fun Throwable.printOnDebug() = Unit

// utils/ThrowableExtensions.kt L5
val Throwable.stackTraceStr: String
    get() {
        val stackTrace = stackTraceToString()
        val lMsg = this.localizedMessage ?: "noErrorMsg"
        return when {
            stackTrace.isNotEmpty() -> stackTrace
            else -> lMsg
        }
    }

// utils/StringExtensions.kt L37/L42/L47
fun String?.isAbsUrl() =
    this?.let { it.startsWith("http://", true) || it.startsWith("https://", true) } ?: false

private val dataUriRegex = Regex("^data:.*?;base64,(.*)")

fun String?.isDataUrl() = this?.let { dataUriRegex.matches(it) } ?: false

fun String?.isJson(): Boolean =
    this?.run {
        val str = this.trim()
        when {
            str.startsWith("{") && str.endsWith("}") -> true
            str.startsWith("[") && str.endsWith("]") -> true
            else -> false
        }
    } ?: false

// utils/StringExtensions.kt L88
fun String.splitNotBlank(regex: Regex, limit: Int = 0): Array<String> = run {
    this.split(regex, limit).map { it.trim() }.filterNot { it.isBlank() }.toTypedArray()
}

fun String.splitNotBlank(vararg delimiter: String, limit: Int = 0): Array<String> = run {
    this.split(*delimiter, limit = limit).map { it.trim() }.filterNot { it.isBlank() }.toTypedArray()
}



// utils/ContextExtensions.kt:差分永远在后台线程语义下跑
val isMainThread: Boolean get() = false

// utils/NetworkUtils.kt L166/L180 —— 逐行复刻(AppLog 换成空实现)
object NetworkUtils {

    fun getAbsoluteURL(baseURL: String?, relativePath: String): String {
        if (baseURL.isNullOrEmpty()) return relativePath.trim()
        var absoluteUrl: java.net.URL? = null
        try {
            absoluteUrl = java.net.URL(baseURL.substringBefore(","))
        } catch (e: Exception) {
            e.printOnDebug()
        }
        return getAbsoluteURL(absoluteUrl, relativePath)
    }

    fun getAbsoluteURL(baseURL: java.net.URL?, relativePath: String): String {
        val relativePathTrim = relativePath.trim()
        if (baseURL == null) return relativePathTrim
        if (relativePathTrim.isAbsUrl()) return relativePathTrim
        if (relativePathTrim.isDataUrl()) return relativePathTrim
        if (relativePathTrim.startsWith("javascript")) return ""
        var relativeUrl = relativePathTrim
        try {
            val parseUrl = java.net.URL(baseURL, relativePath)
            relativeUrl = parseUrl.toString()
            return relativeUrl
        } catch (e: Exception) {
            // AppLog.put("网址拼接出错\n${e.localizedMessage}", e)
        }
        return relativeUrl
    }

    // utils/NetworkUtils.kt L197
    fun getBaseUrl(url: String?): String? {
        url ?: return null
        if (url.startsWith("http://", true) || url.startsWith("https://", true)) {
            val index = url.indexOf("/", 9)
            return if (index == -1) url else url.substring(0, index)
        }
        return null
    }

    // utils/NetworkUtils.kt L243
    fun getDomain(url: String): String {
        val baseUrl = getBaseUrl(url) ?: return url
        return kotlin.runCatching { java.net.URL(baseUrl).host }.getOrDefault(baseUrl)
    }

    // utils/NetworkUtils.kt L217:PublicSuffixDatabase 走 okhttp internal
    fun getSubDomain(url: String): String {
        val baseUrl = getBaseUrl(url) ?: return url
        return kotlin.runCatching {
            val host: String = java.net.URL(baseUrl).host
            if (isIPAddress(host)) return host
            okhttp3.internal.publicsuffix.PublicSuffixDatabase.Companion.get()
                .getEffectiveTldPlusOne(host) ?: host
        }.getOrDefault(baseUrl)
    }

    private val ipv4Pattern =
        Regex("^(25[0-5]|2[0-4]\\d|[0-1]?\\d?\\d)\\.(25[0-5]|2[0-4]\\d|[0-1]?\\d?\\d)\\.(25[0-5]|2[0-4]\\d|[0-1]?\\d?\\d)\\.(25[0-5]|2[0-4]\\d|[0-1]?\\d?\\d)$")

    fun isIPAddress(input: String?): Boolean =
        !input.isNullOrEmpty() && (ipv4Pattern.matches(input) || input.contains(":"))

    private val notNeedEncodingQuery: java.util.BitSet by lazy {
        val bitSet = java.util.BitSet(256)
        for (i in 'a'.code..'z'.code) bitSet.set(i)
        for (i in 'A'.code..'Z'.code) bitSet.set(i)
        for (i in '0'.code..'9'.code) bitSet.set(i)
        for (char in "!$&()*+,-./:;=?@[\\]^_`{|}~") bitSet.set(char.code)
        bitSet
    }

    private val notNeedEncodingForm: java.util.BitSet by lazy {
        val bitSet = java.util.BitSet(256)
        for (i in 'a'.code..'z'.code) bitSet.set(i)
        for (i in 'A'.code..'Z'.code) bitSet.set(i)
        for (i in '0'.code..'9'.code) bitSet.set(i)
        for (char in "*-._") bitSet.set(char.code)
        bitSet
    }

    private fun isDigit16Char(c: Char): Boolean =
        c in '0'..'9' || c in 'A'..'F' || c in 'a'..'f'

    private fun encoded(str: String, set: java.util.BitSet): Boolean {
        var needEncode = false
        var i = 0
        while (i < str.length) {
            val c = str[i]
            if (set.get(c.code)) {
                i++
                continue
            }
            if (c == '%' && i + 2 < str.length) {
                val c1 = str[++i]
                val c2 = str[++i]
                if (isDigit16Char(c1) && isDigit16Char(c2)) {
                    i++
                    continue
                }
            }
            needEncode = true
            break
        }
        return !needEncode
    }

    fun encodedQuery(str: String): Boolean = encoded(str, notNeedEncodingQuery)

    fun encodedForm(str: String): Boolean = encoded(str, notNeedEncodingForm)
}

// utils/StringExtensions.kt L58/L64/L69
fun String?.isJsonObject(): Boolean =
    this?.run { trim().let { it.startsWith("{") && it.endsWith("}") } } ?: false

fun String?.isJsonArray(): Boolean =
    this?.run { trim().let { it.startsWith("[") && it.endsWith("]") } } ?: false

fun String?.isXml(): Boolean =
    this?.run { trim().let { it.startsWith("<") && it.endsWith(">") } } ?: false

// utils/StringExtensions.kt L156 —— 逐字复刻
fun String.escapeRegex(): String {
    return replace(io.legado.app.constant.AppPattern.regexCharRegex, "\\\\$0")
}

// utils/MapExtensions.kt:新基准下真身已挂载(get/has/getOrPutLimit),垫片撤销

// utils/NetworkUtils.kt 的 dnsIp 解析:差分不联网,恒 null
fun String.parseIpsFromString(): List<java.net.InetAddress>? = null

// utils/EncoderUtils.kt L11 —— 逐行复刻
object EncoderUtils {
    fun escape(src: String): String {
        val tmp = StringBuilder()
        for (char in src) {
            val charCode = char.code
            if (charCode in 48..57 || charCode in 65..90 || charCode in 97..122) {
                tmp.append(char)
                continue
            }
            val prefix = when {
                charCode < 16 -> "%0"
                charCode < 256 -> "%"
                else -> "%u"
            }
            tmp.append(prefix).append(charCode.toString(16))
        }
        return tmp.toString()
    }
}

// utils/StringUtils.kt:wordCountFormat / isNumeric(逐字复刻)
// utils/StringUtils.kt:真身已挂载(android.text/util/annotation 由垫片顶掉),桩撤销

// utils/StringExtensions.kt L75(逐字复刻)
fun String?.isTrue(nullIsTrue: Boolean = false): Boolean {
    if (this.isNullOrBlank() || this == "null") {
        return nullIsTrue
    }
    return !this.trim().matches("(?i)^(?:false|no|not|0|0.0)$".toRegex())
}


// utils/DebugLog.kt:空实现
object DebugLog {
    fun e(msg: String, e: Throwable? = null) = Unit
}

// ===== 基准切换(LegadoTeam)后新增的面 =====

// utils/StringExtensions.kt L177 逐字
fun String.quoteReplacementJs(): String {
    if (!this.contains('\\')) {
        return this
    }
    val sb = StringBuilder()
    for (c in this) {
        if (c == '\\') {
            sb.append("\\\\")
        } else {
            sb.append(c)
        }
    }
    return sb.toString()
}

// utils/ToastUtils.kt:差分无 UI,空实现
fun android.content.Context.toastOnUi(message: CharSequence?, duration: Int = 0) = Unit
fun android.content.Context.longToastOnUi(message: CharSequence?) = Unit

// utils/ChineseUtils.kt:真身拉 quick-transfer 词典;差分契约「简繁转换关」
// (AppConfig.chineseConverterType 恒 0),这里恒等映射,仅保编译面
object ChineseUtils {
    fun s2t(content: String): String = content
    fun t2s(content: String): String = content
    fun preLoad(async: Boolean, vararg transType: Any) = Unit
}

// utils/RegexExtensions.kt L28 的语义面:真身带超时看门狗 + 重启逻辑;
// 差分里同步执行(JS 分支走确定性桩,quoteReplacementJs 同真身)
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

// utils/HandlerUtils.kt 的 runOnUI:直接跑。剧本 WebView 的内联泵已经把
// 「什么时候跑」接管了,再套一层线程只会引入不确定的交错。
inline fun runOnUI(crossinline block: () -> Unit) = block()
