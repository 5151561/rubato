// 差分垫片:androidx.collection.LruCache(RhinoScriptEngine/ClassNameMatcher 用)。
// 真身是 LRU 淘汰的缓存;差分里只要「存得进取得出」,不设上限。
@file:Suppress("unused", "UNUSED_PARAMETER", "PackageDirectoryMismatch")

package androidx.collection

open class LruCache<K : Any, V : Any>(maxSize: Int) {
    private val map = LinkedHashMap<K, V>()

    open fun create(key: K): V? = null

    operator fun get(key: K): V? = map[key] ?: create(key)?.also { map[key] = it }

    open fun put(key: K, value: V): V? = map.put(key, value)

    open fun remove(key: K): V? = map.remove(key)

    open fun evictAll() = map.clear()

    open fun size(): Int = map.size

    open fun snapshot(): Map<K, V> = LinkedHashMap(map)
}
