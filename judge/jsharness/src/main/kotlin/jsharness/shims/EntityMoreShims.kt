// 差分垫片:AnalyzeRule / BaseSourceExtensions 触到的 RSS 侧实体(RSS 属砍单,
// plan §6),以及 Room 实体 Cookie。取自 :harness 的 EntityShims 同一份写法。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.data.entities

import io.legado.app.model.analyzeRule.RuleDataInterface

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
