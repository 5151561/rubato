// 差分垫片:WebBook.exploreBookAwait 从发现页适配器的 LruCache 里取 InfoMap
// (androidx.collection.LruCache + RecyclerView,挂不进纯 JVM)。差分每例独立,
// 缓存起点恒空;InfoMap 真身已挂载,写入语义与真身一致。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.ui.main.explore

import io.legado.app.utils.InfoMap

class ExploreAdapter {
    companion object {
        val exploreInfoMapList = LinkedHashMap<String, InfoMap>()

        // harness 差分观察面
        fun clearCache() = exploreInfoMapList.clear()
    }
}
