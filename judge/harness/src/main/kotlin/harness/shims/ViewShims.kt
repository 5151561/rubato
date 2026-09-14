// 差分垫片:data/entities/rule/FlexChildStyle.kt 的 apply(View) 只在书源发现页
// 的 UI 上用得到,差分永不调用。此处给出最小类型面让真身文件挂得进纯 JVM。
@file:Suppress("unused", "UNUSED_PARAMETER", "PackageDirectoryMismatch")

package android.view

class ViewGroup {
    open class LayoutParams
}

open class View {
    var layoutParams: Any? = null
}
