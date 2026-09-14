// 差分垫片:io.legado.app.utils 下 JsExtensions/BaseSource 触到、但绑死
// Android 文件系统 / UI / 主线程的面。
//
// **契约**:凡属「本套不接的能力」(文件、压缩包、简繁转换、UI),统一抛
// NoStackTraceException(FS_UNSUPPORTED);被测侧同名 API 抛同一标记,
// 差分因此比的是「两边都到不了」而不是「两边各自乱给」。见
// fixtures/cases/js-host/README.md「不可差分面」。
@file:Suppress("unused", "UNUSED_PARAMETER")

package io.legado.app.utils

import io.legado.app.exception.NoStackTraceException
import java.io.File

const val FS_UNSUPPORTED = "«fs-unsupported»"

private fun nope(): Nothing = throw NoStackTraceException(FS_UNSUPPORTED)

object FileUtils {
    fun getPath(root: File, vararg subDirFiles: String): String = nope()
    fun getPath(rootPath: String, vararg subDirFiles: String): String = nope()
    fun getCachePath(): String = nope()
    fun createFolderIfNotExist(vararg path: String): File = nope()
    fun createFileIfNotExist(vararg path: String): File = nope()
    fun delete(path: String, deleteRootDir: Boolean = true): Boolean = false
    fun delete(file: File, deleteRootDir: Boolean = true): Boolean = false
    fun exist(path: String): Boolean = false
    fun readText(path: String): String = nope()
}

object ArchiveUtils {
    const val TEMP_FOLDER_NAME = "temp"
    fun deCompress(zipPath: String, path: String = ""): List<File> = nope()
}

// utils/FileExtensions.kt 真身(两个函数逐字复刻,依赖的是纯 java.io)
internal fun File.isSameOrDescendantOf(parent: File): Boolean {
    val parentPath = parent.canonicalFile.toPath()
    return canonicalFile.toPath().startsWith(parentPath)
}

fun File.createFileReplace(): File = nope()

// utils/ContextExtensions.kt:appCtx.externalCache
val android.content.Context.externalCache: File
    get() = nope()

fun android.content.Context.toastOnUi(msg: CharSequence?) = Unit
fun android.content.Context.toastOnUi(msg: Int) = Unit
fun android.content.Context.longToastOnUi(msg: CharSequence?) = Unit
fun android.content.Context.longToastOnUi(msg: Int) = Unit
// utils/IntentExtensions.kt:inline fun <reified T> Context.startActivity(block: Intent.() -> Unit)
//
// **界面在差分里换成剧本用户**:`SourceVerificationHelp` 弹的那两个 Activity
// (VerificationCodeActivity / WebViewActivity)在这里由 `jsharness.VerifyStage`
// 顶掉 —— 它照 case 的 `verify` 字段替用户答一句。被测侧的同一份在
// `difftest::verify_script`。
inline fun <reified T> android.content.Context.startActivity(
    noinline configIntent: android.content.Intent.() -> Unit = {},
) {
    val intent = android.content.Intent()
    configIntent(intent)
    jsharness.VerifyStage.opened(T::class.java.simpleName, intent.extras)
}

// utils/HandlerUtils.kt:差分永远跑在非主线程分支
val isMainThread: Boolean get() = false

fun Any.runOnUiThread(block: () -> Unit) = block()

// utils/ChineseUtils.kt:真身拉 quick-transfer 词典;差分契约「简繁转换关」
// (AppConfig.chineseConverterType 恒 0),这里恒等映射(与 :harness 同口径)
object ChineseUtils {
    fun s2t(content: String): String = content
    fun t2s(content: String): String = content
    fun preLoad(async: Boolean, vararg transType: Any) = Unit
}

object UrlUtil {
    // 真身 utils/UrlUtil.kt:取 url 的扩展名(downloadFile 用)
    fun getSuffix(url: String, default: String = "zip"): String {
        val suffix = url.substringAfterLast(".").substringBefore("?")
        return if (suffix.length in 1..4 && suffix.all { it.isLetterOrDigit() }) suffix else default
    }
}

// utils/StringExtensions.kt 里 JsExtensions 用到的一条(按字符簇切分)
fun String.toStringArray(): Array<String> {
    val list = ArrayList<String>()
    var i = 0
    while (i < length) {
        val n = Character.charCount(codePointAt(i))
        list.add(substring(i, i + n))
        i += n
    }
    return list.toTypedArray()
}

fun Any?.showDialogFragment(vararg args: Any?) = Unit
