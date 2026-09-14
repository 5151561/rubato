@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.utils.compress

import io.legado.app.exception.NoStackTraceException
import io.legado.app.utils.FS_UNSUPPORTED
import java.io.File

// 压缩包面(libarchive)属砍单/Phase 3
object LibArchiveUtils {
    fun unArchive(input: File, output: File, filter: ((String) -> Boolean)? = null): List<File> =
        throw NoStackTraceException(FS_UNSUPPORTED)

    fun getByteArrayContent(input: java.io.InputStream, path: String): ByteArray? =
        throw NoStackTraceException(FS_UNSUPPORTED)
}
