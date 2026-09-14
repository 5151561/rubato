// 差分垫片:model/ 下被挂载文件触到的对象。
// Debug 真身绑 UI 会话/时间戳,此处保 log/callback 面;其余为身份桩。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.model

import com.script.ScriptBindings
import io.legado.app.data.entities.BaseBook
import io.legado.app.data.entities.Book
import io.legado.app.data.entities.BookChapter
import io.legado.app.data.entities.BookSource
import kotlin.coroutines.CoroutineContext

object Debug {
    var callback: Callback? = null

    // :harness 那份是空实现;js-host 差分要把 java.log 当**可比较的输出**,故收集
    val logs = ArrayList<String>()

    fun log(
        sourceUrl: String?,
        msg: String = "",
        print: Boolean = true,
        isHtml: Boolean = false,
        showTime: Boolean = true,
        state: Int = 1
    ) {
        logs.add(msg)
    }

    fun log(msg: String?) {
        logs.add(msg ?: "null")
    }

    fun clear() = logs.clear()

    interface Callback {
        fun printLog(state: Int, msg: String)
    }
}

// model/ReadBook.kt:实体 delete() 触到;差分无阅读会话
object ReadBook {
    var book: Book? = null
}

// model/SharedJsScope.kt:桩不共享作用域
object SharedJsScope {
    fun getScope(jsLib: String?, coroutineContext: CoroutineContext? = null): ScriptBindings? = null
    fun getCryptoScope(owner: Any, coroutineContext: CoroutineContext?): ScriptBindings? = null
    fun remove(jsLib: String?) = Unit
}

// model/AudioPlay.kt:BookSource.kt 的 import 触到(死引用);身份桩
object AudioPlay

