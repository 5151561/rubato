// 差分垫片:AppLog / printOnDebug / AppConfig —— 日志与配置面不进差分。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.constant

object AppLog {
    // 真身写内存日志列表 + 发事件;差分只留可观察的一份,给 harness 断言用
    val logs = ArrayList<String>()

    fun put(message: String?, throwable: Throwable? = null, toast: Boolean = false) {
        logs.add(message ?: "null")
    }

    fun putDebug(message: String?, throwable: Throwable? = null) {
        logs.add(message ?: "null")
    }

    fun clear() = logs.clear()
}
