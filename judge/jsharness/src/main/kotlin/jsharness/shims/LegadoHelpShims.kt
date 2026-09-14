// 差分垫片:help/ 下 JsExtensions / BaseSource 触到的对象。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help

import io.legado.app.data.entities.BaseSource

// 真身经 cacheDao 持久化;差分内存表,语义面一致(与 :harness 同实现)
object CacheManager {
    private val memory = HashMap<String, Any>()
    private val cache = HashMap<String, String>()

    // `putFile/getFile` 打的是 **ACache**(真身 CacheManager.kt L151/L155,落盘),
    // 与 `put/get` 的 cacheDao + 内存表**不是同一张** —— `put` 写的 `getFile`
    // 读不到。此前这个垫片整支缺席,于是书源写 `cache.getFile(k)` 在裁判这边
    // 直接 TypeError(js-corpus-02f6203511 / 90341bf923:「找不到函数 getFile」),
    // 而真身跑得通 —— **是垫片的契约缺口,不是被测侧的实现缺口**。
    private val files = HashMap<String, String>()

    @JvmOverloads
    fun put(key: String, value: Any, saveTime: Int = 0) {
        cache[key] = value.toString()
    }

    fun get(key: String): String? = cache[key]
    fun getInt(key: String): Int? = cache[key]?.toIntOrNull()
    fun getLong(key: String): Long? = cache[key]?.toLongOrNull()
    fun getDouble(key: String): Double? = cache[key]?.toDoubleOrNull()
    fun getFloat(key: String): Float? = cache[key]?.toFloatOrNull()
    fun getByteArray(key: String): ByteArray? = cache[key]?.toByteArray()
    fun putByteArray(key: String, value: ByteArray, saveTime: Int = 0) {
        cache[key] = String(value)
    }
    fun delete(key: String) {
        cache.remove(key)
        memory.remove(key)
        files.remove(key)
    }

    @JvmOverloads
    fun putFile(key: String, value: String, saveTime: Int = 0) {
        files[key] = value
    }

    fun getFile(key: String): String? = files[key]
    fun getFromMemory(key: String): Any? = memory[key]
    fun putMemory(key: String, value: Any) {
        memory[key] = value
    }
    fun deleteMemory(key: String) {
        memory.remove(key)
    }

    // 差分观察面,真身没有
    fun snapshot(): Map<String, String> = cache.toSortedMap()

    // ACache 那张的终态。与 snapshot() 分开出字段(`cacheFile`)——
    // 两张表在真身里就是两处存储,混一张会把「写内存、读磁盘」照成一致。
    fun fileSnapshot(): Map<String, String> = files.toSortedMap()

    fun clearAll() {
        cache.clear()
        memory.clear()
        files.clear()
    }
}

// 真身是 ACache 落盘;差分内存表
object AppCacheManager {
    private val map = HashMap<String, String>()

    @JvmOverloads
    fun put(key: String, value: Any, saveTime: Int = 0) {
        map[key] = value.toString()
    }

    fun get(key: String): String? = map[key]
    fun getByteArray(key: String): ByteArray? = map[key]?.toByteArray()
    fun putByteArray(key: String, value: ByteArray, saveTime: Int = 0) {
        map[key] = String(value)
    }
    // QueryTTF 字体缓存属 Phase 3,永不命中
    fun getQueryTTF(key: String): io.legado.app.model.analyzeRule.QueryTTF? = null
    fun delete(key: String) {
        map.remove(key)
    }
}

// JsExtensions 的 singleFlight/lock/tick(源级并发闸):差分单线程,闸门直通
object SourceLock {
    fun <T> singleFlight(key: String, timeoutMs: Long, block: () -> T): T = block()
    fun <T> lock(key: String, timeoutMs: Long, block: () -> T): T = block()
    fun tick(key: String): Int = 0
}

// 并发限流:差分里直接放行
class ConcurrentRateLimiter(source: BaseSource?) {
    suspend inline fun <T> withLimit(block: () -> T): T = block()
    inline fun <T> withLimitBlocking(block: () -> T): T = block()

    companion object {
        // 真身 ConcurrentRateLimiter.kt L16
        fun updateConcurrentRate(key: String, concurrentRate: String) = Unit
    }
}

// help/WebCacheManager.kt:`isRule = true` 时注进页面的那个缓存对象
// (`window.result = __wvCache.getFromMemory('webview_result')` 读它)。
@Suppress("unused")
object WebCacheManager {
    fun getFromMemory(key: String): String? = CacheManager.getFromMemory(key)?.toString()
}
