package com.script.rhino

import com.script.CompiledScript
import com.script.ScriptBindings
import com.script.StubCompiledScript
import org.htmlunit.corejs.javascript.TopLevel
import org.htmlunit.corejs.javascript.VarScope
import kotlin.coroutines.CoroutineContext

// 差分垫片:与 com.script 的桩配套(JS 桩指令集见 fixtures/cases/rule-engine/README.md)
object RhinoScriptEngine {
    fun initialize() = Unit
    fun getRuntimeScope(bindings: ScriptBindings): ScriptBindings = bindings
    fun newStandardTopLevel(): TopLevel = TopLevel()
    fun compile(script: String): CompiledScript = StubCompiledScript(script)
    fun eval(script: String, scope: VarScope): Any? = eval(script, scope, null)

    fun eval(script: String, scope: VarScope, coroutineContext: CoroutineContext?): Any? =
        StubCompiledScript(script).eval(scope, coroutineContext)

    // 真身:eval(js) { bindingsConfig } —— RegexExtensions 垫片、BaseSource 桩会用
    fun eval(script: String, bindingsConfig: ScriptBindings.() -> Unit): Any? {
        val bindings = ScriptBindings()
        bindings.apply(bindingsConfig)
        return eval(script, bindings, null)
    }

    inline fun <T> run(block: RhinoScriptEngine.() -> T): T = this.block()
}

// 真身负责把 Rhino Context 绑到协程上下文;差分里直接执行
inline fun <T> runScriptWithContext(coroutineContext: CoroutineContext, block: () -> T): T = block()
