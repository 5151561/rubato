// 差分垫片:android.system(仅 icu4j 的文件描述符路径触到,永不调用)
@file:Suppress("unused", "UNUSED_PARAMETER", "PackageDirectoryMismatch", "FunctionName")

package android.system

import java.io.FileDescriptor

object OsConstants {
    @JvmField
    val SEEK_SET: Int = 0
}

object Os {
    @JvmStatic
    fun lseek(fd: FileDescriptor, offset: Long, whence: Int): Long =
        throw UnsupportedOperationException("差分桩")

    @JvmStatic
    fun read(fd: FileDescriptor, bytes: ByteArray, byteOffset: Int, byteCount: Int): Int =
        throw UnsupportedOperationException("差分桩")
}
