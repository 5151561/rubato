// 差分垫片:help/ 下 AnalyzeRule 触及的对象。JsExtensions 只留 AnalyzeRule
// 实际 override / 调用的那几个成员(真身 1199 行,全是 Android/OkHttp 依赖)。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help

import io.legado.app.data.entities.BaseSource

interface JsExtensions {
    fun getSource(): BaseSource?
    fun getTag(): String?
    // 给默认实现,AnalyzeUrl 不 override 也能编译(AnalyzeRule 会 override)
    fun ajax(url: Any): String? = null
    fun ajax(url: Any, callTimeout: Long?): String? = null
    fun log(msg: Any?): Any? = msg
}

// 并发限流:差分里直接放行
class ConcurrentRateLimiter(source: BaseSource?) {
    suspend inline fun <T> withLimit(block: () -> T): T = block()
}

object CacheManager {
    private val memory = HashMap<String, Any>()
    private val cache = HashMap<String, String>()

    // 真身经 cacheDao 持久化;差分内存表,语义面一致
    @JvmOverloads
    fun put(key: String, value: Any, saveTime: Int = 0) {
        cache[key] = value.toString()
    }
    fun get(key: String): String? = cache[key]
    fun delete(key: String) {
        cache.remove(key)
    }
    fun cacheSnapshot(): Map<String, String> = cache.toSortedMap()
    fun clearCache() = cache.clear()
    fun getFromMemory(key: String): Any? = memory[key]
    fun putMemory(key: String, value: Any) {
        memory[key] = value
    }
    fun deleteMemory(key: String) {
        memory.remove(key)
    }

    // 下面两个是 harness 的差分观察面,真身没有
    fun memorySnapshot(): Map<String, Any> = memory.toMap()
    fun clearMemory() = memory.clear()
}

// help/WebCacheManager.kt:`isRule = true` 时注进页面的那个缓存对象
// (`window.result = __wvCache.getFromMemory('webview_result')` 读它)。
// 差分侧转给 CacheManager 的内存表 —— 与真身同一张。
@Suppress("unused")
object WebCacheManager {
    fun getFromMemory(key: String): String? = CacheManager.getFromMemory(key)?.toString()
}
