// 差分垫片:实体里 getString(R.string.x)/toastOnUi 需要的 Context 面
@file:Suppress("unused", "UNUSED_PARAMETER")

package android.content

open class Context {
    fun getString(id: Int): String = "res:$id"
    fun getString(id: Int, vararg args: Any?): String = "res:$id:${args.joinToString(",")}"
}
