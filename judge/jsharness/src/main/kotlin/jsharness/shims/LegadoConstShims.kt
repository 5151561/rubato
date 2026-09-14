// 差分垫片:AppConst —— 真身在 object 初始化里就摸 appCtx.resources / packageManager
// (非 lazy),挂不进纯 JVM。这里**逐字复刻**被挂载文件真正读到的成员。
@file:Suppress("unused")

package io.legado.app.constant

import org.apache.commons.lang3.time.FastDateFormat

object AppConst {
    // 真身 AppConst.kt L27
    const val UA_NAME = "User-Agent"

    // 真身 L39-41(JsExtensions.timeFormat 用它格式化毫秒时间戳)
    val dateFormat: FastDateFormat by lazy {
        FastDateFormat.getInstance("yyyy/MM/dd HH:mm")
    }

    // 真身读 Settings.Secure.ANDROID_ID;差分固定值(判据面见 README「不可差分面」)。
    // **必须正好 16 字符**:BaseSource.getLoginInfo/putLoginInfo 拿它做 AES 密钥
    // (`androidId.encodeToByteArray(0, 16)`),短一个字就是 IndexOutOfBounds ——
    // 那样登录信息面在裁判侧永远失败,比出来的是垫片的长度而不是语义。
    val androidId: String = "rubatodifftest16"
}
