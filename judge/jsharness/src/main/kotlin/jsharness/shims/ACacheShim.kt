// 差分垫片:`ACache` 真身是**落盘**缓存(File + appCtx.cacheDir + 定时清理),
// 纯 JVM 里既挂不上也不该挂 —— 差分要的是**冷算**,不是缓存命中。
//
// 这里只顶 `BookSourceExtensions` 用到的那四个口(get / getAsString / put /
// remove),背后是一张内存表;`clear()` 给执行器每例开跑前清场用。
//
// **口径**:被测侧(rubato)的 `pipeline::explore_kinds` 根本没有缓存层,
// 每次都是冷算。两侧因此对齐在「冷算一次」这一位上 —— 缓存是**产品面**的
// 事(真身进程内 `exploreKindsMap` + 落盘 ACache),不是判据面的事。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.utils

import java.io.File

class ACache private constructor() {

    private val table = HashMap<String, String>()

    fun getAsString(key: String): String? = table[key]

    fun put(key: String, value: String) {
        table[key] = value
    }

    fun remove(key: String): Boolean = table.remove(key) != null

    fun clear() = table.clear()

    companion object {
        private val instances = HashMap<String, ACache>()

        @JvmStatic
        @JvmOverloads
        fun get(
            cacheName: String = "ACache",
            maxSize: Long = 0,
            maxCount: Int = 0,
            cacheDir: Boolean = true
        ): ACache = synchronized(this) { instances.getOrPut(cacheName) { ACache() } }

        @JvmStatic
        @JvmOverloads
        fun get(cacheDir: File, maxSize: Long = 0, maxCount: Int = 0): ACache =
            get(cacheDir.name, maxSize, maxCount)

        /** 每个 case 开跑前清场:差分不许跨例带缓存 */
        fun clearAll() = synchronized(this) { instances.values.forEach { it.clear() } }
    }
}
