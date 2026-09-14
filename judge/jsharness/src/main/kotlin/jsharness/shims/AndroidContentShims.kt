// 差分垫片:Context 与 splitties appCtx —— JsExtensions 的 UI/文件面只当类型用。
@file:Suppress("unused", "UNUSED_PARAMETER", "PackageDirectoryMismatch")

package android.content

open class Context {
    fun getSystemService(name: String): Any? = null
    val externalCacheDir: java.io.File? = null
    val cacheDir: java.io.File = java.io.File(System.getProperty("java.io.tmpdir"), "rubato-jsharness")
    fun getString(id: Int): String = "«res:$id»"
    fun getString(id: Int, vararg args: Any?): String = "«res:$id»"
    fun startActivity(intent: Any?) = Unit
}

class Intent {
    /** **记下来**:`startActivity` 那一侧的剧本用户要照着答(见 jsharness.VerifyStage) */
    val extras = linkedMapOf<String, Any?>()

    fun putExtra(k: String, v: String?): Intent = apply { extras[k] = v }
    fun putExtra(k: String, v: Int): Intent = apply { extras[k] = v }
    fun putExtra(k: String, v: Boolean): Intent = apply { extras[k] = v }

    // 真身传的是 `Boolean?`(`saveResult` / `refetchAfterSuccess` 都是可空的)——
    // Android 那边它落在 `putExtra(String, Serializable)` 那个重载上,这里同此
    fun putExtra(k: String, v: java.io.Serializable?): Intent = apply { extras[k] = v }
    var type: String? = null
    var data: android.net.Uri? = null
}
