// 差分垫片:把 Rhino 换成**确定性 JS 桩**。
//
// 为什么不接真 Rhino:Phase 1 的被测对象是 AnalyzeRule 的分发/拼接逻辑,
// 接真引擎会把 JS 方言差异混进来(那是 Phase 2 js-host 的差分范围)。
// 桩的指令集契约见 fixtures/cases/rule-engine/README.md,两侧必须一致。
package com.script

import org.htmlunit.corejs.javascript.TopLevel
import org.htmlunit.corejs.javascript.VarScope
import kotlin.coroutines.CoroutineContext

// 真身:com.script.SharedGlobalStateHandle(rhino 模块);桩只承载身份
class SharedGlobalStateHandle(val key: String)

class ScriptBindings : TopLevel() {
    val vars = LinkedHashMap<String, Any?>()

    operator fun set(key: String, value: Any?) {
        vars[key] = value
    }

    operator fun set(index: Int, value: Any?) {
        vars[index.toString()] = value
    }

    fun put(key: String, value: Any?) = set(key, value)

    // 真身把父作用域接到原型链;桩不共享作用域,空操作
    fun chainTo(parent: TopLevel, sharedGlobalState: SharedGlobalStateHandle? = null) = Unit

    companion object {
        private val handles = HashMap<String, SharedGlobalStateHandle>()
        fun getSharedGlobalStateHandle(key: String): SharedGlobalStateHandle =
            handles.getOrPut(key) { SharedGlobalStateHandle(key) }
        fun removeSharedGlobalState(handle: SharedGlobalStateHandle) {
            handles.remove(handle.key)
        }
        fun removeSharedGlobalStatesBySource(className: String, keyMd5: String) = Unit
    }
}

inline fun buildScriptBindings(block: (bindings: ScriptBindings) -> Unit): ScriptBindings {
    val bindings = ScriptBindings()
    block(bindings)
    return bindings
}

// 真身 rhino/src/main/java/com/script/CompiledScript.kt:作用域类型是 VarScope(非 Scriptable)
abstract class CompiledScript {
    fun eval(scope: VarScope): Any? = eval(scope, null)
    abstract fun eval(scope: VarScope, coroutineContext: CoroutineContext?): Any?
    abstract suspend fun evalSuspend(scope: VarScope): Any?
}

class StubJsException(msg: String) : RuntimeException(msg)

/** 桩解释器:整段源码就是一条指令,不认的指令原样当字符串返回 */
class StubCompiledScript(private val src: String) : CompiledScript() {

    override suspend fun evalSuspend(scope: VarScope): Any? = eval(scope, null)

    override fun eval(scope: VarScope, coroutineContext: CoroutineContext?): Any? {
        val vars = (scope as? ScriptBindings)?.vars ?: LinkedHashMap()
        val s = src.trim()
        val java = vars["java"]
        fun call(name: String, vararg args: Any?): Any? {
            val m = java!!.javaClass.methods.first { it.name == name && it.parameterCount == args.size }
            return m.invoke(java, *args)
        }
        return when {
            s == "#null" -> null
            s == "#result" -> vars["result"]?.toString()
            s == "#baseUrl" -> vars["baseUrl"]
            s == "#src" -> vars["src"]?.toString()
            s == "#title" -> vars["title"]
            s == "#nextChapterUrl" -> vars["nextChapterUrl"]
            s == "#fromBookInfo" -> vars["fromBookInfo"]?.toString()
            s == "#key" -> vars["key"]
            s == "#page" -> vars["page"]?.toString()
            s == "#err" -> throw StubJsException("stub js error")
            s.startsWith("#num:") -> s.substring(5).toDouble()
            s.startsWith("#bool:") -> s.substring(6).toBoolean()
            s.startsWith("#echo:") -> s.substring(6)
            s.startsWith("#list:") -> s.substring(6).split("|")
            s.startsWith("#get:") -> call("get", s.substring(5))
            s.startsWith("#put:") -> {
                val body = s.substring(5)
                val i = body.indexOf('=')
                call("put", body.substring(0, i), body.substring(i + 1))
            }
            else -> src
        }
    }
}
