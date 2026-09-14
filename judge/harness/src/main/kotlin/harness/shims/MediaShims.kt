// 差分垫片:AnalyzeUrl 的听书出口 getMediaItem() 的类型面。
// 该出口是播放器专用,四步流水线差分不经过;这里只给类型,让 AnalyzeUrl.kt 真身
// 挂得进纯 JVM harness。
@file:Suppress("unused", "UNUSED_PARAMETER", "PackageDirectoryMismatch")

package androidx.media3.common

class MediaItem private constructor(val uri: String) {
    class Builder {
        private var uri: String = ""
        fun setUri(uri: String): Builder = apply { this.uri = uri }
        fun build(): MediaItem = MediaItem(uri)
    }
}
