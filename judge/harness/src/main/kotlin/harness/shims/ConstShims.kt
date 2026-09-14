@file:Suppress("unused")

package io.legado.app.constant

object AppConst {
    const val UA_NAME = "User-Agent"
}

// constant/AppLog.kt:差分不落日志
object AppLog {
    fun put(message: String?, throwable: Throwable? = null, toast: Boolean = false) = Unit
}
