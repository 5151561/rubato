// 差分垫片:GsonExtensions 的真身要拉 6 个 rule 实体的自定义反序列化器,
// 这里按其 INITIAL_GSON 配置复刻 AnalyzeRule 用到的那部分
// (@put 的 Map<String,String> 解析、@webjs 的 List<String> 解析)。
@file:Suppress("unused")

package io.legado.app.utils

import com.google.gson.Gson
import com.google.gson.GsonBuilder
import com.google.gson.JsonDeserializationContext
import com.google.gson.JsonDeserializer
import com.google.gson.JsonElement
import com.google.gson.JsonSyntaxException
import com.google.gson.Strictness
import com.google.gson.ToNumberPolicy
import com.google.gson.reflect.TypeToken
import java.lang.reflect.Type

// utils/GsonExtensions.kt L84
class StringJsonDeserializer : JsonDeserializer<String?> {
    override fun deserialize(
        json: JsonElement,
        typeOfT: Type,
        context: JsonDeserializationContext?
    ): String? = when {
        json.isJsonPrimitive -> json.asString
        json.isJsonNull -> null
        else -> json.toString()
    }
}

class IntJsonDeserializer : JsonDeserializer<Int?> {
    override fun deserialize(
        json: JsonElement,
        typeOfT: Type?,
        context: JsonDeserializationContext?
    ): Int? = when {
        json.isJsonPrimitive -> json.asJsonPrimitive.let { if (it.isNumber) it.asNumber.toInt() else null }
        else -> null
    }
}

val INITIAL_GSON: Gson by lazy {
    GsonBuilder()
        .registerTypeAdapter(Int::class.java, IntJsonDeserializer())
        .registerTypeAdapter(String::class.java, StringJsonDeserializer())
        .setObjectToNumberStrategy(ToNumberPolicy.LONG_OR_DOUBLE)
        .disableHtmlEscaping()
        .setPrettyPrinting()
        .create()
}

// 真身 GSON = INITIAL_GSON + rule 实体反序列化器(ReviewRule 被砍,字段不进垫片)
val GSON: Gson by lazy {
    INITIAL_GSON.newBuilder()
        .registerTypeAdapter(
            io.legado.app.data.entities.rule.ExploreRule::class.java,
            io.legado.app.data.entities.rule.ExploreRule.jsonDeserializer
        )
        .registerTypeAdapter(
            io.legado.app.data.entities.rule.SearchRule::class.java,
            io.legado.app.data.entities.rule.SearchRule.jsonDeserializer
        )
        .registerTypeAdapter(
            io.legado.app.data.entities.rule.BookInfoRule::class.java,
            io.legado.app.data.entities.rule.BookInfoRule.jsonDeserializer
        )
        .registerTypeAdapter(
            io.legado.app.data.entities.rule.TocRule::class.java,
            io.legado.app.data.entities.rule.TocRule.jsonDeserializer
        )
        .registerTypeAdapter(
            io.legado.app.data.entities.rule.ContentRule::class.java,
            io.legado.app.data.entities.rule.ContentRule.jsonDeserializer
        )
        .create()
}

val GSONStrict: Gson by lazy { GSON.newBuilder().setStrictness(Strictness.STRICT).create() }

inline fun <reified T> genericType(): Type = object : TypeToken<T>() {}.type

inline fun <reified T> Gson.fromJsonObject(json: String?): Result<T> = kotlin.runCatching {
    if (json == null) throw JsonSyntaxException("解析字符串为空")
    fromJson(json, genericType<T>()) as T
}

inline fun <reified T> Gson.fromJsonArray(json: String?): Result<List<T>> = kotlin.runCatching {
    if (json == null) throw JsonSyntaxException("解析字符串为空")
    val type = TypeToken.getParameterized(List::class.java, T::class.java).type
    val list = fromJson(json, type) as List<T?>
    @Suppress("UNCHECKED_CAST")
    list.filterNotNull() as List<T>
}
