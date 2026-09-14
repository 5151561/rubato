@file:Suppress("unused", "UNUSED_PARAMETER")

package androidx.appcompat.app

// JsExtensions.showBrowser 只把它当「顶层 Activity」的类型用;差分里永远取不到
open class AppCompatActivity : android.content.Context() {
    val isFinishing: Boolean = true
    val isDestroyed: Boolean = true
    val supportFragmentManager: FragmentManagerStub = FragmentManagerStub()

    fun runOnUiThread(block: Runnable) = block.run()

    class FragmentManagerStub {
        val isStateSaved: Boolean = true
    }
}
