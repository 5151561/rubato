// 差分垫片:io.legado.app.help 下的三个单例(缓存 / 配置)。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.help

object CacheManager {
    private val mem = linkedMapOf<String, String>()

    fun put(key: String, value: Any, saveTime: Int = 0) {
        mem[key] = value.toString()
    }

    fun putMemory(key: String, value: String) {
        mem[key] = value
    }

    fun get(key: String): String? = mem[key]
    fun getFromMemory(key: String): String? = mem[key]
    fun clear() = mem.clear()
}

/** `isRule = true` 时注进页面的那个缓存对象;本套不覆盖 */
object WebCacheManager {
    fun getFromMemory(key: String): String? = CacheManager.getFromMemory(key)
}
