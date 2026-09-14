// 差分垫片:BaseSource 接口(真身 data/entities/BaseSource.kt 继承完整
// JsExtensions 且拉 AES/登录 UI/JS 书源引擎,挂不进纯 JVM)。
// 此处按 LegadoTeam 真身逐字复刻**被挂载文件触到的成员**:
// getHeaderMap / put / get / putVariable / getVariable / getLoginJs /
// evalJS(走确定性 JS 桩)。BookSource 真身已按原路径挂载,不再是垫片。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.data.entities

import com.script.ScriptBindings
import com.script.buildScriptBindings
import com.script.rhino.RhinoScriptEngine
import io.legado.app.constant.AppConst
import io.legado.app.constant.AppLog
import io.legado.app.constant.AppPattern.JS_PATTERN
import io.legado.app.help.CacheManager
import io.legado.app.help.JsExtensions
import io.legado.app.help.config.AppConfig
import io.legado.app.help.http.CookieStore
import io.legado.app.model.analyzeRule.RuleDataInterface
import io.legado.app.utils.GSON
import io.legado.app.utils.fromJsonObject
import io.legado.app.utils.has

interface BaseSource : JsExtensions {
    var concurrentRate: String?
    var loginUrl: String?
    var loginUi: String?
    var header: String?
    var enabledCookieJar: Boolean?
    var jsLib: String?

    override fun getTag(): String

    fun getKey(): String

    override fun getSource(): BaseSource? {
        return this
    }

    private fun extractInlineJs(rule: String?): String? {
        val text = rule?.trim().orEmpty()
        if (text.isBlank()) return null
        val matcher = JS_PATTERN.matcher(text)
        if (!matcher.matches()) return null
        return (matcher.group(1) ?: matcher.group(2))?.trim()
    }

    fun getLoginJs(): String? {
        val loginRule = loginUrl?.trim()
        if (loginRule.isNullOrBlank()) return null
        return extractInlineJs(loginRule) ?: loginRule
    }

    /**
     * 解析header规则(真身逐字;JS 头走确定性桩)
     */
    fun getHeaderMap(hasLoginHeader: Boolean = false) = HashMap<String, String>().apply {
        header?.let {
            try {
                val json = extractInlineJs(it)?.let { js ->
                    normalizeJsResult(evalJS(js)).orEmpty()
                } ?: it
                GSON.fromJsonObject<Map<String, String>>(json).getOrNull()?.let { map ->
                    putAll(map)
                }
            } catch (e: Exception) {
                AppLog.put("执行请求头规则出错\n$e", e)
            }
        }
        if (!has(AppConst.UA_NAME, true)) {
            put(AppConst.UA_NAME, AppConfig.userAgent)
        }
        if (hasLoginHeader) {
            getLoginHeaderMap()?.let {
                putAll(it)
            }
        }
    }

    /**
     * 真身 JsSourceEngine.normalizeJsResult(L129):null/Undefined → null,
     * String/CharSequence → 原串,其余 → GSON.toJson。JsSourceEngine 整体
     * 绑死 JS 书源引擎挂不进来,这里按桩 JS 能产出的类型复刻。
     */
    private fun normalizeJsResult(result: Any?): String? = when (result) {
        null -> null
        is String -> result
        is CharSequence -> result.toString()
        else -> GSON.toJson(result)
    }

    fun getLoginHeader(): String? {
        return CacheManager.get("loginHeader_${getKey()}")
    }

    fun getLoginHeaderMap(): Map<String, String>? {
        val cache = getLoginHeader() ?: return null
        return GSON.fromJsonObject<Map<String, String>>(cache).getOrNull()
    }

    fun putLoginHeader(header: String) {
        val headerMap = GSON.fromJsonObject<Map<String, String>>(header).getOrNull()
        val cookie = headerMap?.get("Cookie") ?: headerMap?.get("cookie")
        cookie?.let {
            CookieStore.replaceCookie(getKey(), it)
        }
        CacheManager.put("loginHeader_${getKey()}", header)
    }

    fun removeLoginHeader() {
        CacheManager.delete("loginHeader_${getKey()}")
        CookieStore.removeCookie(getKey())
    }

    fun putVariable(variable: String?) {
        if (variable != null) {
            CacheManager.put("sourceVariable_${getKey()}", variable)
        } else {
            CacheManager.delete("sourceVariable_${getKey()}")
        }
    }

    fun getVariable(): String {
        return CacheManager.get("sourceVariable_${getKey()}") ?: ""
    }

    fun put(key: String, value: String): String {
        CacheManager.put("v_${getKey()}_${key}", value)
        return value
    }

    fun get(key: String): String {
        return CacheManager.get("v_${getKey()}_${key}") ?: ""
    }

    /**
     * 执行JS(真身逐字,scope 机制由 com.script 桩承接)
     */
    @Throws(Exception::class)
    fun evalJS(jsStr: String, bindingsConfig: ScriptBindings.() -> Unit = {}): Any? {
        val bindings = buildScriptBindings { bindings ->
            bindings["java"] = this
            bindings["source"] = this
            bindings["sourceApi"] = this
            bindings["baseUrl"] = getKey()
            bindings["cookie"] = CookieStore
            bindings["cache"] = CacheManager
            bindings.apply(bindingsConfig)
        }
        val scope = RhinoScriptEngine.getRuntimeScope(bindings)
        return RhinoScriptEngine.eval(jsStr, scope)
    }
}

open class SimpleRuleData : RuleDataInterface {
    override val variableMap: HashMap<String, String> = hashMapOf()
    private val big = HashMap<String, String>()
    override fun putBigVariable(key: String, value: String?) {
        if (value == null) big.remove(key) else big[key] = value
    }
    override fun getBigVariable(key: String): String? = big[key]
}

class RssArticle : SimpleRuleData() {
    var title: String = ""
}

// data/entities/RssSource.kt 的最小面(BaseSourceExtensions.getSourceType 触到)
class RssSource : SimpleRuleData(), BaseSource {
    var sourceUrl: String = ""
    var sourceName: String = ""
    override var concurrentRate: String? = null
    override var loginUrl: String? = null
    override var loginUi: String? = null
    override var header: String? = null
    override var enabledCookieJar: Boolean? = false
    override var jsLib: String? = null
    override fun getTag(): String = sourceName
    override fun getKey(): String = sourceUrl
}

// data/entities/Cookie.kt(真身带 Room 注解)
class Cookie(var url: String = "", var cookie: String = "")
